use crate::{
    error::{CommandError, CommandResult},
    models::ReferenceImage,
    AppState,
};
use chrono::Utc;
use image::GenericImageView;
use rusqlite::{params, OptionalExtension};
use std::path::{Path, PathBuf};
use tauri::State;
use uuid::Uuid;

use super::prompt::validate_category;

fn portable_file_name(path: &Path, extension: &str) -> String {
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("reference");
    let slug: String = stem
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let slug = slug.trim_matches('-');
    format!(
        "{}-{}.{}",
        &Uuid::new_v4().simple().to_string()[..8],
        if slug.is_empty() { "reference" } else { slug },
        extension
    )
}

fn file_hash(path: &Path) -> CommandResult<String> {
    let bytes = std::fs::read(path)?;
    Ok(blake3::hash(&bytes).to_hex().to_string())
}

fn reference_extension(bytes: &[u8]) -> CommandResult<&'static str> {
    match image::guess_format(bytes)
        .map_err(|error| CommandError::new("invalid_reference_image", error.to_string()))?
    {
        image::ImageFormat::Png => Ok("png"),
        image::ImageFormat::Jpeg => Ok("jpg"),
        image::ImageFormat::WebP => Ok("webp"),
        image::ImageFormat::Gif => Ok("gif"),
        _ => Err(CommandError::new(
            "unsupported_reference_format",
            "Reference images must be PNG, JPEG, WebP, or GIF",
        )),
    }
}

fn store_reference_bytes(
    worktree_id: String,
    source_name: String,
    bytes: Vec<u8>,
    category: String,
    notes: Option<String>,
    app: Option<tauri::AppHandle>,
    state: &AppState,
) -> CommandResult<ReferenceImage> {
    validate_category(&category)?;
    if bytes.is_empty() || bytes.len() > 25 * 1024 * 1024 {
        return Err(CommandError::new(
            "invalid_reference_size",
            "Reference images must be between 1 byte and 25 MB",
        ));
    }
    let image = image::load_from_memory(&bytes)
        .map_err(|error| CommandError::new("invalid_reference_image", error.to_string()))?;
    let (width, height) = image.dimensions();
    let format = reference_extension(&bytes)?.to_string();
    let source = PathBuf::from(if source_name.trim().is_empty() {
        "pasted-reference"
    } else {
        source_name.as_str()
    });
    let (project_id, project_path, worktree_slug): (String, String, String) = {
        let connection = state
            .db
            .lock()
            .map_err(|_| CommandError::new("database_locked", "Database lock was poisoned"))?;
        connection
            .query_row(
                r#"SELECT p.id, p.path, w.slug
                   FROM worktrees w JOIN projects p ON p.id = w.project_id
                   WHERE w.id = ?1"#,
                [&worktree_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?
            .ok_or_else(|| {
                CommandError::new("worktree_not_found", "The worktree no longer exists")
            })?
    };
    let project_root = PathBuf::from(&project_path);
    let reference_directory = project_root
        .join("worktrees")
        .join(worktree_slug)
        .join("references");
    std::fs::create_dir_all(&reference_directory)?;
    let destination = reference_directory.join(portable_file_name(&source, &format));
    std::fs::write(&destination, &bytes)?;
    crate::allow_asset_file(app.as_ref(), &destination)?;
    let relative_path = destination
        .strip_prefix(&project_root)
        .unwrap_or(&destination)
        .to_string_lossy()
        .replace('\\', "/");
    let now = Utc::now().to_rfc3339();
    let reference = ReferenceImage {
        id: Uuid::new_v4().to_string(),
        project_id,
        worktree_id,
        name: source
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("Pasted reference")
            .to_string(),
        path: destination.to_string_lossy().into_owned(),
        relative_path,
        category,
        notes: notes.filter(|value| !value.trim().is_empty()),
        format,
        width,
        height,
        file_size: bytes.len() as u64,
        content_hash: file_hash(&destination)?,
        created_at: now.clone(),
        updated_at: now,
    };
    let connection = state
        .db
        .lock()
        .map_err(|_| CommandError::new("database_locked", "Database lock was poisoned"))?;
    connection.execute(
        r#"INSERT INTO reference_images(
            id, project_id, worktree_id, name, path, relative_path, category, notes,
            format, width, height, file_size, content_hash, created_at, updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)"#,
        params![
            reference.id,
            reference.project_id,
            reference.worktree_id,
            reference.name,
            reference.path,
            reference.relative_path,
            reference.category,
            reference.notes,
            reference.format,
            reference.width,
            reference.height,
            reference.file_size,
            reference.content_hash,
            reference.created_at,
            reference.updated_at
        ],
    )?;
    Ok(reference)
}

#[tauri::command]
pub fn import_reference_image(
    worktree_id: String,
    source_path: String,
    category: String,
    notes: Option<String>,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> CommandResult<ReferenceImage> {
    let source = PathBuf::from(&source_path);
    if !source.is_file() {
        return Err(CommandError::new(
            "reference_not_found",
            "The selected reference image no longer exists",
        ));
    }
    ensure_reference_source_within_project(&worktree_id, &source, &state)?;
    let bytes = std::fs::read(&source)?;
    let source_name = source
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("reference")
        .to_string();
    store_reference_bytes(
        worktree_id,
        source_name,
        bytes,
        category,
        notes,
        Some(app),
        &state,
    )
}

fn ensure_reference_source_within_project(
    worktree_id: &str,
    source: &Path,
    state: &AppState,
) -> CommandResult<()> {
    let project_path: String = {
        let connection = state
            .db
            .lock()
            .map_err(|_| CommandError::new("database_locked", "Database lock was poisoned"))?;
        connection
            .query_row(
                r#"SELECT p.path
                   FROM worktrees w JOIN projects p ON p.id = w.project_id
                   WHERE w.id = ?1"#,
                [worktree_id],
                |row| row.get(0),
            )
            .optional()?
            .ok_or_else(|| {
                CommandError::new("worktree_not_found", "The worktree no longer exists")
            })?
    };
    let project_root = PathBuf::from(&project_path)
        .canonicalize()
        .map_err(CommandError::from)?;
    let canonical_source = source
        .canonicalize()
        .map_err(CommandError::from)?;
    if !canonical_source.starts_with(&project_root) {
        return Err(CommandError::new(
            "reference_outside_project",
            "Reference images must live inside the open project folder",
        ));
    }
    Ok(())
}

pub(crate) fn import_reference_image_inner(
    worktree_id: String,
    source_path: String,
    category: String,
    notes: Option<String>,
    state: &AppState,
) -> CommandResult<ReferenceImage> {
    let source = PathBuf::from(&source_path);
    if !source.is_file() {
        return Err(CommandError::new(
            "reference_not_found",
            "The selected reference image no longer exists",
        ));
    }
    ensure_reference_source_within_project(&worktree_id, &source, state)?;
    let bytes = std::fs::read(&source)?;
    let source_name = source
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("reference")
        .to_string();
    store_reference_bytes(
        worktree_id,
        source_name,
        bytes,
        category,
        notes,
        None,
        state,
    )
}

#[tauri::command]
pub fn import_reference_bytes(
    worktree_id: String,
    file_name: String,
    bytes: Vec<u8>,
    category: String,
    notes: Option<String>,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> CommandResult<ReferenceImage> {
    store_reference_bytes(
        worktree_id,
        file_name,
        bytes,
        category,
        notes,
        Some(app),
        &state,
    )
}
