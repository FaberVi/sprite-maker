use crate::{
    error::{CommandError, CommandResult},
    models::{GenerationOptions, MotionPhase, MotionPlan},
};

use super::classify::{classify_motion, PhaseDefinition};

fn explicit_frame_count(prompt: &str) -> Option<u32> {
    let words: Vec<_> = prompt
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .collect();
    words.windows(2).find_map(|pair| {
        (pair[1].eq_ignore_ascii_case("frame") || pair[1].eq_ignore_ascii_case("frames"))
            .then(|| pair[0].parse().ok())
            .flatten()
    })
}

fn allocate_phases(definitions: Vec<PhaseDefinition>, frame_count: u32) -> Vec<MotionPhase> {
    if frame_count == 0 {
        return Vec::new();
    }
    let desired = frame_count as usize;
    let selected = if definitions.len() <= desired {
        definitions
    } else if desired == 1 {
        vec![definitions[definitions.len() / 2]]
    } else {
        (0..desired)
            .map(|index| {
                let source = index * (definitions.len() - 1) / (desired - 1);
                definitions[source]
            })
            .collect()
    };
    let mut phases: Vec<_> = selected
        .iter()
        .map(|definition| MotionPhase {
            name: definition.name.into(),
            description: definition.description.into(),
            frame_count: 1,
            timing_weight: definition.weight,
        })
        .collect();
    let mut remaining = desired.saturating_sub(phases.len());
    let mut order: Vec<_> = (0..phases.len()).collect();
    order.sort_by(|left, right| {
        phases[*right]
            .timing_weight
            .total_cmp(&phases[*left].timing_weight)
    });
    let mut index = 0;
    while remaining > 0 {
        phases[order[index % order.len()]].frame_count += 1;
        remaining -= 1;
        index += 1;
    }
    phases
}

pub fn build_motion_plan(
    prompt: &str,
    generation: &GenerationOptions,
) -> CommandResult<MotionPlan> {
    if !matches!(generation.frame_mode.as_str(), "fixed" | "auto") {
        return Err(CommandError::new(
            "invalid_frame_mode",
            "Frame mode must be Fixed or Auto",
        ));
    }
    let minimum = generation.min_frames.clamp(1, 64);
    let maximum = generation.max_frames.clamp(minimum, 64);
    let (motion_kind, recommended, definitions, inferred_loop, limb_identity) =
        classify_motion(prompt);
    let explicit = explicit_frame_count(prompt).filter(|count| (1..=64).contains(count));
    let single_frame = prompt.trim_start().starts_with("/sprite")
        || prompt.to_ascii_lowercase().contains("single frame")
        || prompt.to_ascii_lowercase().contains("one frame");
    let selected = if single_frame {
        1
    } else if let Some(explicit) = explicit {
        explicit
    } else if generation.frame_mode == "fixed" {
        generation.frames.clamp(1, 64)
    } else {
        recommended.clamp(minimum, maximum)
    };
    let selected = if prompt.trim_start().starts_with("/animate") && !single_frame {
        selected.max(2)
    } else {
        selected
    };
    let effective_mode = if explicit.is_some() || single_frame {
        "fixed"
    } else {
        generation.frame_mode.as_str()
    };
    let explanation = if single_frame {
        "A single frame was selected because the request is for a static sprite.".into()
    } else if explicit.is_some() || generation.frame_mode == "fixed" {
        format!(
            "Exactly {selected} frames will be used and the motion phases will be fitted to that limit."
        )
    } else {
        format!(
            "{selected} frames selected within the {minimum}–{maximum} limit because the request describes {motion_kind}."
        )
    };
    let explanation = if limb_identity {
        format!(
            "{explanation} Paired limbs are named by camera depth (NEAR limb and FAR limb), never by screen side; the FAR limb keeps a darker shade so the legs never swap identity."
        )
    } else {
        explanation
    };
    Ok(MotionPlan {
        frame_mode: effective_mode.into(),
        selected_frame_count: selected,
        minimum_frame_count: if effective_mode == "auto" {
            minimum
        } else {
            selected
        },
        maximum_frame_count: if effective_mode == "auto" {
            maximum
        } else {
            selected
        },
        fps: generation.fps,
        looping: inferred_loop,
        allow_interpolation: generation.allow_interpolation,
        allow_auto_adjust: generation.allow_auto_adjust,
        explanation,
        phases: allocate_phases(definitions, selected),
    })
}
