use crate::error::CommandResult;
use image::RgbaImage;
use serde::Serialize;
use std::collections::HashMap;

use super::ik::point_segment_distance;
use super::types::RigSuggestion;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MorphologyScore {
    pub morphology: String,
    pub confidence: f64,
    pub reasoning: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RigFitReport {
    pub detections: Vec<MorphologyScore>,
    pub recommended: RigSuggestion,
    pub capsule_fit: f64,
    pub warnings: Vec<String>,
}

#[derive(Debug, Default)]
struct BandShape {
    runs: usize,
    covered: f64,
    max_run: f64,
}

#[derive(Debug)]
pub(super) struct ShapeFeatures {
    aspect: f64,
    fill: f64,
    top: BandShape,
    mid: BandShape,
    lower: BandShape,
    /// Two lower-band runs on opposite sides of the silhouette's center.
    lower_straddle: bool,
    /// At least two separated runs somewhere in the lower half.
    pub(super) lower_separated: bool,
    /// Per-column thickness stays roughly constant (snake-like mass).
    uniform_thickness: bool,
}

fn band_shape(
    alpha: &[bool],
    width: usize,
    min_x: usize,
    max_x: usize,
    y_start: usize,
    y_end: usize,
    box_width: f64,
) -> BandShape {
    let span = (max_x - min_x + 1).max(1);
    let mut occupied = vec![false; span];
    for y in y_start..=y_end.min(alpha.len() / width - 1) {
        let row = y * width;
        for index in 0..span {
            if alpha[row + min_x + index] {
                occupied[index] = true;
            }
        }
    }
    let mut runs = Vec::new();
    let mut start: Option<usize> = None;
    for (index, is_occupied) in occupied.iter().copied().enumerate().take(span) {
        if is_occupied && start.is_none() {
            start = Some(index);
        }
        if start.is_some() && (!is_occupied || index + 1 == span) {
            let begin = start.take().expect("run start");
            let end = if is_occupied { index } else { index - 1 };
            if end - begin + 1 >= 2 {
                runs.push((begin, end));
            }
        }
    }
    let covered = runs
        .iter()
        .map(|run| (run.1 - run.0 + 1) as f64)
        .sum::<f64>()
        / span as f64;
    let max_run = runs
        .iter()
        .map(|run| (run.1 - run.0 + 1) as f64)
        .fold(0.0_f64, f64::max)
        / box_width.max(1.0);
    BandShape {
        runs: runs.len(),
        covered,
        max_run,
    }
}

pub(super) fn shape_features(master: &RgbaImage) -> Option<ShapeFeatures> {
    let width = master.width() as usize;
    let height = master.height() as usize;
    let alpha: Vec<bool> = master.pixels().map(|pixel| pixel[3] > 8).collect();
    let mut min_x = width;
    let mut min_y = height;
    let mut max_x = 0usize;
    let mut max_y = 0usize;
    let mut opaque = 0usize;
    for y in 0..height {
        for x in 0..width {
            if alpha[y * width + x] {
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
                opaque += 1;
            }
        }
    }
    if opaque == 0 || max_y <= min_y + 2 || max_x <= min_x + 2 {
        return None;
    }
    let box_width = (max_x - min_x + 1) as f64;
    let box_height = (max_y - min_y + 1) as f64;
    let band = (box_height / 5.0).ceil() as usize;
    let top = band_shape(&alpha, width, min_x, max_x, min_y, min_y + band, box_width);
    let mid = band_shape(
        &alpha,
        width,
        min_x,
        max_x,
        min_y + 2 * band,
        min_y + 3 * band,
        box_width,
    );
    let lower_band_start = min_y + ((max_y - min_y) as f64 * 0.66) as usize;
    let lower = band_shape(
        &alpha,
        width,
        min_x,
        max_x,
        lower_band_start,
        max_y,
        box_width,
    );
    // Lower straddle: one run fully left of center, another fully right.
    let center = span_center(min_x, max_x);
    let mut left = false;
    let mut right = false;
    if lower.runs >= 2 {
        let span_alpha: Vec<bool> = (0..(max_x - min_x + 1))
            .map(|index| (lower_band_start..=max_y).any(|y| alpha[y * width + min_x + index]))
            .collect();
        let mut runs = Vec::new();
        let mut start: Option<usize> = None;
        for index in 0..span_alpha.len() {
            if span_alpha[index] && start.is_none() {
                start = Some(index);
            }
            if start.is_some() && (!span_alpha[index] || index + 1 == span_alpha.len()) {
                let begin = start.take().expect("run start");
                let end = if span_alpha[index] { index } else { index - 1 };
                if end - begin + 1 >= 2 {
                    runs.push((begin + min_x, end + min_x));
                }
            }
        }
        for (begin, end) in runs {
            if ((end as f64) + 0.5) < center {
                left = true;
            }
            if ((begin as f64) + 0.5) > center {
                right = true;
            }
        }
    }
    // Uniform thickness: per-column vertical extent stays near its mean.
    let mut thickness = Vec::new();
    for x in min_x..=max_x {
        let column: Vec<usize> = (min_y..=max_y).filter(|y| alpha[*y * width + x]).collect();
        if !column.is_empty() {
            thickness.push(column.len() as f64);
        }
    }
    let mean = thickness.iter().sum::<f64>() / thickness.len().max(1) as f64;
    let variance =
        thickness.iter().map(|t| (t - mean).powi(2)).sum::<f64>() / thickness.len().max(1) as f64;
    let uniform_thickness = mean > 0.0 && (variance.sqrt() / mean) < 0.55;
    let lower_runs = lower.runs;
    Some(ShapeFeatures {
        aspect: box_width / box_height,
        fill: opaque as f64 / (box_width * box_height),
        top,
        mid,
        lower,
        lower_straddle: left && right,
        lower_separated: lower_runs >= 2,
        uniform_thickness,
    })
}

fn span_center(min_x: usize, max_x: usize) -> f64 {
    (min_x as f64 + max_x as f64 + 1.0) / 2.0
}

fn scored(morphology: &str, mut score: f64, reasons: Vec<String>) -> MorphologyScore {
    let reasoning = if reasons.is_empty() {
        "generic silhouette match".to_string()
    } else {
        reasons.join("; ")
    };
    score = score.clamp(0.05, 0.97);
    MorphologyScore {
        morphology: morphology.to_string(),
        confidence: (score * 100.0).round() / 100.0,
        reasoning,
    }
}

/// Scores every rig profile against the sprite's silhouette so the app can
/// recommend (or reject) a rig before any points are placed.
pub(crate) fn detect_morphology(master: &RgbaImage) -> CommandResult<Vec<MorphologyScore>> {
    let Some(features) = shape_features(master) else {
        return Ok(vec![scored(
            "object",
            0.5,
            vec!["the sprite is too small or thin to profile".into()],
        )]);
    };
    let mut results = Vec::new();

    let mut biped = 0.2;
    let mut biped_reasons = vec!["upright proportions".to_string()];
    if features.aspect < 0.8 {
        biped += 0.35;
        biped_reasons.push("taller than wide".into());
    } else if features.aspect < 0.95 {
        biped += 0.2;
    }
    if features.lower_separated {
        biped += 0.2;
        biped_reasons.push("two separated legs".into());
    }
    if features.lower_straddle {
        biped += 0.1;
        biped_reasons.push("legs straddle the center line".into());
    }
    if features.top.max_run < features.mid.max_run * 0.85 {
        biped += 0.1;
        biped_reasons.push("head narrower than torso".into());
    }
    results.push(scored("biped", biped, biped_reasons));

    let mut quadruped = 0.15;
    let mut quad_reasons = Vec::new();
    if features.aspect > 1.2 {
        quadruped += 0.35;
        quad_reasons.push("longer than tall".into());
    } else if features.aspect > 1.05 {
        quadruped += 0.2;
    }
    if features.lower_straddle {
        quadruped += 0.2;
        quad_reasons.push("support mass on both ends".into());
    }
    if features.lower.runs >= 3 {
        quadruped += 0.15;
        quad_reasons.push("multiple legs visible".into());
    }
    if features.mid.covered > 0.6 {
        quadruped += 0.1;
        quad_reasons.push("continuous horizontal body mass".into());
    }
    results.push(scored("quadruped", quadruped, quad_reasons));

    let mut winged = 0.1;
    let mut wing_reasons = Vec::new();
    if features.aspect > 1.4 {
        winged += 0.35;
        wing_reasons.push("very wide silhouette".into());
    } else if features.aspect > 1.15 {
        winged += 0.15;
    }
    if features.top.max_run > features.mid.max_run * 1.1 {
        winged += 0.25;
        wing_reasons.push("upper mass wider than the torso — wings".into());
    }
    if features.top.covered > 0.5 && features.aspect > 1.2 {
        winged += 0.1;
    }
    results.push(scored("winged", winged, wing_reasons));

    let mut serpentine = 0.15;
    let mut snake_reasons = Vec::new();
    if features.aspect > 1.7 {
        serpentine += 0.4;
        snake_reasons.push("very elongated body".into());
    } else if features.aspect > 1.35 {
        serpentine += 0.2;
    }
    if features.uniform_thickness {
        serpentine += 0.2;
        snake_reasons.push("constant body thickness".into());
    }
    if features.fill < 0.45 {
        serpentine += 0.15;
        snake_reasons.push("thin mass for its bounding box".into());
    }
    if features.lower.runs <= 1 && features.mid.runs <= 1 {
        serpentine += 0.1;
        snake_reasons.push("no separated limbs".into());
    }
    results.push(scored("serpentine", serpentine, snake_reasons));

    let mut amorphous = 0.15;
    let mut amorph_reasons = Vec::new();
    if (0.8..=1.3).contains(&features.aspect) {
        amorphous += 0.3;
        amorph_reasons.push("roughly round silhouette".into());
    }
    if features.fill > 0.6 {
        amorphous += 0.25;
        amorph_reasons.push("solid filled mass".into());
    }
    if !features.lower_separated {
        amorphous += 0.15;
        amorph_reasons.push("no separated legs".into());
    }
    results.push(scored("amorphous", amorphous, amorph_reasons));

    let mut object = 0.2;
    let mut object_reasons = Vec::new();
    if (0.7..=1.45).contains(&features.aspect) {
        object += 0.2;
        object_reasons.push("compact proportions".into());
    }
    if !features.lower_separated {
        object += 0.15;
        object_reasons.push("single rigid body".into());
    }
    if features.fill > 0.45 {
        object += 0.1;
    }
    results.push(scored("object", object, object_reasons));

    results.sort_by(|left, right| {
        right
            .confidence
            .partial_cmp(&left.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    Ok(results)
}

/// Fraction of opaque pixels claimed by a capsule proper (not the nearest-bone
/// fallback) — how well the suggested bones tile the silhouette.
pub(crate) fn capsule_coverage(master: &RgbaImage, suggestion: &RigSuggestion) -> f64 {
    let mut positions = HashMap::new();
    for point in &suggestion.points {
        positions.insert(point.name.clone(), (point.x, point.y));
    }
    type Capsule = ((f64, f64), (f64, f64), f64);
    let capsules: Vec<Capsule> = suggestion
        .bones
        .iter()
        .filter_map(|bone| {
            let start = positions.get(&bone.start_point).copied()?;
            let end = positions.get(&bone.end_point).copied()?;
            Some((start, end, bone.radius))
        })
        .collect();
    if capsules.is_empty() {
        return 0.0;
    }
    let mut inside = 0usize;
    let mut total = 0usize;
    for (x, y, pixel) in master.enumerate_pixels() {
        if pixel[3] <= 8 {
            continue;
        }
        total += 1;
        let px = x as f64 + 0.5;
        let py = y as f64 + 0.5;
        if capsules.iter().any(|(start, end, radius)| {
            point_segment_distance(px, py, start.0, start.1, end.0, end.1) <= *radius
        }) {
            inside += 1;
        }
    }
    if total == 0 {
        0.0
    } else {
        inside as f64 / total as f64
    }
}
