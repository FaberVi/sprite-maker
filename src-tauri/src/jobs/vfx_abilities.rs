use image::{Rgba, RgbaImage};

use super::vfx_draw::{draw_disc, draw_ring, draw_stroke, pseudo};

pub(super) fn draw_frost_lance(image: &mut RgbaImage, seed: u64, t: f64, width: f64, height: f64) {
    // A travelling freeze front: narrow at the caster, opening into an
    // icy impact cluster. Each shard is a separate layer, not a flat
    // blue explosion, which keeps the effect legible at sprite scale.
    let travel = t.powf(0.68);
    let front_x = width * (0.18 + travel * 0.62);
    let ground_y = height * 0.70;
    let glow = (std::f64::consts::PI * t).sin().max(0.0);
    draw_stroke(
        image,
        width * 0.14,
        ground_y,
        front_x,
        ground_y,
        1.1,
        Rgba([97, 207, 255, (150.0 * glow) as u8]),
    );
    for shard in 0..9 {
        let spread = (pseudo(seed, shard) - 0.5) * height * (0.10 + travel * 0.35);
        let x = front_x + (pseudo(seed + 5, shard) - 0.35) * width * 0.15;
        let y = ground_y + spread;
        let size = width.min(height) * (0.025 + travel * 0.055) * (0.65 + pseudo(seed + 9, shard));
        draw_disc(
            image,
            x,
            y,
            size * 1.8,
            Rgba([75, 168, 255, (105.0 * glow) as u8]),
        );
        draw_disc(
            image,
            x,
            y - size * 0.22,
            size * 0.8,
            Rgba([216, 250, 255, (235.0 * glow) as u8]),
        );
    }
    draw_ring(
        image,
        front_x,
        ground_y,
        width.min(height) * (0.04 + travel * 0.14),
        1.2,
        Rgba([196, 247, 255, (210.0 * (1.0 - t * 0.45)) as u8]),
    );
}

pub(super) fn draw_storm_lance(image: &mut RgbaImage, seed: u64, t: f64, width: f64, height: f64) {
    // Three offset filaments make this read as electricity rather than
    // as a generic beam. The target remains the brightest focal point.
    let travel = t.powf(0.55);
    let end_x = width * (0.16 + travel * 0.68);
    let start_y = height * 0.58;
    for strand in 0..3 {
        let lane = (strand as f64 - 1.0) * 2.4;
        let mut last_x = width * 0.14;
        let mut last_y = start_y + lane;
        for node in 1..10 {
            let u = node as f64 / 9.0;
            let x = width * 0.14 + (end_x - width * 0.14) * u;
            let jitter = (pseudo(seed + strand, node) - 0.5) * height * 0.14 * (1.0 - u * 0.35);
            let y = start_y + lane + jitter;
            draw_stroke(
                image,
                last_x,
                last_y,
                x,
                y,
                if strand == 1 { 1.45 } else { 0.8 },
                Rgba([157, 104, 255, (125.0 + 90.0 * travel) as u8]),
            );
            draw_stroke(
                image,
                last_x,
                last_y,
                x,
                y,
                0.55,
                Rgba([235, 250, 255, (160.0 + 80.0 * travel) as u8]),
            );
            last_x = x;
            last_y = y;
        }
    }
    let impact = width.min(height) * (0.035 + travel * 0.12);
    draw_disc(
        image,
        end_x,
        start_y,
        impact * 1.9,
        Rgba([105, 71, 255, (125.0 * travel) as u8]),
    );
    draw_disc(
        image,
        end_x,
        start_y,
        impact * 0.62,
        Rgba([244, 253, 255, (245.0 * travel) as u8]),
    );
}

