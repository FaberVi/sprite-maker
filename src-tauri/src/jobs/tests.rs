use super::spritesheet::{frame_offset, layout_dimensions};
use super::vfx_draw::render_vfx_frame;
use crate::models::ProceduralVfxInput;

#[test]
fn computes_horizontal_vertical_and_grid_layouts() {
    assert_eq!(layout_dimensions("horizontal", 12, 4), (12, 1));
    assert_eq!(layout_dimensions("vertical", 12, 4), (1, 12));
    assert_eq!(layout_dimensions("grid", 12, 4), (4, 3));
    assert_eq!(layout_dimensions("grid", 10, 4), (4, 3));
}

#[test]
fn aligns_frames_without_losing_ground_contact() {
    assert_eq!(frame_offset("top_left", 64, 64, 32, 40), (0, 0));
    assert_eq!(frame_offset("center", 64, 64, 32, 40), (16, 12));
    assert_eq!(frame_offset("bottom_center", 64, 64, 32, 40), (16, 24));
}

#[test]
fn procedural_vfx_frames_are_distinct_and_transparent() {
    for effect_type in [
        "magic",
        "frost_lance",
        "storm_lance",
        "nova_beam",
        "voltaic_snare",
    ] {
        let input = ProceduralVfxInput {
            project_id: "project".into(),
            worktree_id: "vfx".into(),
            name: "Effect test".into(),
            effect_type: effect_type.into(),
            blend_mode: "screen".into(),
            width: 64,
            height: 64,
            frames: 12,
            fps: 12,
            looping: effect_type == "magic",
            seed: 42,
        };
        let frames: Vec<_> = (0..input.frames)
            .map(|index| render_vfx_frame(&input, index))
            .collect();
        let hashes: std::collections::HashSet<_> = frames
            .iter()
            .map(|frame| blake3::hash(frame.as_raw()))
            .collect();
        assert_eq!(
            hashes.len(),
            frames.len(),
            "{effect_type} frames should animate"
        );
        assert!(frames
            .iter()
            .all(|frame| frame.pixels().any(|pixel| pixel[3] == 0)));
        assert!(frames
            .iter()
            .all(|frame| frame.pixels().any(|pixel| pixel[3] > 0)));
    }
}
