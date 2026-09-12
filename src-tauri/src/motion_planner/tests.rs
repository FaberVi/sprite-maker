use super::build_motion_plan;
use crate::models::GenerationOptions;

fn options(mode: &str, frames: u32, minimum: u32, maximum: u32) -> GenerationOptions {
    GenerationOptions {
        quality: "custom".into(),
        width: 64,
        height: 64,
        frames,
        fps: 12,
        frame_mode: mode.into(),
        min_frames: minimum,
        max_frames: maximum,
        allow_interpolation: false,
        allow_auto_adjust: mode == "auto",
    }
}

#[test]
fn auto_mode_uses_motion_complexity_inside_user_limits() {
    let plan = build_motion_plan(
        "a heavy spinning greatsword combo",
        &options("auto", 6, 5, 14),
    )
    .expect("plan should build");
    assert_eq!(plan.selected_frame_count, 12);
    assert_eq!(
        plan.phases
            .iter()
            .map(|phase| phase.frame_count)
            .sum::<u32>(),
        12
    );
    assert!(plan.explanation.contains("multi-stage heavy action"));
}

#[test]
fn fixed_mode_respects_the_exact_count_and_fits_phases() {
    let plan = build_motion_plan("quick sword slash", &options("fixed", 4, 4, 12))
        .expect("plan should build");
    assert_eq!(plan.selected_frame_count, 4);
    assert_eq!(plan.phases.len(), 4);
    assert_eq!(plan.minimum_frame_count, 4);
    assert_eq!(plan.maximum_frame_count, 4);
}

#[test]
fn explicit_prompt_count_overrides_auto_mode() {
    let plan = build_motion_plan(
        "open the chest using exactly 10 frames",
        &options("auto", 6, 4, 8),
    )
    .expect("plan should build");
    assert_eq!(plan.frame_mode, "fixed");
    assert_eq!(plan.selected_frame_count, 10);
}

#[test]
fn dynamic_motion_matrix_uses_the_smallest_readable_budget() {
    let cases = [
        ("idle breathing", 5, true),
        ("walk cycle", 8, true),
        ("run cycle", 8, true),
        ("sword attack", 7, false),
        ("heavy greatsword attack", 12, false),
        ("combat dodge", 6, false),
        ("spell cast", 9, false),
        ("character death", 10, false),
    ];
    for (prompt, expected_frames, expected_loop) in cases {
        let plan = build_motion_plan(prompt, &options("auto", 6, 4, 16))
            .unwrap_or_else(|error| panic!("{prompt} should plan: {}", error.message));
        assert_eq!(plan.selected_frame_count, expected_frames, "{prompt}");
        assert_eq!(plan.looping, expected_loop, "{prompt}");
        assert_eq!(
            plan.phases
                .iter()
                .map(|phase| phase.frame_count)
                .sum::<u32>(),
            expected_frames,
            "{prompt}"
        );
    }
}

#[test]
fn locomotion_phases_name_limbs_by_camera_depth_not_screen_side() {
    for prompt in ["run cycle", "walk cycle"] {
        let plan = build_motion_plan(prompt, &options("auto", 6, 4, 16))
            .unwrap_or_else(|error| panic!("{prompt} should plan: {}", error.message));
        let text = plan
            .phases
            .iter()
            .map(|phase| format!("{} {}", phase.name, phase.description))
            .collect::<Vec<_>>()
            .join(" ");
        assert!(
            text.contains("NEAR leg") && text.contains("FAR leg"),
            "{prompt} phases must name NEAR and FAR legs: {text}"
        );
        assert!(
            !text.to_lowercase().contains("left foot")
                && !text.to_lowercase().contains("right foot"),
            "{prompt} phases must not use screen-side leg names: {text}"
        );
        assert!(plan.explanation.contains("NEAR limb and FAR limb"));
    }
    // Non-locomotion motions keep their phase vocabulary untouched.
    let effect = build_motion_plan("explosion vfx loop", &options("auto", 6, 4, 16))
        .expect("effect plan should build");
    assert!(!effect.explanation.contains("NEAR limb"));
}

#[test]
fn run_cycle_alternates_leg_contacts_and_keeps_flight_symmetric() {
    let plan =
        build_motion_plan("run cycle", &options("auto", 6, 4, 16)).expect("run plan should build");
    let names: Vec<&str> = plan
        .phases
        .iter()
        .map(|phase| phase.name.as_str())
        .collect();
    let near_contacts = names.iter().filter(|name| **name == "Near contact").count();
    let far_contacts = names.iter().filter(|name| **name == "Far contact").count();
    assert_eq!(near_contacts, 1, "one NEAR contact per cycle: {names:?}");
    assert_eq!(far_contacts, 1, "one FAR contact per cycle: {names:?}");
    let flights = names
        .iter()
        .filter(|name| name.starts_with("Flight"))
        .count();
    assert_eq!(flights, 2, "a run cycle has two flight phases: {names:?}");
}

#[test]
fn fixed_and_dynamic_sword_attacks_keep_distinct_contracts() {
    let fixed = build_motion_plan("sword attack", &options("fixed", 6, 4, 16))
        .expect("fixed plan should build");
    let dynamic = build_motion_plan("sword attack", &options("auto", 6, 4, 16))
        .expect("dynamic plan should build");
    assert_eq!(fixed.selected_frame_count, 6);
    assert_eq!(fixed.minimum_frame_count, 6);
    assert_eq!(fixed.maximum_frame_count, 6);
    assert!(!fixed.allow_auto_adjust);
    assert_eq!(dynamic.selected_frame_count, 7);
    assert_eq!(dynamic.minimum_frame_count, 4);
    assert_eq!(dynamic.maximum_frame_count, 16);
    assert!(dynamic.allow_auto_adjust);
}