pub(super) fn draw_nova_beam(image: &mut RgbaImage, t: f64, width: f64, height: f64) {
    // Charge, release, sustained column, then collapse. The three
    // strokes are the sprite equivalent of core / shell / halo passes.
    let charge = (t / 0.22).clamp(0.0, 1.0);
    let release = ((t - 0.18) / 0.16).clamp(0.0, 1.0);
    let fade = ((1.0 - t) / 0.20).clamp(0.0, 1.0);
    let end_x = width * (0.22 + 0.62 * release);
    let y = height * 0.56;
    let orb = width.min(height) * (0.035 + charge * 0.11) * fade.max(0.45);
    let charge_visibility = charge.max(0.08);
    draw_disc(
        image,
        width * 0.19,
        y,
        orb * 1.8,
        Rgba([68, 210, 255, (95.0 * charge_visibility) as u8]),
    );
    draw_disc(
        image,
        width * 0.19,
        y,
        orb * 0.72,
        Rgba([255, 246, 178, (240.0 * charge_visibility) as u8]),
    );
    // Intake motes keep even the quiet charge frames visibly alive.
    for mote in 0..4 {
        let angle = std::f64::consts::TAU * (mote as f64 / 4.0 - t * 1.7);
        draw_disc(
            image,
            width * 0.19 + angle.cos() * orb * 1.65,
            y + angle.sin() * orb * 1.05,
            0.8,
            Rgba([255, 223, 117, (110.0 * charge_visibility) as u8]),
        );
    }
    if release > 0.0 {
        draw_stroke(
            image,
            width * 0.19,
            y,
            end_x,
            y,
            orb * 1.15,
            Rgba([49, 192, 255, (90.0 * fade) as u8]),
        );
        draw_stroke(
            image,
            width * 0.19,
            y,
            end_x,
            y,
            orb * 0.57,
            Rgba([104, 239, 255, (150.0 * fade) as u8]),
        );
        draw_stroke(
            image,
            width * 0.19,
            y,
            end_x,
            y,
            orb * 0.20,
            Rgba([255, 252, 216, (250.0 * fade) as u8]),
        );
        for ring in 0..4 {
            draw_ring(
                image,
                width * (0.22 + release * (0.14 + ring as f64 * 0.13)),
                y,
                orb * (0.6 + ring as f64 * 0.18),
                0.8,
                Rgba([255, 207, 89, (150.0 * fade) as u8]),
            );
        }
    }
}

pub(super) fn draw_voltaic_snare(
    image: &mut RgbaImage,
    t: f64,
    width: f64,
    height: f64,
    center_x: f64,
) {
    // A measured zone: boundary first, an outward snap, then a charged
    // central pillar and orbiting sparks.
    let grow = (t / 0.26).clamp(0.0, 1.0);
    let hold = (std::f64::consts::PI * t).sin().max(0.25);
    let radius = width.min(height) * (0.12 + 0.24 * grow);
    draw_disc(
        image,
        center_x,
        height * 0.62,
        radius,
        Rgba([92, 39, 194, (62.0 * hold) as u8]),
    );
    draw_ring(
        image,
        center_x,
        height * 0.62,
        radius,
        1.8,
        Rgba([185, 86, 255, (235.0 * hold) as u8]),
    );
    draw_ring(
        image,
        center_x,
        height * 0.62,
        radius * (1.14 - grow * 0.14),
        0.8,
        Rgba([231, 207, 255, (160.0 * (1.0 - grow * 0.5)) as u8]),
    );
    draw_stroke(
        image,
        center_x,
        height * 0.62,
        center_x,
        height * (0.62 - 0.32 * grow),
        radius * 0.42,
        Rgba([115, 50, 255, (120.0 * hold) as u8]),
    );
    draw_stroke(
        image,
        center_x,
        height * 0.62,
        center_x,
        height * (0.62 - 0.32 * grow),
        radius * 0.15,
        Rgba([241, 228, 255, (210.0 * hold) as u8]),
    );
    for spark in 0..7 {
        let angle = std::f64::consts::TAU * (spark as f64 / 7.0 + t * 1.4);
        draw_disc(
            image,
            center_x + angle.cos() * radius,
            height * 0.62 + angle.sin() * radius * 0.48,
            1.2,
            Rgba([247, 236, 255, (220.0 * hold) as u8]),
        );
    }
}
