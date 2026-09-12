use crate::{
    error::{CommandError, CommandResult},
    models::{ProceduralVfxInput, VfxEffect},
};
use image::{Rgba, RgbaImage};

use super::vfx_abilities::{
    draw_frost_lance, draw_nova_beam, draw_storm_lance, draw_voltaic_snare,
};

pub(super) fn vfx_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<VfxEffect> {
    Ok(VfxEffect {
        id: row.get(0)?,
        project_id: row.get(1)?,
        worktree_id: row.get(2)?,
        animation_id: row.get(3)?,
        name: row.get(4)?,
        effect_type: row.get(5)?,
        blend_mode: row.get(6)?,
        center_x: row.get(7)?,
        center_y: row.get(8)?,
        opacity: row.get(9)?,
        looping: row.get(10)?,
        fps: row.get(11)?,
        created_at: row.get(12)?,
        updated_at: row.get(13)?,
    })
}

pub(super) fn select_vfx() -> &'static str {
    r#"SELECT id, project_id, worktree_id, animation_id, name, effect_type,
              blend_mode, center_x, center_y, opacity, looping, fps, created_at, updated_at
       FROM vfx_effects"#
}

pub(super) fn validate_vfx_input(input: &ProceduralVfxInput) -> CommandResult<()> {
    if input.name.trim().is_empty() {
        return Err(CommandError::new(
            "invalid_vfx_name",
            "Effect name cannot be empty",
        ));
    }
    if !matches!(
        input.effect_type.as_str(),
        "fire"
            | "explosion"
            | "magic"
            | "slash"
            | "smoke"
            | "frost_lance"
            | "storm_lance"
            | "nova_beam"
            | "voltaic_snare"
    ) {
        return Err(CommandError::new(
            "invalid_vfx_type",
            "Choose a base effect or one of the experimental ability effects",
        ));
    }
    if !matches!(
        input.blend_mode.as_str(),
        "normal" | "add" | "screen" | "multiply"
    ) {
        return Err(CommandError::new(
            "invalid_blend_mode",
            "Choose Normal, Add, Screen, or Multiply blending",
        ));
    }
    if !(8..=1024).contains(&input.width) || !(8..=1024).contains(&input.height) {
        return Err(CommandError::new(
            "invalid_vfx_size",
            "VFX dimensions must be between 8 and 1024 pixels",
        ));
    }
    if !(2..=64).contains(&input.frames) {
        return Err(CommandError::new(
            "invalid_vfx_frames",
            "Procedural VFX must contain between 2 and 64 frames",
        ));
    }
    if !(1..=60).contains(&input.fps) {
        return Err(CommandError::new(
            "invalid_vfx_fps",
            "VFX playback must be between 1 and 60 FPS",
        ));
    }
    Ok(())
}

fn blend_pixel(image: &mut RgbaImage, x: i32, y: i32, color: Rgba<u8>) {
    if x < 0 || y < 0 || x >= image.width() as i32 || y >= image.height() as i32 {
        return;
    }
    let destination = image.get_pixel_mut(x as u32, y as u32);
    let source_alpha = color[3] as f32 / 255.0;
    let destination_alpha = destination[3] as f32 / 255.0;
    let output_alpha = source_alpha + destination_alpha * (1.0 - source_alpha);
    if output_alpha <= f32::EPSILON {
        return;
    }
    for channel in 0..3 {
        let value = (color[channel] as f32 * source_alpha
            + destination[channel] as f32 * destination_alpha * (1.0 - source_alpha))
            / output_alpha;
        destination[channel] = value.round().clamp(0.0, 255.0) as u8;
    }
    destination[3] = (output_alpha * 255.0).round().clamp(0.0, 255.0) as u8;
}

