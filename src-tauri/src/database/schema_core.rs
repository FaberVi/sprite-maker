use crate::error::{CommandError, CommandResult};
use rusqlite::{Connection, Transaction};

pub(crate) fn migrate_v1(connection: &mut Connection) -> CommandResult<()> {
    let transaction = connection.transaction()?;
    transaction.execute_batch(
            r#"
        CREATE TABLE workspaces (
          id TEXT PRIMARY KEY,
          name TEXT NOT NULL,
          path TEXT NOT NULL UNIQUE,
          created_at TEXT NOT NULL,
          last_opened_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_workspaces_last_opened ON workspaces(last_opened_at DESC);

        CREATE TABLE conversations (
          id TEXT PRIMARY KEY,
          workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
          title TEXT NOT NULL,
          provider TEXT NOT NULL DEFAULT 'codex',
          provider_session_id TEXT,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_conversations_workspace ON conversations(workspace_id, updated_at DESC);

        CREATE TABLE messages (
          id TEXT PRIMARY KEY,
          conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
          role TEXT NOT NULL,
          kind TEXT NOT NULL DEFAULT 'text',
          content TEXT NOT NULL DEFAULT '',
          status TEXT NOT NULL DEFAULT 'completed',
          metadata_json TEXT NOT NULL DEFAULT '{}',
          created_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_messages_conversation ON messages(conversation_id, created_at);

        CREATE TABLE generations (
          id TEXT PRIMARY KEY,
          workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
          conversation_id TEXT REFERENCES conversations(id) ON DELETE SET NULL,
          prompt TEXT NOT NULL,
          provider TEXT NOT NULL,
          source_asset_id TEXT,
          operation TEXT NOT NULL,
          status TEXT NOT NULL,
          created_at TEXT NOT NULL
        );

        CREATE TABLE assets (
          id TEXT PRIMARY KEY,
          workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
          name TEXT NOT NULL,
          path TEXT NOT NULL UNIQUE,
          relative_path TEXT NOT NULL,
          category TEXT NOT NULL,
          format TEXT NOT NULL,
          width INTEGER NOT NULL,
          height INTEGER NOT NULL,
          file_size INTEGER NOT NULL,
          has_alpha INTEGER NOT NULL DEFAULT 0,
          generation_id TEXT REFERENCES generations(id) ON DELETE SET NULL,
          created_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_assets_workspace ON assets(workspace_id, category, name);

        CREATE TABLE animations (
          id TEXT PRIMARY KEY,
          workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
          name TEXT NOT NULL,
          fps REAL NOT NULL,
          looping INTEGER NOT NULL DEFAULT 1,
          frames_json TEXT NOT NULL DEFAULT '[]',
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_animations_workspace ON animations(workspace_id, updated_at DESC);

        CREATE TABLE settings (
          key TEXT PRIMARY KEY,
          value_json TEXT NOT NULL,
          updated_at TEXT NOT NULL
        );

        CREATE TABLE provider_settings (
          provider TEXT PRIMARY KEY,
          enabled INTEGER NOT NULL DEFAULT 1,
          settings_json TEXT NOT NULL DEFAULT '{}',
          updated_at TEXT NOT NULL
        );

        INSERT INTO migrations(version, applied_at) VALUES (1, datetime('now'));
        "#,
        )
        .map_err(|error| CommandError::new("migration_failed", error.to_string()))?;
    transaction.commit()?;
    Ok(())
}

pub(crate) fn migrate_v2(transaction: &Transaction<'_>) -> CommandResult<()> {
    transaction
        .execute_batch(
            r#"
        ALTER TABLE workspaces RENAME TO projects;

        CREATE TABLE worktrees (
          id TEXT PRIMARY KEY,
          project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
          name TEXT NOT NULL,
          slug TEXT NOT NULL,
          kind TEXT NOT NULL CHECK(kind IN ('general','character','environment','creature','object','tileset','animation','vfx','ui')),
          description TEXT,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL,
          UNIQUE(project_id, slug)
        );
        CREATE INDEX idx_worktrees_project ON worktrees(project_id, updated_at DESC);

        ALTER TABLE conversations ADD COLUMN worktree_id TEXT REFERENCES worktrees(id) ON DELETE CASCADE;
        ALTER TABLE generations ADD COLUMN worktree_id TEXT REFERENCES worktrees(id) ON DELETE SET NULL;
        ALTER TABLE animations ADD COLUMN worktree_id TEXT REFERENCES worktrees(id) ON DELETE SET NULL;

        CREATE TABLE asset_worktrees (
          asset_id TEXT NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
          worktree_id TEXT NOT NULL REFERENCES worktrees(id) ON DELETE CASCADE,
          relationship TEXT NOT NULL DEFAULT 'owned' CHECK(relationship IN ('owned','referenced')),
          created_at TEXT NOT NULL,
          PRIMARY KEY(asset_id, worktree_id)
        );
        CREATE INDEX idx_asset_worktrees_worktree ON asset_worktrees(worktree_id, relationship);

        CREATE TABLE asset_versions (
          id TEXT PRIMARY KEY,
          asset_id TEXT NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
          version_number INTEGER NOT NULL CHECK(version_number > 0),
          parent_version_id TEXT REFERENCES asset_versions(id) ON DELETE SET NULL,
          generation_id TEXT REFERENCES generations(id) ON DELETE SET NULL,
          path TEXT NOT NULL,
          format TEXT NOT NULL,
          width INTEGER NOT NULL,
          height INTEGER NOT NULL,
          file_size INTEGER NOT NULL,
          has_alpha INTEGER NOT NULL DEFAULT 0,
          content_hash TEXT NOT NULL,
          change_kind TEXT NOT NULL DEFAULT 'imported',
          available INTEGER NOT NULL DEFAULT 1,
          selected INTEGER NOT NULL DEFAULT 0,
          created_at TEXT NOT NULL,
          UNIQUE(asset_id, version_number)
        );
        CREATE INDEX idx_asset_versions_asset ON asset_versions(asset_id, version_number DESC);

        CREATE TABLE animation_frames (
          id TEXT PRIMARY KEY,
          animation_id TEXT NOT NULL REFERENCES animations(id) ON DELETE CASCADE,
          asset_id TEXT NOT NULL REFERENCES assets(id) ON DELETE RESTRICT,
          position INTEGER NOT NULL CHECK(position >= 0),
          duration_ms INTEGER CHECK(duration_ms IS NULL OR duration_ms > 0),
          offset_x INTEGER NOT NULL DEFAULT 0,
          offset_y INTEGER NOT NULL DEFAULT 0,
          pivot_x REAL,
          pivot_y REAL,
          created_at TEXT NOT NULL,
          UNIQUE(animation_id, position)
        );
        CREATE INDEX idx_animation_frames_animation ON animation_frames(animation_id, position);

        INSERT INTO worktrees(id, project_id, name, slug, kind, description, created_at, updated_at)
        SELECT lower(hex(randomblob(16))), id, 'General', 'general', 'general',
               'Migrated project-level assets and conversations', created_at, last_opened_at
        FROM projects;

        INSERT INTO asset_versions(
          id, asset_id, version_number, path, format, width, height, file_size,
          has_alpha, content_hash, change_kind, selected, created_at
        )
        SELECT lower(hex(randomblob(16))), id, 1, path, format, width, height, file_size,
               has_alpha, '', 'migrated', 1, created_at
        FROM assets;

        INSERT INTO asset_worktrees(asset_id, worktree_id, relationship, created_at)
        SELECT a.id, w.id, 'owned', a.created_at
        FROM assets a
        JOIN worktrees w ON w.project_id = a.workspace_id AND w.kind = 'general';

        INSERT INTO migrations(version, applied_at) VALUES (2, datetime('now'));
        "#,
        )
        .map_err(|error| CommandError::new("migration_failed", error.to_string()))?;

    backfill_animation_frames(transaction, false)
}

pub(crate) fn migrate_v3(transaction: &Transaction<'_>) -> CommandResult<()> {
    backfill_animation_frames(transaction, true)?;
    transaction
        .execute_batch(
            r#"
        UPDATE animations
        SET worktree_id = (
          SELECT aw.worktree_id
          FROM animation_frames af
          JOIN asset_worktrees aw ON aw.asset_id = af.asset_id
          JOIN worktrees w ON w.id = aw.worktree_id
          WHERE af.animation_id = animations.id AND w.kind <> 'general'
          GROUP BY aw.worktree_id
          HAVING COUNT(DISTINCT af.position) = (
            SELECT COUNT(*) FROM animation_frames all_frames
            WHERE all_frames.animation_id = animations.id
          )
          ORDER BY MIN(aw.created_at)
          LIMIT 1
        )
        WHERE worktree_id IS NULL
          AND EXISTS (
            SELECT 1 FROM animation_frames af
            WHERE af.animation_id = animations.id
          );

        INSERT INTO migrations(version, applied_at) VALUES (3, datetime('now'));
        "#,
        )
        .map_err(|error| CommandError::new("migration_failed", error.to_string()))?;
    Ok(())
}

fn backfill_animation_frames(
    transaction: &Transaction<'_>,
    only_missing: bool,
) -> CommandResult<()> {
    let query = if only_missing {
        r#"SELECT id, frames_json, created_at
           FROM animations
           WHERE frames_json <> '[]'
             AND NOT EXISTS (
               SELECT 1 FROM animation_frames
               WHERE animation_frames.animation_id = animations.id
             )"#
    } else {
        "SELECT id, frames_json, created_at FROM animations WHERE frames_json <> '[]'"
    };
    let mut statement = transaction.prepare(query)?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;
    for row in rows {
        let (animation_id, frames_json, created_at) = row?;
        let frames: Vec<crate::models::AnimationFrame> =
            serde_json::from_str(&frames_json).unwrap_or_default();
        for (position, frame) in frames.into_iter().enumerate() {
            transaction.execute(
                r#"INSERT INTO animation_frames(
                    id, animation_id, asset_id, position, duration_ms, created_at
                )
                SELECT ?1, ?2, assets.id, ?4, ?5, ?6
                FROM assets
                WHERE assets.id = ?3"#,
                rusqlite::params![
                    uuid::Uuid::new_v4().to_string(),
                    animation_id,
                    frame.asset_id,
                    position as i64,
                    frame.duration_ms,
                    created_at
                ],
            )?;
        }
    }
    Ok(())
}
