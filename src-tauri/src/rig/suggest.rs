use crate::error::{CommandError, CommandResult};
use image::RgbaImage;
use std::collections::HashMap;

use super::biped_template::{BIPED_BONES, BIPED_POINTS};
use super::creature_templates::{
    QUADRUPED_BONES, QUADRUPED_POINTS, SERPENTINE_BONES, SERPENTINE_POINTS, WINGED_BONES,
    WINGED_POINTS,
};
use super::object_templates::{AMPHORPHOUS_BONES, AMPHORPHOUS_POINTS, OBJECT_BONES, OBJECT_POINTS};
use super::types::{RigBone, RigPoint, RigSuggestion, Template};

pub(crate) fn distance_transform(alpha: &[bool], width: usize, height: usize) -> Vec<f64> {
    let large = (width.max(height) as f64) * 4.0;
    // Transparent pixels are the source (0); opaque pixels accumulate their
    // distance to the nearest transparent pixel through the chamfer passes.
    let mut distances = vec![0.0; width * height];
    for (index, opaque) in alpha.iter().enumerate() {
        if *opaque {
            distances[index] = large;
        }
    }
    for y in 0..height {
        for x in 0..width {
            let index = y * width + x;
            let mut value = distances[index];
            if x > 0 {
                value = value.min(distances[index - 1] + 3.0);
            }
            if y > 0 {
                value = value.min(distances[index - width] + 3.0);
                if x > 0 {
                    value = value.min(distances[index - width - 1] + 4.0);
                }
                if x + 1 < width {
                    value = value.min(distances[index - width + 1] + 4.0);
                }
            }
            distances[index] = value;
        }
    }
    for y in (0..height).rev() {
        for x in (0..width).rev() {
            let index = y * width + x;
            let mut value = distances[index];
            if x + 1 < width {
                value = value.min(distances[index + 1] + 3.0);
            }
            if y + 1 < height {
                value = value.min(distances[index + width] + 3.0);
                if x + 1 < width {
                    value = value.min(distances[index + width + 1] + 4.0);
                }
                if x > 0 {
                    value = value.min(distances[index + width - 1] + 4.0);
                }
            }
            distances[index] = value;
        }
    }
    distances
}

fn template_for(morphology: &str) -> Template {
    match morphology {
        "quadruped" => Template {
            points: QUADRUPED_POINTS,
            bones: QUADRUPED_BONES,
        },
        "winged" => Template {
            points: WINGED_POINTS,
            bones: WINGED_BONES,
        },
        "serpentine" => Template {
            points: SERPENTINE_POINTS,
            bones: SERPENTINE_BONES,
        },
        "object" => Template {
            points: OBJECT_POINTS,
            bones: OBJECT_BONES,
        },
        "amorphous" => Template {
            points: AMPHORPHOUS_POINTS,
            bones: AMPHORPHOUS_BONES,
        },
        _ => Template {
            points: BIPED_POINTS,
            bones: BIPED_BONES,
        },
    }
}

pub(crate) fn suggest_points(master: &RgbaImage, morphology: &str) -> CommandResult<RigSuggestion> {
    let width = master.width() as usize;
    let height = master.height() as usize;
    if width == 0 || height == 0 {
        return Err(CommandError::new(
            "invalid_master",
            "The source sprite has no pixels to rig",
        ));
    }
    let alpha: Vec<bool> = master.pixels().map(|pixel| pixel[3] > 0).collect();
    if !alpha.iter().any(|value| *value) {
        return Err(CommandError::new(
            "empty_alpha",
            "The source sprite is fully transparent",
        ));
    }
    let distances = distance_transform(&alpha, width, height);
    let mut min_x = width;
    let mut min_y = height;
    let mut max_x = 0usize;
    let mut max_y = 0usize;
    for y in 0..height {
        for x in 0..width {
            if alpha[y * width + x] {
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
        }
    }
    let box_width = (max_x - min_x + 1) as f64;
    let box_height = (max_y - min_y + 1) as f64;
    let template = template_for(morphology);
    let search_radius = ((box_width.min(box_height) * 0.08).round() as i64).max(2);
    let mut points = Vec::new();
    for (index, entry) in template.points.iter().enumerate() {
        let center_x = min_x as f64 + entry.nx * (box_width - 1.0);
        let center_y = min_y as f64 + entry.ny * (box_height - 1.0);
        let mut best: Option<(f64, f64, f64)> = None; // (score, x, y)
        let window = search_radius;
        let y_lo = (center_y as i64 - window).max(0) as usize;
        let y_hi = (center_y as i64 + window).min(height as i64 - 1) as usize;
        let x_lo = (center_x as i64 - window).max(0) as usize;
        let x_hi = (center_x as i64 + window).min(width as i64 - 1) as usize;
        for y in y_lo..=y_hi {
            for x in x_lo..=x_hi {
                let index = y * width + x;
                if !alpha[index] {
                    continue;
                }
                // Prefer thick, medial pixels close to the template position;
                // thickness weighs double so limb points snap to limb centers
                // rather than the near edge of a thin limb.
                let offset = ((x as f64 - center_x).powi(2) + (y as f64 - center_y).powi(2)).sqrt();
                let score = (distances[index] / 3.0) * 2.0 - offset;
                if best.is_none() || score > best.unwrap().0 {
                    best = Some((score, x as f64, y as f64));
                }
            }
        }
        let (x, y, confidence) = match best {
            Some((_, x, y)) => {
                let thickness = distances[y as usize * width + x as usize] / 3.0;
                let confidence = (0.45 + 0.5 * (thickness / search_radius as f64)).clamp(0.4, 0.97);
                (x, y, confidence)
            }
            None => (center_x, center_y, 0.3),
        };
        points.push(RigPoint {
            id: format!("p{}", index + 1),
            name: entry.name.to_string(),
            kind: entry.kind.to_string(),
            x,
            y,
            confidence: (confidence * 100.0).round() / 100.0,
            source: "auto".to_string(),
            note: None,
        });
    }
    let positions: HashMap<String, (f64, f64)> = points
        .iter()
        .map(|point| (point.name.clone(), (point.x, point.y)))
        .collect();
    let scale_reference = box_width.min(box_height);
    let mut bones = Vec::new();
    for (index, entry) in template.bones.iter().enumerate() {
        let Some(&start) = positions.get(entry.start) else {
            continue;
        };
        let Some(&end) = positions.get(entry.end) else {
            continue;
        };
        let midpoint_thickness = {
            let mx = ((start.0 + end.0) / 2.0)
                .round()
                .clamp(0.0, (width - 1) as f64) as usize;
            let my = ((start.1 + end.1) / 2.0)
                .round()
                .clamp(0.0, (height - 1) as f64) as usize;
            distances[my * width + mx] / 3.0
        };
        let radius =
            (midpoint_thickness * entry.radius_factor).clamp(1.0, (scale_reference / 2.0).max(3.0));
        bones.push(RigBone {
            id: format!("b{}", index + 1),
            name: entry.name.to_string(),
            start_point: entry.start.to_string(),
            end_point: entry.end.to_string(),
            radius: (radius * 10.0).round() / 10.0,
            parent: entry.parent.map(str::to_string),
            z: entry.z,
        });
    }
    let reasoning = format!(
        "Placed {} points and {} bones from the {morphology} anatomy template, snapped onto the medial axis of the {}×{} silhouette.",
        points.len(),
        bones.len(),
        master.width(),
        master.height()
    );
    Ok(RigSuggestion {
        morphology: morphology.to_string(),
        points,
        bones,
        frames: Vec::new(),
        reasoning,
        source: "auto".to_string(),
    })
}
