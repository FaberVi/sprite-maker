use crate::error::{CommandError, CommandResult};
use std::collections::HashMap;

use super::types::{Rig, RigTransform};

pub fn validate_rig(rig: &Rig, width: u32, height: u32) -> CommandResult<Vec<String>> {
    let mut warnings = Vec::new();
    let mut names = std::collections::HashSet::new();
    for point in &rig.points {
        if !names.insert(point.name.clone()) {
            warnings.push(format!("Point `{}` is defined more than once", point.name));
        }
        if point.x < 0.0 || point.y < 0.0 || point.x >= width as f64 || point.y >= height as f64 {
            warnings.push(format!(
                "Point `{}` sits outside the {}×{} canvas",
                point.name, width, height
            ));
        }
    }
    let mut seen_bones = Vec::new();
    for bone in &rig.bones {
        if seen_bones.contains(&bone.name) {
            if !warnings
                .iter()
                .any(|warning| warning.contains(&format!("Bone `{}` is defined", bone.name)))
            {
                warnings.push(format!("Bone `{}` is defined more than once", bone.name));
            }
        } else {
            seen_bones.push(bone.name.clone());
        }
        if !names.contains(&bone.start_point) || !names.contains(&bone.end_point) {
            warnings.push(format!(
                "Bone `{}` references a point that does not exist",
                bone.name
            ));
        }
        if !(0.5..=64.0).contains(&bone.radius) {
            warnings.push(format!(
                "Bone `{}` radius {} is outside 0.5–64 px",
                bone.name, bone.radius
            ));
        }
        if let Some(parent) = bone.parent.as_deref() {
            if parent == bone.name {
                warnings.push(format!("Bone `{}` parents itself", bone.name));
            } else if !seen_bones.iter().any(|name| name == parent) {
                warnings.push(format!(
                    "Bone `{}` parents missing bone `{}`",
                    bone.name, parent
                ));
            }
        }
    }
    // Cycle detection over the parent graph.
    for bone in &rig.bones {
        let mut visited = std::collections::HashSet::new();
        let mut current = Some(bone.name.clone());
        while let Some(name) = current {
            if !visited.insert(name.clone()) {
                warnings.push(format!("Bone `{}` is part of a parent cycle", bone.name));
                break;
            }
            current = rig
                .bones
                .iter()
                .find(|candidate| candidate.name == name)
                .and_then(|candidate| candidate.parent.clone());
        }
    }
    for frame in &rig.frames {
        for transform in &frame.transforms {
            if !seen_bones.contains(&transform.bone) {
                warnings.push(format!(
                    "A frame transform targets missing bone `{}`",
                    transform.bone
                ));
            }
        }
        for contact in &frame.contacts {
            if !seen_bones.contains(&contact.bone) {
                warnings.push(format!(
                    "A frame contact targets missing bone `{}`",
                    contact.bone
                ));
            }
        }
    }
    if rig.bones.is_empty() {
        warnings.push("The rig has no bones yet — add at least one before rendering".into());
    }
    Ok(warnings)
}

fn perceptibly_animated_bones(rig: &Rig) -> std::collections::HashSet<String> {
    let mut samples: HashMap<String, Vec<&RigTransform>> = HashMap::new();
    for frame in &rig.frames {
        for transform in &frame.transforms {
            samples
                .entry(transform.bone.clone())
                .or_default()
                .push(transform);
        }
    }
    let mut meaningful: std::collections::HashSet<String> = samples
        .into_iter()
        .filter_map(|(bone, values)| {
            let span = |read: fn(&RigTransform) -> f64| {
                let mut minimum = 0.0_f64;
                let mut maximum = 0.0_f64;
                for value in &values {
                    let sample = read(value);
                    minimum = minimum.min(sample);
                    maximum = maximum.max(sample);
                }
                maximum - minimum
            };
            let translation = span(|value| value.dx).hypot(span(|value| value.dy));
            let rotation = span(|value| value.rotate);
            let scale = span(|value| value.scale_x - 1.0).max(span(|value| value.scale_y - 1.0));
            (translation >= 1.5 || rotation >= 8.0 || scale >= 0.05).then_some(bone)
        })
        .collect();
    let mut contact_samples: HashMap<String, Vec<(f64, f64)>> = HashMap::new();
    for frame in &rig.frames {
        for contact in &frame.contacts {
            contact_samples
                .entry(contact.bone.clone())
                .or_default()
                .push((contact.x, contact.y));
        }
    }
    for (bone, values) in contact_samples {
        if values.len() < 2 {
            continue;
        }
        let min_x = values
            .iter()
            .map(|value| value.0)
            .fold(f64::INFINITY, f64::min);
        let max_x = values
            .iter()
            .map(|value| value.0)
            .fold(f64::NEG_INFINITY, f64::max);
        let min_y = values
            .iter()
            .map(|value| value.1)
            .fold(f64::INFINITY, f64::min);
        let max_y = values
            .iter()
            .map(|value| value.1)
            .fold(f64::NEG_INFINITY, f64::max);
        if (max_x - min_x).hypot(max_y - min_y) >= 1.5 {
            meaningful.insert(bone);
        }
    }
    meaningful
}

