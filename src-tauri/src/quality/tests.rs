use super::alignment::align_frame_to_canvas;
use super::interpolation::{interpolate_rgba, interpolation_neighbors, rebalance_motion_plan};
use super::metrics::{compute_metrics, pixel_difference, AnalyzedFrame, FrameMetrics};
use super::motion_checks::{
    leg_alternation_checks, limb_shading_checks, lower_body_view, LowerBodyView,
};
use crate::models::{MotionPhase, MotionPlan};
use image::{Rgba, RgbaImage};
use uuid::Uuid;

#[test]
fn pixel_difference_distinguishes_identical_and_changed_frames() {
    let first = RgbaImage::from_pixel(16, 16, Rgba([0, 0, 0, 0]));
    let mut second = first.clone();
    assert_eq!(pixel_difference(&first, &second), 0.0);
    second.put_pixel(8, 8, Rgba([255, 255, 255, 255]));
    assert!(pixel_difference(&first, &second) > 0.0);
}

#[test]
fn metrics_find_alpha_bounds_centroid_and_edges() {
    let directory = std::env::temp_dir().join(format!("sprite-quality-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&directory).expect("temp directory should create");
    let path = directory.join("frame.png");
    let mut image = RgbaImage::new(16, 16);
    for y in 5..10 {
        for x in 4..8 {
            image.put_pixel(x, y, Rgba([255, 100, 20, 255]));
        }
    }
    image.save(&path).expect("frame should save");
    let analyzed = compute_metrics("asset", path.to_str().expect("path should be utf8"))
        .expect("metrics should compute");
    assert_eq!(analyzed.metrics.bounds, Some((4, 5, 7, 9)));
    assert_eq!(analyzed.metrics.opaque_edge_pixels, 0);
    assert!(analyzed.metrics.alpha_coverage > 0.0);
    std::fs::remove_dir_all(directory).expect("temp directory should remove");
}

#[test]
fn alignment_preserves_pixels_when_bounds_are_missing() {
    let mut source = RgbaImage::new(4, 4);
    source.put_pixel(1, 2, Rgba([255, 0, 0, 255]));
    let repaired = align_frame_to_canvas(&source, None, 8, 8);
    assert_eq!(repaired.pixels().filter(|pixel| pixel[3] > 0).count(), 1);
}

#[test]
fn alignment_repair_bottom_centers_subject_without_losing_pixels() {
    let mut source = RgbaImage::new(6, 6);
    source.put_pixel(0, 1, Rgba([255, 0, 0, 255]));
    source.put_pixel(1, 2, Rgba([0, 255, 0, 255]));

    let repaired = align_frame_to_canvas(&source, Some((0, 1, 1, 2)), 8, 8);

    assert_eq!(repaired.get_pixel(3, 5), &Rgba([255, 0, 0, 255]));
    assert_eq!(repaired.get_pixel(4, 6), &Rgba([0, 255, 0, 255]));
    assert_eq!(repaired.pixels().filter(|pixel| pixel[3] > 0).count(), 2);
}

#[test]
fn interpolation_creates_a_true_midpoint_frame() {
    let first = RgbaImage::from_pixel(2, 2, Rgba([20, 40, 60, 0]));
    let second = RgbaImage::from_pixel(2, 2, Rgba([220, 140, 100, 200]));
    let transition = interpolate_rgba(&first, &second).expect("frames should interpolate");
    assert_eq!(transition.get_pixel(0, 0), &Rgba([120, 90, 80, 100]));
}

#[test]
fn fixed_loop_repairs_use_neighbors_without_changing_the_budget() {
    assert_eq!(interpolation_neighbors(0, 8, true), Some((7, 1)));
    assert_eq!(interpolation_neighbors(3, 8, true), Some((2, 4)));
    assert_eq!(interpolation_neighbors(7, 8, true), Some((6, 0)));
    assert_eq!(interpolation_neighbors(0, 8, false), None);
    assert_eq!(interpolation_neighbors(7, 8, false), None);
    assert_eq!(interpolation_neighbors(1, 2, true), None);
}

#[test]
fn optimization_rebalances_phase_counts_to_the_new_budget() {
    let plan = MotionPlan {
        frame_mode: "auto".into(),
        selected_frame_count: 6,
        minimum_frame_count: 4,
        maximum_frame_count: 10,
        fps: 12,
        looping: false,
        allow_interpolation: true,
        allow_auto_adjust: true,
        explanation: "Initial plan.".into(),
        phases: vec![
            MotionPhase {
                name: "Start".into(),
                description: "Start".into(),
                frame_count: 2,
                timing_weight: 0.8,
            },
            MotionPhase {
                name: "Impact".into(),
                description: "Impact".into(),
                frame_count: 2,
                timing_weight: 1.2,
            },
            MotionPhase {
                name: "Recovery".into(),
                description: "Recovery".into(),
                frame_count: 2,
                timing_weight: 1.0,
            },
        ],
    };
    let reduced = rebalance_motion_plan(plan.clone(), 5);
    let expanded = rebalance_motion_plan(plan, 8);
    assert_eq!(reduced.selected_frame_count, 5);
    assert_eq!(
        reduced
            .phases
            .iter()
            .map(|phase| phase.frame_count)
            .sum::<u32>(),
        5
    );
    assert_eq!(expanded.selected_frame_count, 8);
    assert_eq!(
        expanded
            .phases
            .iter()
            .map(|phase| phase.frame_count)
            .sum::<u32>(),
        8
    );
    assert!(expanded.phases[1].frame_count > 2);
}

fn walker_frame(left_leg: Rgba<u8>, right_leg: Rgba<u8>, merged: bool) -> AnalyzedFrame {
    let mut image = RgbaImage::new(16, 24);
    for y in 2..13usize {
        for x in 4..12usize {
            image.put_pixel(x as u32, y as u32, Rgba([180, 180, 180, 255]));
        }
    }
    let leg_columns: Vec<u32> = if merged {
        (5..12).collect()
    } else {
        (5..8).chain(9..12).collect()
    };
    for y in 13..23usize {
        for x in &leg_columns {
            let color = if *x < 8 { left_leg } else { right_leg };
            image.put_pixel(*x, y as u32, color);
        }
    }
    AnalyzedFrame {
        metrics: FrameMetrics {
            asset_id: Uuid::new_v4().to_string(),
            content_hash: String::new(),
            width: 16,
            height: 24,
            bounds: Some((4, 2, 11, 22)),
            centroid: None,
            alpha_coverage: 0.4,
            opaque_edge_pixels: 0,
            perceptual_hash: 0,
            palette: (0.0, 0.0, 0.0),
        },
        image,
    }
}

const NEAR_LEG: Rgba<u8> = Rgba([220, 220, 220, 255]);
const FAR_LEG: Rgba<u8> = Rgba([120, 120, 120, 255]);

#[test]
fn lower_limb_blobs_separate_two_legs_and_measure_shading() {
    let frame = walker_frame(FAR_LEG, NEAR_LEG, false);
    let blobs = match lower_body_view(&frame.image, frame.metrics.bounds) {
        LowerBodyView::TwoBlobs(blobs) => blobs,
        other => panic!("two legs should split, got {other:?}"),
    };
    assert_eq!(blobs.len(), 2);
    assert!(
        blobs[0].max_x < blobs[1].min_x,
        "blobs must be left-to-right"
    );
    let gap = (blobs[0].luminance - blobs[1].luminance).abs();
    assert!(
        (gap - 100.0).abs() < 1.0,
        "dark far leg vs light near leg should be ~100 apart, got {gap}"
    );
}

#[test]
fn passing_pose_legs_merge_into_one_blob_and_are_skipped() {
    let frame = walker_frame(FAR_LEG, NEAR_LEG, true);
    assert_eq!(
        lower_body_view(&frame.image, frame.metrics.bounds),
        LowerBodyView::Indistinct,
        "a merged passing pose must classify as indistinct"
    );
}

#[test]
fn shading_lock_break_flags_only_the_flat_frames() {
    let analyzed = vec![
        walker_frame(FAR_LEG, NEAR_LEG, false),
        walker_frame(NEAR_LEG, FAR_LEG, false),
        walker_frame(NEAR_LEG, NEAR_LEG, false),
        walker_frame(FAR_LEG, NEAR_LEG, false),
    ];
    let checks = limb_shading_checks(&analyzed);
    assert_eq!(checks.len(), 1, "only the shading-less frame flags");
    assert_eq!(checks[0].frame_index, Some(2));
    assert_eq!(checks[0].check_type, "limb_identity");
    assert!(checks[0].message.contains("far-limb shading lock"));
    assert_eq!(checks[0].repair_action, Some("regenerate"));
}

#[test]
fn animations_without_a_shading_lock_never_flag() {
    let analyzed = vec![
        walker_frame(NEAR_LEG, NEAR_LEG, false),
        walker_frame(NEAR_LEG, NEAR_LEG, false),
        walker_frame(NEAR_LEG, NEAR_LEG, false),
        walker_frame(NEAR_LEG, NEAR_LEG, false),
    ];
    assert!(
        limb_shading_checks(&analyzed).is_empty(),
        "no established lock means nothing to break"
    );
}

#[test]
fn hop_dominant_cycles_flag_as_lost_leg_alternation() {
    // One split contact pose, then the legs fuse for the rest of the
    // cycle — the exact shape of a failed AI run cycle.
    let analyzed = vec![
        walker_frame(FAR_LEG, NEAR_LEG, false),
        walker_frame(FAR_LEG, NEAR_LEG, true),
        walker_frame(FAR_LEG, NEAR_LEG, true),
        walker_frame(FAR_LEG, NEAR_LEG, true),
        walker_frame(FAR_LEG, NEAR_LEG, true),
        walker_frame(FAR_LEG, NEAR_LEG, true),
    ];
    let checks = leg_alternation_checks(&analyzed);
    assert_eq!(checks.len(), 1);
    assert_eq!(checks[0].check_type, "leg_separation");
    assert_eq!(checks[0].severity, "error", "5/6 fused frames is severe");
    assert!(checks[0].message.contains("plays as a hop"));
    assert!(checks[0].message.contains("split stances"));
    assert_eq!(checks[0].frame_index, None, "the whole cycle is flagged");
    assert_eq!(checks[0].repair_action, Some("regenerate"));
}

#[test]
fn alternating_run_cycles_with_gathered_flights_stay_clean() {
    let analyzed = vec![
        walker_frame(FAR_LEG, NEAR_LEG, false),
        walker_frame(FAR_LEG, NEAR_LEG, false),
        walker_frame(FAR_LEG, NEAR_LEG, true),
        walker_frame(FAR_LEG, NEAR_LEG, false),
        walker_frame(FAR_LEG, NEAR_LEG, false),
        walker_frame(FAR_LEG, NEAR_LEG, true),
        walker_frame(FAR_LEG, NEAR_LEG, false),
        walker_frame(FAR_LEG, NEAR_LEG, false),
    ];
    assert!(
        leg_alternation_checks(&analyzed).is_empty(),
        "two gathered flight frames in eight is a healthy run"
    );
}

#[test]
fn idle_stances_with_always_together_legs_never_flag() {
    let analyzed = (0..6)
        .map(|_| walker_frame(NEAR_LEG, NEAR_LEG, true))
        .collect::<Vec<_>>();
    assert!(
        leg_alternation_checks(&analyzed).is_empty(),
        "no separated-leg frame means no proof of leg alternation to lose"
    );
}
