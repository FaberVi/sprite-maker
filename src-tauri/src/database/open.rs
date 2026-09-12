use super::migrate::migrate;
use crate::error::CommandResult;
use rusqlite::Connection;
use std::path::Path;

pub fn open(path: &Path) -> CommandResult<Connection> {
    open_with_recovery(path, true)
}

/// Opens the same SQLite file as the desktop app without cancelling in-flight
/// GUI work. Used by the headless MCP server that may share the DB via WAL.
pub fn open_shared(path: &Path) -> CommandResult<Connection> {
    open_with_recovery(path, false)
}

pub(crate) fn open_with_recovery(
    path: &Path,
    recover_interrupted: bool,
) -> CommandResult<Connection> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut connection = Connection::open(path)?;
    connection.busy_timeout(std::time::Duration::from_secs(5))?;
    connection.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")?;
    migrate(&mut connection)?;
    if recover_interrupted {
        connection.execute(
            "UPDATE messages SET status = 'cancelled' WHERE status = 'running'",
            [],
        )?;
        connection.execute(
            "UPDATE background_jobs SET status='failed', stage='Interrupted', error_message='The application closed before this job finished', completed_at=datetime('now'), updated_at=datetime('now') WHERE status IN ('queued','running','analyzing')",
            [],
        )?;
    }
    Ok(connection)
}