fn frames_share_pose(left: &super::types::RigFrame, right: &super::types::RigFrame) -> bool {
    if (left.root_dx - right.root_dx).abs() > 0.01
        || (left.root_dy - right.root_dy).abs() > 0.01
    {
        return false;
    }
    if left.transforms.len() != right.transforms.len() {
        return false;
    }
    left.transforms.iter().all(|left_transform| {
        right.transforms.iter().any(|right_transform| {
            left_transform.bone == right_transform.bone
                && (left_transform.dx - right_transform.dx).abs() <= 0.01
                && (left_transform.dy - right_transform.dy).abs() <= 0.01
                && (left_transform.rotate - right_transform.rotate).abs() <= 0.01
                && (left_transform.scale_x - right_transform.scale_x).abs() <= 0.01
                && (left_transform.scale_y - right_transform.scale_y).abs() <= 0.01
        })
    })
}

pub(super) fn validate_rig_loop_motion(rig: &Rig) -> CommandResult<()> {
    if rig.frames.len() < 2 {
        return Ok(());
    }
    for index in 0..rig.frames.len() - 1 {
        if !rig.frames[index].hold && frames_share_pose(&rig.frames[index], &rig.frames[index + 1]) {
            return Err(CommandError::new(
                "duplicate_rig_frames",
                format!(
                    "Pose frames {} and {} are identical. Add motion or mark the hold frame explicitly.",
                    index + 1,
                    index + 2
                ),
            ));
        }
    }
    if rig.looping {
        let first = &rig.frames[0];
        let last = &rig.frames[rig.frames.len() - 1];
        if frames_share_pose(first, last) && rig.frames.len() > 2 {
            return Err(CommandError::new(
                "static_rig_loop",
                "The looping animation ends on the same pose as the first frame without a distinct transition.",
            ));
        }
        let drift = (last.root_dx - first.root_dx)
            .hypot(last.root_dy - first.root_dy);
        let canvas_limit = 8.0;
        if drift > canvas_limit {
            return Err(CommandError::new(
                "root_loop_drift",
                format!(
                    "Root motion drifts {:.1}px between the last and first frame. Keep in-place loops under {:.0}px.",
                    drift,
                    canvas_limit
                ),
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_perceptible_rig_motion(rig: &Rig) -> CommandResult<()> {
    if rig.frames.len() < 2 {
        return Err(CommandError::new(
            "static_rig",
            "Add at least two different pose frames before rendering an animation",
        ));
    }
    let meaningful = perceptibly_animated_bones(rig);
    let required = if matches!(rig.morphology.as_str(), "biped" | "quadruped" | "winged") {
        2
    } else {
        1
    };
    if meaningful.len() < required {
        return Err(CommandError::new(
            "imperceptible_rig_motion",
            format!(
                "The pose frames are effectively static at sprite resolution. Animate at least {required} bone{} across 8°, 1.5 px, or 5% scale, then preview again.",
                if required == 1 { "" } else { "s" }
            ),
        ));
    }
    let body_tokens: &[&str] = match rig.morphology.as_str() {
        "biped" => &["body", "torso", "pelvis", "spine", "chest", "hip"],
        "quadruped" => &["body", "torso", "pelvis", "spine", "chest"],
        "winged" => &["body", "torso", "spine", "chest"],
        "serpentine" => &["body", "segment", "spine", "head", "tail"],
        _ => &[],
    };
    if !body_tokens.is_empty() {
        let core_bones: std::collections::HashSet<&str> = rig
            .bones
            .iter()
            .filter(|bone| {
                let tokens: std::collections::HashSet<&str> = bone
                    .name
                    .split(|character: char| !character.is_ascii_alphanumeric())
                    .filter(|token| !token.is_empty())
                    .collect();
                body_tokens.iter().any(|token| tokens.contains(token))
            })
            .map(|bone| bone.name.as_str())
            .collect();
        let required_core = if rig.morphology == "serpentine" { 2 } else { 1 };
        let animated_core = meaningful
            .iter()
            .filter(|bone| core_bones.contains(bone.as_str()))
            .count();
        if core_bones.len() < required_core || animated_core < required_core {
            return Err(CommandError::new(
                "missing_body_motion",
                format!(
                    "The limbs move, but the {} body stays rigid. Add and animate {} torso, pelvis, spine, or body bone{} with visible compression, rotation, or counter-motion.",
                    rig.morphology,
                    required_core,
                    if required_core == 1 { "" } else { "s" }
                ),
            ));
        }
    }
    validate_rig_loop_motion(rig)?;
    Ok(())
}
