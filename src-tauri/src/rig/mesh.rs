use image::RgbaImage;
use std::collections::HashMap;

use super::ik::point_segment_distance;
use super::types::Rig;

#[derive(Debug, Clone)]
pub(super) struct MeshVertex {
    pub(super) source: (f64, f64),
    pub(super) influences: Vec<(usize, f64)>,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct MeshTriangle {
    pub(super) vertices: [usize; 3],
    pub(super) z: f64,
}

#[derive(Debug, Clone)]
pub(super) struct DeformMesh {
    pub(super) vertices: Vec<MeshVertex>,
    pub(super) triangles: Vec<MeshTriangle>,
}

fn is_core_bone(name: &str) -> bool {
    name.split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty())
        .any(|token| {
            matches!(
                token.to_ascii_lowercase().as_str(),
                "body" | "torso" | "pelvis" | "spine" | "chest" | "hip" | "segment"
            )
        })
}

/// Derive smooth, normalized bone weights from the bind-pose capsules. The
/// four strongest influences are retained so the result is stable and cheap
/// to evaluate. Core bones receive a small bias so torso/spine transforms
/// deform the silhouette instead of being visually swallowed by nearby limbs.
fn vertex_influences(
    x: f64,
    y: f64,
    rig: &Rig,
    positions: &HashMap<String, (f64, f64)>,
) -> Vec<(usize, f64)> {
    let mut weights: Vec<(usize, f64)> = rig
        .bones
        .iter()
        .enumerate()
        .map(|(index, bone)| {
            let start = positions
                .get(&bone.start_point)
                .copied()
                .unwrap_or((0.0, 0.0));
            let end = positions
                .get(&bone.end_point)
                .copied()
                .unwrap_or((0.0, 0.0));
            let normalized =
                point_segment_distance(x, y, start.0, start.1, end.0, end.1) / bone.radius.max(0.5);
            let core_bias = if is_core_bone(&bone.name) { 1.35 } else { 1.0 };
            // Inverse-square falloff remains well-defined outside every
            // capsule, which guarantees complete coverage of a flat sprite.
            (index, core_bias / (0.35 + normalized).powi(2))
        })
        .collect();
    weights.sort_by(|left, right| {
        right
            .1
            .total_cmp(&left.1)
            .then_with(|| left.0.cmp(&right.0))
    });
    weights.truncate(4);
    let total: f64 = weights.iter().map(|(_, weight)| *weight).sum();
    if total <= f64::EPSILON {
        return vec![(0, 1.0)];
    }
    for (_, weight) in &mut weights {
        *weight /= total;
    }
    weights
}

/// Build a deterministic adaptive grid over the opaque silhouette. This is a
/// deliberately compact first-stage mesh: cells that contain no source alpha
/// are omitted, while exact image boundaries are retained. No random sampling
/// means identical inputs always produce identical topology and pixels.
pub(super) fn build_deform_mesh(
    master: &RgbaImage,
    rig: &Rig,
    positions: &HashMap<String, (f64, f64)>,
) -> DeformMesh {
    let width = master.width() as usize;
    let height = master.height() as usize;
    let longest = width.max(height);
    let spacing = if longest <= 64 {
        2
    } else if longest <= 128 {
        4
    } else if longest <= 256 {
        6
    } else {
        8
    };
    let axis = |length: usize| {
        let mut values: Vec<usize> = (0..length).step_by(spacing).collect();
        if values.last().copied() != Some(length) {
            values.push(length);
        }
        values
    };
    let xs = axis(width);
    let ys = axis(height);
    let mut vertices = Vec::with_capacity(xs.len() * ys.len());
    for &y in &ys {
        for &x in &xs {
            vertices.push(MeshVertex {
                source: (x as f64, y as f64),
                influences: vertex_influences(x as f64, y as f64, rig, positions),
            });
        }
    }
    let row_width = xs.len();
    let mut triangles = Vec::new();
    for row in 0..ys.len().saturating_sub(1) {
        for column in 0..xs.len().saturating_sub(1) {
            let has_alpha = (ys[row]..ys[row + 1]).any(|y| {
                (xs[column]..xs[column + 1]).any(|x| master.get_pixel(x as u32, y as u32)[3] > 0)
            });
            if !has_alpha {
                continue;
            }
            let top_left = row * row_width + column;
            let top_right = top_left + 1;
            let bottom_left = top_left + row_width;
            let bottom_right = bottom_left + 1;
            for indices in [
                [top_left, top_right, bottom_right],
                [top_left, bottom_right, bottom_left],
            ] {
                let z = indices
                    .iter()
                    .map(|vertex| {
                        vertices[*vertex]
                            .influences
                            .iter()
                            .map(|(bone, weight)| rig.bones[*bone].z as f64 * weight)
                            .sum::<f64>()
                    })
                    .sum::<f64>()
                    / 3.0;
                triangles.push(MeshTriangle {
                    vertices: indices,
                    z,
                });
            }
        }
    }
    triangles.sort_by(|left, right| left.z.total_cmp(&right.z));
    DeformMesh {
        vertices,
        triangles,
    }
}
