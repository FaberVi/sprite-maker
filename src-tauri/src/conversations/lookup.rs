use crate::{
    error::{CommandError, CommandResult},
    models::Conversation,
    AppState,
};
use rusqlite::OptionalExtension;

pub(crate) fn conversation_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Conversation> {
    Ok(Conversation {
        id: row.get(0)?,
        workspace_id: row.get(1)?,
        worktree_id: row.get(2)?,
        title: row.get(3)?,
        provider: row.get(4)?,
        provider_session_id: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
        archived_at: row.get(8)?,
    })
}

pub fn get_conversation(state: &AppState, id: &str) -> CommandResult<Conversation> {
    let connection = state
        .db
        .lock()
        .map_err(|_| CommandError::new("database_locked", "Database lock was poisoned"))?;
    connection.query_row(
        "SELECT id, workspace_id, worktree_id, title, provider, provider_session_id, created_at, updated_at, archived_at FROM conversations WHERE id = ?1 AND archived_at IS NULL",
        [id], conversation_row
    ).optional()?.ok_or_else(|| CommandError::new("conversation_not_found", "Conversation no longer exists"))
}
