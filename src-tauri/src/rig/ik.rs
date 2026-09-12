use image::RgbaImage;
use std::collections::HashMap;

use super::types::{Rig, RigBone, RigTransform};

#[derive(Debug, Clone, Copy)]
pub(crate) struct Affine {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub e: f64,
    pub f: f64,
}

impl Affine {
    pub(crate) fn identity() -> Self {
        Self {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            e: 0.0,
            f: 0.0,
        }
    }

    pub(crate) fn translation(dx: f64, dy: f64) -> Self {
        Self {
            e: dx,
            f: dy,
            ..Self::identity()
        }
    }

    pub(crate) fn scaling(sx: f64, sy: f64) -> Self {
        Self {
            a: sx,
            d: sy,
            ..Self::identity()
        }
    }

    pub(crate) fn rotation_degrees(degrees: f64) -> Self {
        let radians = degrees.to_radians();
        let (sin, cos) = radians.sin_cos();
        Self {
            a: cos,
            b: sin,
            c: -sin,
            d: cos,
            e: 0.0,
            f: 0.0,
        }
    }

    /// self followed by next (next ∘ self).
    pub(crate) fn chain(&self, next: &Affine) -> Affine {
        Affine {
            a: next.a * self.a + next.c * self.b,
            b: next.b * self.a + next.d * self.b,
            c: next.a * self.c + next.c * self.d,
            d: next.b * self.c + next.d * self.d,
            e: next.a * self.e + next.c * self.f + next.e,
            f: next.b * self.e + next.d * self.f + next.f,
        }
    }

    pub(crate) fn apply(&self, x: f64, y: f64) -> (f64, f64) {
        (
            self.a * x + self.c * y + self.e,
            self.b * x + self.d * y + self.f,
        )
    }

    pub(crate) fn inverse(&self) -> Affine {
        let determinant = self.a * self.d - self.b * self.c;
        if determinant.abs() < f64::EPSILON {
            return Affine::identity();
        }
        let inverse = 1.0 / determinant;
        Affine {
            a: self.d * inverse,
            b: -self.b * inverse,
            c: -self.c * inverse,
            d: self.a * inverse,
            e: (self.c * self.f - self.d * self.e) * inverse,
            f: (self.b * self.e - self.a * self.f) * inverse,
        }
    }
}

fn local_bone_affine(start: (f64, f64), transform: &RigTransform) -> Affine {
    Affine::translation(-start.0, -start.1)
        .chain(&Affine::scaling(transform.scale_x, transform.scale_y))
        .chain(&Affine::rotation_degrees(transform.rotate))
        .chain(&Affine::translation(start.0, start.1))
        .chain(&Affine::translation(transform.dx, transform.dy))
}

// ---------------------------------------------------------------------------
// Geometry helpers
// ---------------------------------------------------------------------------

pub(crate) fn point_segment_distance(px: f64, py: f64, ax: f64, ay: f64, bx: f64, by: f64) -> f64 {
    let dx = bx - ax;
    let dy = by - ay;
    let length_sq = dx * dx + dy * dy;
    if length_sq <= f64::EPSILON {
        ((px - ax).powi(2) + (py - ay).powi(2)).sqrt()
    } else {
        let t = (((px - ax) * dx + (py - ay) * dy) / length_sq).clamp(0.0, 1.0);
        let cx = ax + t * dx;
        let cy = ay + t * dy;
        ((px - cx).powi(2) + (py - cy).powi(2)).sqrt()
    }
}

pub(crate) fn point_positions(rig: &Rig, width: u32, height: u32) -> HashMap<String, (f64, f64)> {
    let mut positions = HashMap::new();
    for point in &rig.points {
        positions.insert(
            point.name.clone(),
            (
                point.x.clamp(0.0, (width.saturating_sub(1)) as f64),
                point.y.clamp(0.0, (height.saturating_sub(1)) as f64),
            ),
        );
    }
    positions
}

