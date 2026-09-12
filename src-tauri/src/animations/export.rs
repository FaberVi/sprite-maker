use crate::{
    assets::get_asset,
    error::{CommandError, CommandResult},
    models::ExportResult,
    workspace::{resolve_export_directory, workspace_path},
    AppState,
};
use image::{GenericImage, RgbaImage};
use rusqlite::OptionalExtension;
use tauri::State;

use super::{animation_row, load_animation_frames_from_table};

#[tauri::command]
pub fn export_animation(
    id: String,
    destination: Option<String>,
    state: State<'_, AppState>,
) -> CommandResult<ExportResult> {
    export_animation_inner(&id, destination, &state)
}

pub(crate) fn export_animation_inner(
    id: &str,
    destination: Option<String>,
    state: &AppState,
) -> CommandResult<ExportResult> {
    let animation = {
        let connection = state
            .db
            .lock()
            .map_err(|_| CommandError::new("database_locked", "Database lock was poisoned"))?;
        let mut animation = connection
            .query_row(
                "SELECT id, workspace_id, worktree_id, name, fps, looping, frames_json, created_at, updated_at FROM animations WHERE id = ?1",
                [id],
                animation_row,
            )
            .optional()?
            .ok_or_else(|| CommandError::new("animation_not_found", "Animation no longer exists"))?;
        if animation.frames.is_empty() {
            animation.frames = load_animation_frames_from_table(&connection, id)?;
        }
        animation
    };
    if animation.frames.is_empty() {
        return Err(CommandError::new(
            "empty_animation",
            "Add at least one frame before exporting",
        ));
    }
    let mut decoded = Vec::new();
    let mut frame_width = 0;
    let mut frame_height = 0;
    for frame in &animation.frames {
        let asset = get_asset(state, &frame.asset_id)?;
        let image = image::open(&asset.path)?.to_rgba8();
        frame_width = frame_width.max(image.width());
        frame_height = frame_height.max(image.height());
        decoded.push((asset, image));
    }
    let sheet_width = frame_width
        .checked_mul(decoded.len() as u32)
        .ok_or_else(|| {
            CommandError::new("export_too_large", "Spritesheet dimensions are too large")
        })?;
    let mut sheet = RgbaImage::new(sheet_width, frame_height);
    for (index, (_, frame)) in decoded.iter().enumerate() {
        sheet.copy_from(frame, index as u32 * frame_width, 0)?;
    }
    let workspace_root = workspace_path(state, &animation.workspace_id)?;
    let output_directory =
        resolve_export_directory(&workspace_root, destination.as_deref())?;
    std::fs::create_dir_all(&output_directory)?;
    let slug: String = animation
        .name
        .chars()
        .map(|value| {
            if value.is_ascii_alphanumeric() {
                value.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let slug = slug.trim_matches('-');
    let png_path = output_directory.join(format!("{slug}.png"));
    let metadata_path = output_directory.join(format!("{slug}.json"));
    sheet.save(&png_path)?;
    let frames: Vec<_> = decoded.iter().enumerate().map(|(index, (asset, _))| serde_json::json!({
        "index": index, "assetId": asset.id, "source": asset.relative_path,
        "x": index as u32 * frame_width, "y": 0, "width": frame_width, "height": frame_height,
        "durationMs": animation.frames[index].duration_ms.unwrap_or_else(|| (1000.0 / animation.fps).round() as u32)
    })).collect();
    let metadata = serde_json::json!({
        "name": animation.name, "image": png_path.file_name().and_then(|value| value.to_str()).unwrap_or("spritesheet.png"),
        "frameWidth": frame_width, "frameHeight": frame_height, "frameCount": frames.len(), "fps": animation.fps,
        "loop": animation.looping, "frames": frames
    });
    std::fs::write(
        &metadata_path,
        serde_json::to_vec_pretty(&metadata)
            .map_err(|error| CommandError::new("serialization_error", error.to_string()))?,
    )?;
    Ok(ExportResult {
        png_path: png_path.to_string_lossy().into_owned(),
        metadata_path: metadata_path.to_string_lossy().into_owned(),
        width: sheet_width,
        height: frame_height,
    })
}
