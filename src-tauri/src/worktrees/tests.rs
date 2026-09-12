use super::{delete_worktree_record, slug, validate_kind};
use crate::database;
use rusqlite::params;

#[test]
fn validates_explicit_worktree_types() {
    for kind in [
        "general",
        "character",
        "environment",
        "creature",
        "object",
        "tileset",
        "animation",
        "vfx",
        "ui",
    ] {
        assert_eq!(validate_kind(kind).expect("kind should be valid"), kind);
    }
    assert!(validate_kind("unknown").is_err());
}

#[test]
fn creates_portable_worktree_slugs() {
    assert_eq!(slug("Knight / One-Handed"), "knight-one-handed");
    assert_eq!(slug("  Fire   Magic  "), "fire-magic");
}

#[test]
fn deleting_a_worktree_moves_content_and_rigs_to_general() {
    let path = std::env::temp_dir().join(format!(
        "sprite-studio-worktree-delete-{}.sqlite3",
        uuid::Uuid::new_v4()
    ));
    let mut connection = database::open(&path).expect("database should open");
    connection.execute(
            "INSERT INTO projects(id, name, path, created_at, last_opened_at) VALUES ('p1','Test','test','now','now')",
            [],
        ).expect("project should insert");
    for (id, name, slug, kind) in [
        ("general", "General", "general", "general"),
        ("terrain", "Terrain", "terrain", "tileset"),
    ] {
        connection.execute(
                "INSERT INTO worktrees(id, project_id, name, slug, kind, created_at, updated_at) VALUES (?1,'p1',?2,?3,?4,'now','now')",
                params![id, name, slug, kind],
            ).expect("worktree should insert");
    }
    connection.execute(
            "INSERT INTO conversations(id, workspace_id, worktree_id, title, provider, created_at, updated_at) VALUES ('c1','p1','terrain','Chat','codex','now','now')",
            [],
        ).expect("conversation should insert");
    connection.execute(
            "INSERT INTO messages(id, conversation_id, role, content, status, created_at) VALUES ('m1','c1','user','hello','completed','now')",
            [],
        ).expect("message should insert");
    connection.execute(
            "INSERT INTO assets(id, workspace_id, name, path, relative_path, category, format, width, height, file_size, created_at) VALUES ('a1','p1','Tile','tile.png','assets/terrain/tile.png','terrain','png',1,1,1,'now')",
            [],
        ).expect("asset should insert");
    connection.execute(
            "INSERT INTO asset_worktrees(asset_id, worktree_id, relationship, created_at) VALUES ('a1','terrain','owned','now')",
            [],
        ).expect("asset link should insert");
    connection.execute(
            "INSERT INTO rigs(id, workspace_id, worktree_id, asset_id, name, spec_json, created_at, updated_at) VALUES ('r1','p1','terrain','a1','Tile rig','{}','now','now')",
            [],
        ).expect("rig should insert");

    delete_worktree_record(&mut connection, "terrain").expect("delete should succeed");

    let chat_worktree: String = connection
        .query_row(
            "SELECT worktree_id FROM conversations WHERE id='c1'",
            [],
            |row| row.get(0),
        )
        .expect("conversation should remain");
    let rig_worktree: String = connection
        .query_row("SELECT worktree_id FROM rigs WHERE id='r1'", [], |row| {
            row.get(0)
        })
        .expect("rig should remain");
    assert_eq!(chat_worktree, "general");
    assert_eq!(rig_worktree, "general");
    assert_eq!(
        connection
            .query_row(
                "SELECT COUNT(*) FROM messages WHERE conversation_id='c1'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .expect("message count should work"),
        1
    );
    assert_eq!(connection.query_row(
            "SELECT COUNT(*) FROM asset_worktrees WHERE asset_id='a1' AND worktree_id='general'", [], |row| row.get::<_, i64>(0),
        ).expect("asset link count should work"), 1);
    assert_eq!(
        connection
            .query_row(
                "SELECT COUNT(*) FROM worktrees WHERE id='terrain'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .expect("worktree count should work"),
        0
    );
    drop(connection);
    let _ = std::fs::remove_file(path);
}