fn better_candidate(current: Option<(f64, i64, usize)>, candidate: (f64, i64, usize)) -> bool {
    match current {
        None => true,
        Some((distance, z, _)) => {
            candidate.0 < distance || (candidate.0 == distance && candidate.1 < z)
        }
    }
}

struct Capsule {
    start: (f64, f64),
    end: (f64, f64),
    radius: f64,
    z: i64,
}

/// Assign every opaque master pixel to exactly one bone. Pixels inside a bone
/// capsule go to the closest capsule; leftover pixels fall to the nearest bone
/// by segment distance so the rig always re-renders the whole silhouette.
pub(crate) fn build_ownership(
    master: &RgbaImage,
    rig: &Rig,
    positions: &HashMap<String, (f64, f64)>,
) -> Vec<i16> {
    let width = master.width() as usize;
    let height = master.height() as usize;
    let mut assignment = vec![-1i16; width * height];
    if rig.bones.is_empty() {
        return assignment;
    }
    let capsules: Vec<Capsule> = rig
        .bones
        .iter()
        .map(|bone| Capsule {
            start: positions
                .get(&bone.start_point)
                .copied()
                .unwrap_or((0.0, 0.0)),
            end: positions
                .get(&bone.end_point)
                .copied()
                .unwrap_or((0.0, 0.0)),
            radius: bone.radius.max(0.5),
            z: bone.z,
        })
        .collect();
    for y in 0..height {
        for x in 0..width {
            if master.get_pixel(x as u32, y as u32)[3] == 0 {
                continue;
            }
            let px = x as f64 + 0.5;
            let py = y as f64 + 0.5;
            let mut best_capped: Option<(f64, i64, usize)> = None;
            let mut best_any: Option<(f64, i64, usize)> = None;
            for (index, capsule) in capsules.iter().enumerate() {
                let distance = point_segment_distance(
                    px,
                    py,
                    capsule.start.0,
                    capsule.start.1,
                    capsule.end.0,
                    capsule.end.1,
                );
                let candidate = (distance, capsule.z, index);
                if distance <= capsule.radius && better_candidate(best_capped, candidate) {
                    best_capped = Some(candidate);
                }
                if better_candidate(best_any, candidate) {
                    best_any = Some(candidate);
                }
            }
            if let Some((_, _, index)) = best_capped.or(best_any) {
                assignment[y * width + x] = index as i16;
            }
        }
    }
    assignment
}

/// Two-bone analytic IK. Rotates the contact bone's chain so the bone's end
/// point lands on the canvas-space target, with `bend` choosing the elbow or
/// knee direction. Returns rotation deltas in degrees for (parent, bone).
pub(super) fn solve_contact_ik(
    bones: &[RigBone],
    bone_index: usize,
    positions: &HashMap<String, (f64, f64)>,
    target: (f64, f64),
    bend: f64,
) -> (f64, f64) {
    let bone = &bones[bone_index];
    let joint = positions
        .get(&bone.start_point)
        .copied()
        .unwrap_or((0.0, 0.0));
    let end = positions
        .get(&bone.end_point)
        .copied()
        .unwrap_or((0.0, 0.0));
    let l2 = ((end.0 - joint.0).powi(2) + (end.1 - joint.1).powi(2)).sqrt();
    if l2 <= f64::EPSILON {
        return (0.0, 0.0);
    }
    let child_angle = (end.1 - joint.1).atan2(end.0 - joint.0);
    let Some(parent_index) = bones
        .iter()
        .position(|candidate| Some(candidate.name.clone()) == bone.parent)
    else {
        // Single-bone chain: point the bone straight at the target.
        let target_angle = (target.1 - joint.1).atan2(target.0 - joint.0);
        return (0.0, (target_angle - child_angle).to_degrees());
    };
    let parent = &bones[parent_index];
    let parent_start = positions
        .get(&parent.start_point)
        .copied()
        .unwrap_or((0.0, 0.0));
    let l1 = ((joint.0 - parent_start.0).powi(2) + (joint.1 - parent_start.1).powi(2)).sqrt();
    if l1 <= f64::EPSILON {
        return (0.0, 0.0);
    }
    let parent_angle = (joint.1 - parent_start.1).atan2(joint.0 - parent_start.0);
    let reach = ((target.0 - parent_start.0).powi(2) + (target.1 - parent_start.1).powi(2)).sqrt();
    let reach = reach.clamp((l1 - l2).abs() + 1e-3, l1 + l2 - 1e-3);
    let beta = ((l1 * l1 + reach * reach - l2 * l2) / (2.0 * l1 * reach))
        .clamp(-1.0, 1.0)
        .acos();
    let gamma = ((l1 * l1 + l2 * l2 - reach * reach) / (2.0 * l1 * l2))
        .clamp(-1.0, 1.0)
        .acos();
    let target_angle = (target.1 - parent_start.1).atan2(target.0 - parent_start.0);
    let bend = if bend >= 0.0 { 1.0 } else { -1.0 };
    let parent_delta = target_angle + bend * beta - parent_angle;
    let child_delta = bend * (std::f64::consts::PI - gamma) + parent_angle - child_angle;
    (parent_delta.to_degrees(), child_delta.to_degrees())
}

