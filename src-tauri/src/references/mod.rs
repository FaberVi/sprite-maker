use crate::{
    error::{CommandError, CommandResult},
    models::ReferenceImage,
    AppState,
};
use chrono::Utc;
use rusqlite::{params, OptionalExtension};
use std::path::{Path, PathBuf};
use tauri::State;

mod import;
mod prompt;

pub(crate) use import::import_reference_image_inner;
pub use import::{
    __cmd__import_reference_bytes, __cmd__import_reference_image,
    __tauri_command_name_import_reference_bytes, __tauri_command_name_import_reference_image,
    import_reference_bytes, import_reference_image,
};
pub use prompt::prompt_context;

use prompt::validate_category;

fn reference_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ReferenceImage> {
    Ok(ReferenceImage {
        id: row.get(0)?,
        project_id: row.get(1)?,
        worktree_id: row.get(2)?,
        name: row.get(3)?,
        path: row.get(4)?,
        relative_path: row.get(5)?,
        category: row.get(6)?,
        notes: row.get(7)?,
        format: row.get(8)?,
        width: row.get(9)?,
        height: row.get(10)?,
        file_size: row.get(11)?,
        content_hash: row.get(12)?,
        created_at: row.get(13)?,
        updated_at: row.get(14)?,
    })
}

fn select_reference() -> &'static str {
    "SELECT id, project_id, worktree_id, name, path, relative_path, category, notes, format, width, height, file_size, content_hash, created_at, updated_at FROM reference_images"
}

#[tauri::command]
pub fn list_reference_images(
    worktree_id: String,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> CommandResult<Vec<ReferenceImage>> {
    let connection = state
        .db
        .lock()
        .map_err(|_| CommandError::new("database_locked", "Database lock was poisoned"))?;
    let mut statement = connection.prepare(&format!(
        "{} WHERE worktree_id = ?1 ORDER BY updated_at DESC",
        select_reference()
    ))?;
    let rows = statement.query_map([worktree_id], reference_row)?;
    let references: Vec<_> = rows.filter_map(Result::ok).collect();
    for reference in &references {
        crate::allow_asset_file(Some(&app), Path::new(&reference.path))?;
    }
    Ok(references)
}

#[tauri::command]
pub fn update_reference_image(
    id: String,
    name: String,
    category: String,
    notes: Option<String>,
    state: State<'_, AppState>,
) -> CommandResult<ReferenceImage> {
    validate_category(&category)?;
    let name = name.trim();
    if name.is_empty() {
        return Err(CommandError::new(
            "invalid_reference_name",
            "Reference name cannot be empty",
        ));
    }
    let now = Utc::now().to_rfc3339();
    let connection = state
        .db
        .lock()
        .map_err(|_| CommandError::new("database_locked", "Database lock was poisoned"))?;
    let changed = connection.execute(
        "UPDATE reference_images SET name=?2, category=?3, notes=?4, updated_at=?5 WHERE id=?1",
        params![
            id,
            name,
            category,
            notes.filter(|value| !value.trim().is_empty()),
            now
        ],
    )?;
    if changed == 0 {
        return Err(CommandError::new(
            "reference_not_found",
            "The reference image no longer exists",
        ));
    }
    connection
        .query_row(
            &format!("{} WHERE id = ?1", select_reference()),
            [id],
            reference_row,
        )
        .map_err(Into::into)
}

#[tauri::command]
pub fn delete_reference_image(id: String, state: State<'_, AppState>) -> CommandResult<()> {
    let path: Option<String> = {
        let connection = state
            .db
            .lock()
            .map_err(|_| CommandError::new("database_locked", "Database lock was poisoned"))?;
        connection
            .query_row(
                "SELECT path FROM reference_images WHERE id=?1",
                [&id],
                |row| row.get(0),
            )
            .optional()?
    };
    let path = path.ok_or_else(|| {
        CommandError::new(
            "reference_not_found",
            "The reference image no longer exists",
        )
    })?;
    let connection = state
        .db
        .lock()
        .map_err(|_| CommandError::new("database_locked", "Database lock was poisoned"))?;
    connection.execute("DELETE FROM reference_images WHERE id=?1", [&id])?;
    let path = PathBuf::from(path);
    if path.is_file() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

#[tauri::command]
pub fn set_conversation_reference(
    conversation_id: String,
    reference_id: String,
    active: bool,
    strength: Option<f64>,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    set_conversation_reference_inner(&conversation_id, &reference_id, active, strength, &state)
}

pub(crate) fn set_conversation_reference_inner(
    conversation_id: &str,
    reference_id: &str,
    active: bool,
    strength: Option<f64>,
    state: &AppState,
) -> CommandResult<()> {
    let strength = strength.unwrap_or(1.0).clamp(0.0, 2.0);
    let connection = state
        .db
        .lock()
        .map_err(|_| CommandError::new("database_locked", "Database lock was poisoned"))?;
    let compatible: bool = connection.query_row(
        r#"SELECT EXISTS(
             SELECT 1 FROM conversations c
             JOIN reference_images r ON r.project_id = c.workspace_id
             WHERE c.id=?1 AND r.id=?2
           )"#,
        params![conversation_id, reference_id],
        |row| row.get(0),
    )?;
    if !compatible {
        return Err(CommandError::new(
            "invalid_conversation_reference",
            "The reference and conversation must belong to the same project",
        ));
    }
    if active {
        connection.execute(
            r#"INSERT INTO conversation_references(conversation_id, reference_id, active, strength, created_at)
               VALUES (?1, ?2, 1, ?3, ?4)
               ON CONFLICT(conversation_id, reference_id) DO UPDATE SET active=1, strength=excluded.strength"#,
            params![conversation_id, reference_id, strength, Utc::now().to_rfc3339()],
        )?;
    } else {
        connection.execute(
            "DELETE FROM conversation_references WHERE conversation_id=?1 AND reference_id=?2",
            params![conversation_id, reference_id],
        )?;
    }
    Ok(())
}

#[tauri::command]
pub fn list_conversation_reference_ids(
    conversation_id: String,
    state: State<'_, AppState>,
) -> CommandResult<Vec<String>> {
    let connection = state
        .db
        .lock()
        .map_err(|_| CommandError::new("database_locked", "Database lock was poisoned"))?;
    let mut statement = connection.prepare(
        "SELECT reference_id FROM conversation_references WHERE conversation_id=?1 AND active=1 ORDER BY created_at",
    )?;
    let rows = statement.query_map([conversation_id], |row| row.get(0))?;
    Ok(rows.filter_map(Result::ok).collect())
}
