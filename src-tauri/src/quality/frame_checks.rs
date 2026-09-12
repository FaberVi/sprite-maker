use crate::{
    error::{CommandError, CommandResult},
    jobs::{cancellation_requested, set_job_state, JobProgress},
    AppState,
};

use super::metrics::{
    bounds_area, centroid_distance, palette_distance, pixel_difference, score_with_penalty,
    AnalyzedFrame, PendingCheck,
};

pub(super) struct VisualPenalties {
    pub(super) alignment: f64,
    pub(super) continuity: f64,
    pub(super) consistency: f64,
    pub(super) weapon: f64,
    pub(super) transparency: f64,
}

pub(super) fn collect_visual_checks(
    app: Option<&tauri::AppHandle>,
    state: &AppState,
    job_id: &str,
    analyzed: &[AnalyzedFrame],
) -> CommandResult<(Vec<PendingCheck>, VisualPenalties)> {
    let mut checks = Vec::new();
    let mut alignment_penalty = 0.0;
    let mut continuity_penalty = 0.0;
    let mut consistency_penalty = 0.0;
    let mut weapon_penalty = 0.0;
    let mut transparency_penalty = 0.0;
    let expected_width = analyzed[0].metrics.width;
    let expected_height = analyzed[0].metrics.height;
    for (index, frame) in analyzed.iter().enumerate() {
        let metrics = &frame.metrics;
        if metrics.width != expected_width || metrics.height != expected_height {
            consistency_penalty += 25.0;
            checks.push(PendingCheck {
                check_type: "dimensions",
                frame_index: Some(index as u32),
                comparison_frame_index: None,
                severity: "error",
                score: 30.0,
                message: format!(
                    "Frame {} is {}×{} instead of {}×{}.",
                    index + 1,
                    metrics.width,
                    metrics.height,
                    expected_width,
                    expected_height
                ),
                metric_value: None,
                metric_unit: None,
                repair_action: Some("normalize_dimensions"),
            });
        }
        if metrics.alpha_coverage > 0.96 {
            transparency_penalty += 35.0;
            checks.push(PendingCheck {
                check_type: "transparency",
                frame_index: Some(index as u32),
                comparison_frame_index: None,
                severity: "error",
                score: 35.0,
                message: format!(
                    "Frame {} is almost fully opaque; a background may be baked in.",
                    index + 1
                ),
                metric_value: Some(metrics.alpha_coverage * 100.0),
                metric_unit: Some("% opaque area"),
                repair_action: Some("inspect_transparency"),
            });
        }
        if metrics.opaque_edge_pixels > 0 {
            transparency_penalty += (metrics.opaque_edge_pixels as f64 / 8.0).min(12.0);
            checks.push(PendingCheck {
                check_type: "boundary",
                frame_index: Some(index as u32),
                comparison_frame_index: None,
                severity: if metrics.opaque_edge_pixels > 8 {
                    "error"
                } else {
                    "warning"
                },
                score: score_with_penalty(metrics.opaque_edge_pixels as f64 * 2.0),
                message: format!(
                    "Frame {} touches the canvas boundary at {} pixels.",
                    index + 1,
                    metrics.opaque_edge_pixels
                ),
                metric_value: Some(metrics.opaque_edge_pixels as f64),
                metric_unit: Some("edge pixels"),
                repair_action: Some("add_padding"),
            });
        }
    }
    for index in 1..analyzed.len() {
        if cancellation_requested(state, job_id)? {
            return Err(CommandError::new(
                "job_cancelled",
                "Quality analysis cancelled",
            ));
        }
        let previous = &analyzed[index - 1];
        let current = &analyzed[index];
        let difference = pixel_difference(&previous.image, &current.image);
        let hash_distance =
            (previous.metrics.perceptual_hash ^ current.metrics.perceptual_hash).count_ones();
        let drift = centroid_distance(&previous.metrics, &current.metrics);
        let diagonal = ((expected_width.pow(2) + expected_height.pow(2)) as f64)
            .sqrt()
            .max(1.0);
        let palette = palette_distance(&previous.metrics, &current.metrics);
        let previous_area = bounds_area(&previous.metrics).max(1.0);
        let area_ratio = bounds_area(&current.metrics) / previous_area;
        if difference < 0.012 || hash_distance <= 1 {
            continuity_penalty += 12.0;
            checks.push(PendingCheck {
                check_type: "duplicate",
                frame_index: Some(index as u32),
                comparison_frame_index: Some((index - 1) as u32),
                severity: "warning",
                score: 45.0,
                message: format!(
                    "Frames {} and {} appear nearly identical.",
                    index,
                    index + 1
                ),
                metric_value: Some(difference * 100.0),
                metric_unit: Some("% pixel difference"),
                repair_action: Some("remove_duplicate"),
            });
        }
        if difference > 0.38 {
            continuity_penalty += (difference * 35.0).min(22.0);
            weapon_penalty += 8.0;
            checks.push(PendingCheck {
                check_type: "sudden_change",
                frame_index: Some(index as u32),
                comparison_frame_index: Some((index - 1) as u32),
                severity: if difference > 0.55 {
                    "error"
                } else {
                    "warning"
                },
                score: score_with_penalty(difference * 100.0),
                message: format!(
                    "Large visual change detected between Frames {} and {}.",
                    index,
                    index + 1
                ),
                metric_value: Some(difference * 100.0),
                metric_unit: Some("% pixel difference"),
                repair_action: Some("regenerate_transition"),
            });
        }
        if drift / diagonal > 0.10 {
            alignment_penalty += (drift / diagonal * 100.0).min(25.0);
            checks.push(PendingCheck {
                check_type: "alignment",
                frame_index: Some(index as u32),
                comparison_frame_index: Some((index - 1) as u32),
                severity: if drift / diagonal > 0.20 {
                    "error"
                } else {
                    "warning"
                },
                score: score_with_penalty(drift / diagonal * 180.0),
                message: format!(
                    "Subject drifted {:.1} px between Frames {} and {}.",
                    drift,
                    index,
                    index + 1
                ),
                metric_value: Some(drift),
                metric_unit: Some("pixels"),
                repair_action: Some("auto_align"),
            });
        }
        if !(0.72..=1.38).contains(&area_ratio) {
            consistency_penalty += ((1.0 - area_ratio).abs() * 24.0).min(18.0);
            checks.push(PendingCheck {
                check_type: "scale_consistency",
                frame_index: Some(index as u32),
                comparison_frame_index: Some((index - 1) as u32),
                severity: "warning",
                score: score_with_penalty((1.0 - area_ratio).abs() * 100.0),
                message: format!("Subject scale changes sharply at Frame {}.", index + 1),
                metric_value: Some(area_ratio),
                metric_unit: Some("area ratio"),
                repair_action: Some("normalize_scale"),
            });
        }
        if palette > 58.0 {
            consistency_penalty += (palette / 12.0).min(12.0);
            weapon_penalty += 4.0;
            checks.push(PendingCheck {
                check_type: "palette_consistency",
                frame_index: Some(index as u32),
                comparison_frame_index: Some((index - 1) as u32),
                severity: "warning",
                score: score_with_penalty(palette * 0.7),
                message: format!("Palette may have shifted in Frame {}.", index + 1),
                metric_value: Some(palette),
                metric_unit: Some("RGB distance"),
                repair_action: Some("lock_palette"),
            });
        }
        set_job_state(
            app,
            state,
            job_id,
            JobProgress {
                status: "analyzing",
                progress: 0.48 + 0.37 * (index as f64 / analyzed.len() as f64),
                stage: &format!("Comparing transition {} of {}", index, analyzed.len() - 1),
                error_message: None,
                result_path: None,
            },
        )?;
    }
    Ok((
        checks,
        VisualPenalties {
            alignment: alignment_penalty,
            continuity: continuity_penalty,
            consistency: consistency_penalty,
            weapon: weapon_penalty,
            transparency: transparency_penalty,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::collect_visual_checks;
    use super::super::metrics::{AnalyzedFrame, FrameMetrics};
    use crate::{database, AppState};
    use image::{Rgba, RgbaImage};
    use uuid::Uuid;

    fn analyzed_frame(width: u32, height: u32) -> AnalyzedFrame {
        AnalyzedFrame {
            metrics: FrameMetrics {
                asset_id: "asset".into(),
                content_hash: "hash".into(),
                width,
                height,
                bounds: Some((0, 0, width.saturating_sub(1), height.saturating_sub(1))),
                centroid: Some((width as f64 / 2.0, height as f64 / 2.0)),
                alpha_coverage: 0.2,
                opaque_edge_pixels: 0,
                perceptual_hash: 0,
                palette: (0.0, 0.0, 0.0),
            },
            image: RgbaImage::from_pixel(width, height, Rgba([20, 40, 60, 255])),
        }
    }

    #[test]
    fn flags_dimension_mismatches_between_frames() {
        let root = std::env::temp_dir().join(format!("frame-checks-{}", Uuid::new_v4()));
        let project = root.join("game");
        std::fs::create_dir_all(&project).expect("project directory");
        let connection = database::open(&root.join("app.sqlite3")).expect("database");
        let state = AppState::from_connection(connection);
        let workspace = crate::workspace::create_workspace_inner(
            "Game".into(),
            project.to_string_lossy().into_owned(),
            &state,
        )
        .expect("workspace");
        {
            let connection = state.db.lock().expect("database lock");
            connection
                .execute(
                    "INSERT INTO background_jobs(id, project_id, kind, status, progress, stage, cancel_requested, created_at, updated_at)
                     VALUES ('job', ?1, 'quality', 'analyzing', 0.0, 'compare', 0, 'now', 'now')",
                    [&workspace.id],
                )
                .expect("background job");
        }
        let analyzed = vec![analyzed_frame(16, 16), analyzed_frame(20, 16)];
        let (checks, _) =
            collect_visual_checks(None, &state, "job", &analyzed).expect("visual checks");
        assert!(checks.iter().any(|check| check.check_type == "dimensions"));
        drop(state);
        let _ = std::fs::remove_dir_all(root);
    }
}