pub(super) fn identity_transform(bone: &str) -> RigTransform {
    RigTransform {
        bone: bone.to_string(),
        dx: 0.0,
        dy: 0.0,
        rotate: 0.0,
        scale_x: 1.0,
        scale_y: 1.0,
    }
}

pub(super) fn bone_world_affine(
    index: usize,
    bones: &[RigBone],
    positions: &HashMap<String, (f64, f64)>,
    transforms: &HashMap<String, RigTransform>,
    cache: &mut HashMap<usize, Affine>,
    depth: usize,
) -> Affine {
    if let Some(cached) = cache.get(&index) {
        return *cached;
    }
    // Cycle guard: a malformed parent loop renders each bone with identity
    // ancestors instead of recursing forever.
    if depth > bones.len() {
        return Affine::identity();
    }
    let bone = &bones[index];
    let transform = transforms
        .get(&bone.name)
        .cloned()
        .unwrap_or_else(|| identity_transform(&bone.name));
    let start = positions
        .get(&bone.start_point)
        .copied()
        .unwrap_or((0.0, 0.0));
    let local = local_bone_affine(start, &transform);
    let world = match bones
        .iter()
        .position(|candidate| Some(candidate.name.clone()) == bone.parent)
    {
        Some(parent_index) if parent_index != index => local.chain(&bone_world_affine(
            parent_index,
            bones,
            positions,
            transforms,
            cache,
            depth + 1,
        )),
        _ => local,
    };
    cache.insert(index, world);
    world
}

pub(super) type OwnedBounds = Vec<Option<((i64, i64), (i64, i64))>>;

/// Per-bone bounding box of owned source pixels, used as the render sweep
/// region so residual pixels outside the capsule are never dropped.
pub(crate) fn owned_bounds(ownership: &[i16], bone_count: usize, width: usize) -> OwnedBounds {
    let mut bounds = vec![None; bone_count];
    for (index, owner) in ownership.iter().enumerate() {
        if *owner < 0 {
            continue;
        }
        let owner = *owner as usize;
        if owner >= bounds.len() {
            continue;
        }
        let x = (index % width) as i64;
        let y = (index / width) as i64;
        let entry = bounds[owner].get_or_insert(((i64::MAX, i64::MAX), (i64::MIN, i64::MIN)));
        entry.0 .0 = entry.0 .0.min(x);
        entry.0 .1 = entry.0 .1.min(y);
        entry.1 .0 = entry.1 .0.max(x);
        entry.1 .1 = entry.1 .1.max(y);
    }
    bounds
}