pub(super) fn draw_disc(
    image: &mut RgbaImage,
    center_x: f64,
    center_y: f64,
    radius: f64,
    color: Rgba<u8>,
) {
    let minimum_x = (center_x - radius).floor() as i32;
    let maximum_x = (center_x + radius).ceil() as i32;
    let minimum_y = (center_y - radius).floor() as i32;
    let maximum_y = (center_y + radius).ceil() as i32;
    for y in minimum_y..=maximum_y {
        for x in minimum_x..=maximum_x {
            let distance = ((x as f64 - center_x).powi(2) + (y as f64 - center_y).powi(2)).sqrt();
            if distance <= radius {
                let edge = ((radius - distance) / radius.max(1.0)).clamp(0.0, 1.0);
                let mut shaded = color;
                shaded[3] = (color[3] as f64 * edge.sqrt()).round() as u8;
                blend_pixel(image, x, y, shaded);
            }
        }
    }
}

pub(super) fn draw_ring(
    image: &mut RgbaImage,
    center_x: f64,
    center_y: f64,
    radius: f64,
    thickness: f64,
    color: Rgba<u8>,
) {
    let outer = radius + thickness;
    let minimum_x = (center_x - outer).floor() as i32;
    let maximum_x = (center_x + outer).ceil() as i32;
    let minimum_y = (center_y - outer).floor() as i32;
    let maximum_y = (center_y + outer).ceil() as i32;
    for y in minimum_y..=maximum_y {
        for x in minimum_x..=maximum_x {
            let distance = ((x as f64 - center_x).powi(2) + (y as f64 - center_y).powi(2)).sqrt();
            let delta = (distance - radius).abs();
            if delta <= thickness {
                let mut shaded = color;
                shaded[3] = (color[3] as f64 * (1.0 - delta / thickness.max(0.5))).round() as u8;
                blend_pixel(image, x, y, shaded);
            }
        }
    }
}

/// A soft, layered sprite stroke. It lets the procedural experiments retain a
/// readable direction of force at tiny game resolutions.
pub(super) fn draw_stroke(
    image: &mut RgbaImage,
    from_x: f64,
    from_y: f64,
    to_x: f64,
    to_y: f64,
    radius: f64,
    color: Rgba<u8>,
) {
    let distance = (to_x - from_x).hypot(to_y - from_y);
    let steps = distance.ceil().max(1.0) as u32;
    for step in 0..=steps {
        let u = step as f64 / steps as f64;
        draw_disc(
            image,
            from_x + (to_x - from_x) * u,
            from_y + (to_y - from_y) * u,
            radius,
            color,
        );
    }
}

