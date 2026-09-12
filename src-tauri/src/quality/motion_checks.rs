use image::RgbaImage;

use super::metrics::{score_with_penalty, AnalyzedFrame, PendingCheck};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct LimbBlob {
    pub(super) min_x: u32,
    pub(super) max_x: u32,
    pub(super) pixels: u32,
    pub(super) luminance: f64,
}

/// Classification of a frame's lower body: two separated legs, an occupied
/// band whose legs cannot be told apart (fused or shredded), or no lower
/// body at all.
#[derive(Debug, PartialEq)]
pub(super) enum LowerBodyView {
    TwoBlobs(Vec<LimbBlob>),
    Indistinct,
    Empty,
}

/// Segments the lower body band into leg blobs, left to right. A single
/// column run means the legs fused (or a trailing scarf connected them);
/// three or more runs are equally unmeasurable. Both become `Indistinct`.
pub(super) fn lower_body_view(
    image: &RgbaImage,
    bounds: Option<(u32, u32, u32, u32)>,
) -> LowerBodyView {
    let Some((min_x, min_y, max_x, max_y)) = bounds else {
        return LowerBodyView::Empty;
    };
    if max_x < min_x || max_y <= min_y + 2 {
        return LowerBodyView::Empty;
    }
    let band_start = min_y + ((max_y - min_y) as f64 * 0.66) as u32;
    if band_start >= max_y {
        return LowerBodyView::Empty;
    }
    let width = (max_x - min_x + 1) as usize;
    let mut occupied = vec![false; width];
    let mut occupied_columns = 0_usize;
    for y in band_start..=max_y {
        for x in min_x..=max_x {
            if image.get_pixel(x, y)[3] > 8 {
                let index = (x - min_x) as usize;
                if !occupied[index] {
                    occupied[index] = true;
                    occupied_columns += 1;
                }
            }
        }
    }
    if occupied_columns == 0 {
        return LowerBodyView::Empty;
    }
    let mut runs: Vec<(usize, usize)> = Vec::new();
    let mut start: Option<usize> = None;
    for (index, is_occupied) in occupied.iter().copied().enumerate().take(width) {
        if is_occupied && start.is_none() {
            start = Some(index);
        }
        if start.is_some() && (!is_occupied || index + 1 == width) {
            let begin = start.take().expect("run start");
            let end = if is_occupied { index } else { index - 1 };
            runs.push((begin, end));
        }
    }
    let runs: Vec<_> = runs
        .into_iter()
        .filter(|(begin, end)| end - begin + 1 >= 2)
        .collect();
    if runs.len() != 2 {
        return LowerBodyView::Indistinct;
    }
    let mut blobs = Vec::with_capacity(2);
    for (begin, end) in runs {
        let mut weight = 0.0;
        let mut luminance = 0.0;
        let mut pixels = 0_u32;
        for y in band_start..=max_y {
            for x in (min_x + begin as u32)..=(min_x + end as u32) {
                let pixel = image.get_pixel(x, y);
                let alpha = pixel[3];
                if alpha > 8 {
                    let alpha_weight = alpha as f64 / 255.0;
                    let value =
                        0.299 * pixel[0] as f64 + 0.587 * pixel[1] as f64 + 0.114 * pixel[2] as f64;
                    weight += alpha_weight;
                    luminance += value * alpha_weight;
                    pixels += 1;
                }
            }
        }
        if pixels < 8 || weight <= 0.0 {
            return LowerBodyView::Indistinct;
        }
        blobs.push(LimbBlob {
            min_x: min_x + begin as u32,
            max_x: min_x + end as u32,
            pixels,
            luminance: luminance / weight,
        });
    }
    LowerBodyView::TwoBlobs(blobs)
}

/// Detects a broken far-limb shading lock. In a correct paired-limb cycle the
/// far leg stays visibly darker than the near leg in every frame where both
/// legs are separate; frames that drop the distinction read as a limb swap.
pub(super) fn limb_shading_checks(analyzed: &[AnalyzedFrame]) -> Vec<PendingCheck> {
    let gaps: Vec<Option<(usize, f64)>> = analyzed
        .iter()
        .enumerate()
        .map(
            |(index, frame)| match lower_body_view(&frame.image, frame.metrics.bounds) {
                LowerBodyView::TwoBlobs(blobs) => {
                    Some((index, (blobs[0].luminance - blobs[1].luminance).abs()))
                }
                _ => None,
            },
        )
        .collect();
    let measurable: Vec<(usize, f64)> = gaps.into_iter().flatten().collect();
    if measurable.len() < 3 {
        return Vec::new();
    }
    // 8/255 is the smallest gap that still reads as two shades at 1× scale;
    // measured real cycles sit near 9 (subtle palettes) to 100 (high contrast).
    let strong = measurable.iter().filter(|(_, gap)| *gap >= 8.0).count();
    if strong < 2 || (strong as f64 / measurable.len() as f64) < 0.6 {
        return Vec::new();
    }
    measurable
        .iter()
        .filter(|(_, gap)| *gap < 4.0)
        .map(|(index, gap)| PendingCheck {
            check_type: "limb_identity",
            frame_index: Some(*index as u32),
            comparison_frame_index: None,
            severity: "warning",
            score: score_with_penalty(24.0),
            message: format!(
                "Frame {} lost the far-limb shading lock: both legs read as the same limb (shading gap {:.1}/255). Regenerate it with the far leg visibly darker.",
                index + 1,
                gap
            ),
            metric_value: Some(*gap),
            metric_unit: Some("luminance gap"),
            repair_action: Some("regenerate"),
        })
        .collect()
}

/// Detects a hop masquerading as a run: a paired-limb cycle needs visible leg
/// alternation, so frames where the legs fuse into one silhouette should be
/// the minority (passing or gathered poses). When at least one frame proves
/// the character's legs do separate, an indistinct majority means the cycle
/// collapsed into synchronized legs and must be regenerated.
pub(super) fn leg_alternation_checks(analyzed: &[AnalyzedFrame]) -> Vec<PendingCheck> {
    let mut separated = 0_usize;
    let mut indistinct = 0_usize;
    for frame in analyzed {
        match lower_body_view(&frame.image, frame.metrics.bounds) {
            LowerBodyView::TwoBlobs(_) => separated += 1,
            LowerBodyView::Indistinct => indistinct += 1,
            LowerBodyView::Empty => {}
        }
    }
    let total = separated + indistinct;
    if total < 5 || separated == 0 {
        return Vec::new();
    }
    let ratio = indistinct as f64 / total as f64;
    if ratio <= 0.6 {
        return Vec::new();
    }
    let severe = ratio > 0.75;
    vec![PendingCheck {
        check_type: "leg_separation",
        frame_index: None,
        comparison_frame_index: None,
        severity: if severe { "error" } else { "warning" },
        score: score_with_penalty(if severe { 35.0 } else { 18.0 }),
        message: format!(
            "The legs read as one silhouette in {indistinct} of {total} frames, so the cycle lost leg alternation and plays as a hop. Regenerate it with two wide split stances half a cycle apart (NEAR contact, then FAR contact) and keep the far leg visible as the darker shape behind the near leg in every gathered pose. If AI frames keep failing here, rig the sprite in the Rig tab and render deterministically."
        ),
        metric_value: Some(ratio * 100.0),
        metric_unit: Some("% frames with fused legs"),
        repair_action: Some("regenerate"),
    }]
}
