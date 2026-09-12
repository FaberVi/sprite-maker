use crate::{
    error::{CommandError, CommandResult},
    models::{ProjectBackup, Workspace},
    workspace::workspace_path,
    AppState,
};
use chrono::Utc;
use rusqlite::{backup::Backup, Connection, OptionalExtension};
use std::{path::Path, time::Duration};
use tauri::State;
use uuid::Uuid;

pub(super) const BACKUP_FORMAT_VERSION: u32 = 1;

pub(super) fn safe_slug(value: &str) -> String {
    let slug: String = value
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
    if slug.is_empty() {
        "sprite-project".into()
    } else {
        slug.into()
    }
}

pub(super) fn copy_tree(source: &Path, destination: &Path) -> CommandResult<(u64, u64)> {
    std::fs::create_dir_all(destination)?;
    let mut file_count = 0;
    let mut total_bytes = 0;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        let metadata = std::fs::symlink_metadata(&source_path)?;
        if metadata.file_type().is_symlink() {
            return Err(CommandError::new(
                "backup_symlink",
                format!(
                    "Backups do not follow symbolic links: {}",
                    source_path.display()
                ),
            ));
        }
        if metadata.is_dir() {
            let (nested_count, nested_bytes) = copy_tree(&source_path, &destination_path)?;
            file_count += nested_count;
            total_bytes += nested_bytes;
        } else if metadata.is_file() {
            std::fs::copy(&source_path, &destination_path)?;
            file_count += 1;
            total_bytes += metadata.len();
        }
    }
    Ok((file_count, total_bytes))
}

pub(super) fn workspace_record(state: &AppState, project_id: &str) -> CommandResult<Workspace> {
    let connection = state
        .db
        .lock()
        .map_err(|_| CommandError::new("database_locked", "Database lock was poisoned"))?;
    connection
        .query_row(
            "SELECT id,name,path,created_at,last_opened_at FROM projects WHERE id=?1",
            [project_id],
            |row| {
                Ok(Workspace {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    path: row.get(2)?,
                    created_at: row.get(3)?,
                    last_opened_at: row.get(4)?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| {
            CommandError::new("workspace_not_found", "Workspace is no longer registered")
        })
}

fn create_database_snapshot(
    state: &AppState,
    destination: &Path,
    project_id: &str,
) -> CommandResult<()> {
    let source = state
        .db
        .lock()
        .map_err(|_| CommandError::new("database_locked", "Database lock was poisoned"))?;
    let mut target = Connection::open(destination)?;
    let backup = Backup::new(&source, &mut target)?;
    backup.run_to_completion(32, Duration::from_millis(5), None)?;
    drop(backup);
    target.execute_batch("PRAGMA foreign_keys=ON; PRAGMA secure_delete=ON;")?;
    target.execute("DELETE FROM projects WHERE id<>?1", [project_id])?;
    target.execute("DELETE FROM provider_settings", [])?;
    target.execute(
        "DELETE FROM settings
         WHERE key NOT IN ('workspace-style:'||?1,'active-worktree:'||?1)
           AND NOT EXISTS (
             SELECT 1 FROM conversations c
             WHERE c.workspace_id=?1 AND settings.key LIKE '%:'||c.id
           )",
        [project_id],
    )?;
    target.execute_batch("VACUUM")?;
    Ok(())
}

pub(super) fn create_project_backup_internal(
    project_id: &str,
    destination_directory: &Path,
    state: &AppState,
) -> CommandResult<ProjectBackup> {
    let workspace = workspace_record(state, project_id)?;
    let root = workspace_path(state, project_id)?;
    let busy: bool = state.db.lock().map_err(|_| CommandError::new("database_locked", "Database lock was poisoned"))?.query_row(
        "SELECT EXISTS(
           SELECT 1 FROM conversations c JOIN messages m ON m.conversation_id=c.id
           WHERE c.workspace_id=?1 AND m.status='running'
           UNION ALL
           SELECT 1 FROM background_jobs WHERE project_id=?1 AND status IN ('queued','running','analyzing')
         )",
        [project_id],
        |row| row.get(0),
    )?;
    if busy {
        return Err(CommandError::new(
            "project_busy",
            "Wait for active generation and background jobs to finish before creating a backup",
        ));
    }
    std::fs::create_dir_all(destination_directory)?;
    let destination_directory = destination_directory.canonicalize()?;
    if destination_directory.starts_with(&root) {
        return Err(CommandError::new(
            "unsafe_backup_destination",
            "Choose a backup destination outside the project folder",
        ));
    }
    let timestamp = Utc::now();
    let label = format!(
        "{}-{}.sprite-studio-backup",
        safe_slug(&workspace.name),
        timestamp.format("%Y%m%d-%H%M%S")
    );
    let mut final_path = destination_directory.join(&label);
    if final_path.exists() {
        let suffix = Uuid::new_v4().simple().to_string();
        final_path = destination_directory.join(format!(
            "{}-{}.sprite-studio-backup",
            label.trim_end_matches(".sprite-studio-backup"),
            &suffix[..8]
        ));
    }
    let staging = destination_directory.join(format!(
        ".sprite-studio-backup-{}.incomplete",
        Uuid::new_v4()
    ));
    std::fs::create_dir_all(&staging)?;
    let result = (|| -> CommandResult<ProjectBackup> {
        let (file_count, total_bytes) = copy_tree(&root, &staging.join("workspace"))?;
        create_database_snapshot(state, &staging.join("metadata.sqlite3"), project_id)?;
        let backup = ProjectBackup {
            format_version: BACKUP_FORMAT_VERSION,
            project_id: workspace.id,
            project_name: workspace.name,
            source_path: workspace.path,
            backup_path: final_path.to_string_lossy().into_owned(),
            created_at: timestamp.to_rfc3339(),
            file_count,
            total_bytes,
        };
        std::fs::write(
            staging.join("manifest.json"),
            serde_json::to_vec_pretty(&backup)
                .map_err(|error| CommandError::new("serialization_error", error.to_string()))?,
        )?;
        std::fs::rename(&staging, &final_path)?;
        Ok(backup)
    })();
    if result.is_err() && staging.exists() {
        let _ = std::fs::remove_dir_all(&staging);
    }
    result
}

#[tauri::command]
pub fn create_project_backup(
    project_id: String,
    destination_directory: String,
    state: State<'_, AppState>,
) -> CommandResult<ProjectBackup> {
    create_project_backup_internal(&project_id, Path::new(&destination_directory), &state)
}
