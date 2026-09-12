use crate::error::{CommandError, CommandResult};
use rusqlite::{params, OptionalExtension};
use std::path::Path;

pub(super) const REFERENCE_CATEGORIES: &[&str] = &[
    "character_appearance",
    "clothing",
    "face",
    "weapon",
    "pose",
    "art_style",
    "environment",
    "palette",
    "animation",
    "vfx",
    "anatomy",
    "lighting",
    "other",
];

struct PromptReference {
    name: String,
    path: String,
    category: String,
    notes: Option<String>,
    width: u32,
    height: u32,
    format: String,
    content_hash: String,
}

fn prompt_reference_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<PromptReference> {
    Ok(PromptReference {
        name: row.get(0)?,
        path: row.get(1)?,
        category: row.get(2)?,
        notes: row.get(3)?,
        width: row.get(4)?,
        height: row.get(5)?,
        format: row.get(6)?,
        content_hash: row.get(7)?,
    })
}

pub(super) fn validate_category(category: &str) -> CommandResult<()> {
    if REFERENCE_CATEGORIES.contains(&category) {
        Ok(())
    } else {
        Err(CommandError::new(
            "invalid_reference_category",
            "Choose a supported reference category",
        ))
    }
}

pub fn prompt_context(
    state: &crate::AppState,
    conversation_id: &str,
    reference_ids: &[String],
    maximum: usize,
) -> CommandResult<(String, Vec<String>)> {
    if reference_ids.len() > maximum {
        return Err(CommandError::new(
            "too_many_references",
            format!("The selected provider supports at most {maximum} reference images"),
        ));
    }
    let connection = state
        .db
        .lock()
        .map_err(|_| CommandError::new("database_locked", "Database lock was poisoned"))?;
    let mut lines = Vec::new();
    let mut paths = Vec::new();
    for id in reference_ids {
        let reference = connection
            .query_row(
                r#"SELECT r.name, r.path, r.category, r.notes, r.width, r.height, r.format, r.content_hash
                   FROM reference_images r
                   JOIN conversations c ON c.workspace_id = r.project_id
                   WHERE c.id=?1 AND r.id=?2"#,
                params![conversation_id, id],
                prompt_reference_row,
            )
            .optional()?
            .ok_or_else(|| {
                CommandError::new(
                    "invalid_conversation_reference",
                    "A selected reference is unavailable in this project",
                )
            })?;
        if !Path::new(&reference.path).is_file() {
            return Err(CommandError::new(
                "reference_file_missing",
                format!(
                    "The reference image {} is missing from disk",
                    reference.name
                ),
            ));
        }
        lines.push(format!(
            "- {} [{}]: {} — {}x{} {}, content hash {}{}",
            reference.name,
            reference.category,
            reference.path,
            reference.width,
            reference.height,
            reference.format,
            reference.content_hash,
            reference
                .notes
                .filter(|value| !value.trim().is_empty())
                .map(|value| format!(" — {value}"))
                .unwrap_or_default()
        ));
        paths.push(reference.path);
    }
    let text = if lines.is_empty() {
        String::new()
    } else {
        format!(
            "ACTIVE REFERENCE IMAGES (ATTACHED AS REAL IMAGE INPUTS)\nBefore planning, drawing, masking, or rigging, visually inspect every attached image. Report only what is actually visible: subject/object type, facing direction, silhouette, canvas occupancy, transparent bounds, articulated parts, likely joints/pivots, contact points, overlaps, and any ambiguity. Never infer anatomy from the filename or user wording. Base masks and pivots on observed pixel boundaries, and preserve the requested identity/style/pose roles. Do not copy protected characters or brands.\n{}",
            lines.join("\n")
        )
    };
    Ok((text, paths))
}