pub(super) fn pseudo(seed: u64, value: u64) -> f64 {
    let mut number = seed ^ value.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    number ^= number >> 30;
    number = number.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    number ^= number >> 27;
    number = number.wrapping_mul(0x94D0_49BB_1331_11EB);
    ((number ^ (number >> 31)) as f64) / u64::MAX as f64
}
pub(super) fn render_vfx_frame(input: &ProceduralVfxInput, frame_index: u32) -> RgbaImage {
    let mut image = RgbaImage::new(input.width, input.height);
    let denominator = if input.looping {
        input.frames as f64
    } else {
        input.frames.saturating_sub(1).max(1) as f64
    };
    let t = frame_index as f64 / denominator;
    let width = input.width as f64;
    let height = input.height as f64;
    let center_x = width * 0.5;
    let center_y = height * 0.54;
    match input.effect_type.as_str() {
        "frost_lance" => draw_frost_lance(&mut image, input.seed, t, width, height),
        "storm_lance" => draw_storm_lance(&mut image, input.seed, t, width, height),
        "nova_beam" => draw_nova_beam(&mut image, t, width, height),
        "voltaic_snare" => draw_voltaic_snare(&mut image, t, width, height, center_x),
        "explosion" => {
            let envelope = (std::f64::consts::PI * t).sin().max(0.0);
            let radius = width.min(height) * (0.08 + 0.34 * t);
            draw_disc(
                &mut image,
                center_x,
                center_y,
                radius,
                Rgba([255, 91, 27, (210.0 * envelope) as u8]),
            );
            draw_disc(
                &mut image,
                center_x,
                center_y,
                radius * 0.58,
                Rgba([255, 211, 73, (245.0 * envelope) as u8]),
            );
            draw_ring(
                &mut image,
                center_x,
                center_y,
                radius * 1.12,
                1.4,
                Rgba([255, 237, 154, (230.0 * (1.0 - t)) as u8]),
            );
            for particle in 0..10 {
                let angle = pseudo(input.seed, particle) * std::f64::consts::TAU;
                let distance = radius * (0.7 + pseudo(input.seed + 11, particle) * 0.9);
                draw_disc(
                    &mut image,
                    center_x + angle.cos() * distance,
                    center_y + angle.sin() * distance,
                    1.0 + 2.0 * (1.0 - t),
                    Rgba([255, 137, 35, (220.0 * envelope) as u8]),
                );
            }
        }
        "magic" => {
            let pulse = 0.5 + 0.5 * (std::f64::consts::TAU * t).sin();
            let radius = width.min(height) * (0.20 + 0.08 * pulse);
            draw_disc(
                &mut image,
                center_x,
                center_y,
                radius * 0.45,
                Rgba([110, 67, 255, 110]),
            );
            draw_ring(
                &mut image,
                center_x,
                center_y,
                radius,
                1.6,
                Rgba([121, 238, 255, 240]),
            );
            draw_ring(
                &mut image,
                center_x,
                center_y,
                radius * 0.72,
                1.0,
                Rgba([202, 126, 255, 210]),
            );
            for spark in 0..8 {
                let angle = std::f64::consts::TAU * (spark as f64 / 8.0 + t);
                draw_disc(
                    &mut image,
                    center_x + angle.cos() * radius * 1.25,
                    center_y + angle.sin() * radius * 1.25,
                    1.2,
                    Rgba([229, 251, 255, 230]),
                );
            }
            let marker_angle = std::f64::consts::TAU * t + 0.33;
            draw_disc(
                &mut image,
                center_x + marker_angle.cos() * radius * 1.55,
                center_y + marker_angle.sin() * radius * 1.55,
                1.0,
                Rgba([255, 255, 255, 255]),
            );
        }
        "slash" => {
            let radius = width.min(height) * 0.34;
            let head = -2.2 + 4.0 * t;
            for segment in 0..24 {
                let age = segment as f64 / 23.0;
                let angle = head - age * 1.25;
                let alpha =
                    (255.0 * (1.0 - age) * (std::f64::consts::PI * t).sin().max(0.15)) as u8;
                let x = center_x + angle.cos() * radius;
                let y = center_y + angle.sin() * radius * 0.72;
                draw_disc(
                    &mut image,
                    x,
                    y,
                    1.2 + 2.4 * (1.0 - age),
                    Rgba([173, 244, 255, alpha]),
                );
            }
            draw_disc(
                &mut image,
                center_x,
                center_y,
                2.0,
                Rgba([255, 255, 255, 120]),
            );
        }
        "smoke" => {
            let fade = (1.0 - t).max(0.0);
            for cloud in 0..7 {
                let phase = (t + pseudo(input.seed, cloud)) % 1.0;
                let x = center_x + (pseudo(input.seed + 17, cloud) - 0.5) * width * 0.25;
                let y = height * 0.78 - phase * height * 0.48;
                let radius = width.min(height) * (0.05 + phase * 0.10);
                draw_disc(
                    &mut image,
                    x,
                    y,
                    radius,
                    Rgba([157, 169, 183, (150.0 * fade.max(0.3)) as u8]),
                );
            }
        }
        _ => {
            for flame in 0..8 {
                let phase = (t + pseudo(input.seed, flame)) % 1.0;
                let x = center_x + (pseudo(input.seed + 31, flame) - 0.5) * width * 0.28;
                let y = height * 0.78 - phase * height * 0.42;
                let radius = width.min(height) * (0.035 + (1.0 - phase) * 0.065);
                draw_disc(
                    &mut image,
                    x,
                    y,
                    radius * 1.25,
                    Rgba([255, 72, 18, (210.0 * (1.0 - phase)) as u8]),
                );
                draw_disc(
                    &mut image,
                    x,
                    y + radius * 0.2,
                    radius * 0.7,
                    Rgba([255, 221, 69, (240.0 * (1.0 - phase)) as u8]),
                );
            }
        }
    }
    image
}
