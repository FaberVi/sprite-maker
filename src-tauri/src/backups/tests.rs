use super::{create_project_backup_internal, restore_project_backup_internal, safe_slug};
use crate::{database, AppState};
use rusqlite::Connection;
use uuid::Uuid;

#[test]
fn backup_names_are_portable() {
    assert_eq!(safe_slug("Pirate Train!"), "pirate-train");
    assert_eq!(safe_slug("***"), "sprite-project");
}

#[test]
fn project_backup_restores_files_and_metadata_after_making_a_safety_copy() {
    let root = std::env::temp_dir().join(format!("sprite-studio-backup-test-{}", Uuid::new_v4()));
    let project = root.join("project");
    let backups = root.join("backups");
    std::fs::create_dir_all(project.join("assets/characters")).expect("project folders");
    let connection = database::open(&root.join("app.sqlite3")).expect("database opens");
    connection.execute(
            "INSERT INTO projects(id,name,path,created_at,last_opened_at) VALUES ('project','Test',?1,'created','opened')",
            [project.to_string_lossy().as_ref()],
        ).expect("project inserts");
    connection.execute(
            "INSERT INTO projects(id,name,path,created_at,last_opened_at) VALUES ('private-project','Private',?1,'created','opened')",
            [root.join("private-project").to_string_lossy().as_ref()],
        ).expect("other project inserts");
    connection.execute(
            "INSERT INTO provider_settings(provider,settings_json,updated_at) VALUES ('private-provider','{\"apiKey\":\"secret\"}','now')",
            [],
        ).expect("provider credentials insert");
    connection.execute(
            "INSERT INTO settings(key,value_json,updated_at) VALUES ('theme','\"dark\"','now'),('workspace-style:project','\"pixel-rpg\"','now')",
            [],
        ).expect("settings insert");
    connection.execute(
            "INSERT INTO worktrees(id,project_id,name,slug,kind,created_at,updated_at) VALUES ('general','project','General','general','general','created','created')",
            [],
        ).expect("worktree inserts");
    connection.execute(
            "INSERT INTO conversations(id,workspace_id,worktree_id,title,provider,created_at,updated_at) VALUES ('chat','project','general','Before backup','codex','created','created')",
            [],
        ).expect("chat inserts");
    connection.execute(
            "INSERT INTO rigs(id,workspace_id,worktree_id,name,spec_json,created_at,updated_at) VALUES ('rig','project','general','Before backup rig','{}','created','created')",
            [],
        ).expect("rig inserts");
    std::fs::write(project.join("assets/characters/hero.txt"), "before").expect("fixture writes");
    let state = AppState::from_connection(connection);

    let backup =
        create_project_backup_internal("project", &backups, &state).expect("backup creates");
    let backup_database =
        Connection::open(std::path::Path::new(&backup.backup_path).join("metadata.sqlite3"))
            .expect("backup database opens");
    assert_eq!(
        backup_database
            .query_row("SELECT COUNT(*) FROM projects", [], |row| row
                .get::<_, i64>(0),)
            .expect("project count reads"),
        1
    );
    assert_eq!(
        backup_database
            .query_row("SELECT COUNT(*) FROM provider_settings", [], |row| row
                .get::<_, i64>(0),)
            .expect("provider setting count reads"),
        0
    );
    assert_eq!(
        backup_database
            .query_row(
                "SELECT COUNT(*) FROM settings WHERE key='theme'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .expect("global setting count reads"),
        0
    );
    drop(backup_database);
    std::fs::write(project.join("assets/characters/hero.txt"), "after").expect("project mutates");
    state
        .db
        .lock()
        .expect("database lock")
        .execute(
            "UPDATE conversations SET title='After backup' WHERE id='chat'",
            [],
        )
        .expect("chat mutates");
    state
        .db
        .lock()
        .expect("database lock")
        .execute("UPDATE rigs SET name='After backup rig' WHERE id='rig'", [])
        .expect("rig mutates");

    restore_project_backup_internal("project", std::path::Path::new(&backup.backup_path), &state)
        .expect("backup restores");

    assert_eq!(
        std::fs::read_to_string(project.join("assets/characters/hero.txt")).expect("file reads"),
        "before"
    );
    let title: String = state
        .db
        .lock()
        .expect("database lock")
        .query_row(
            "SELECT title FROM conversations WHERE id='chat'",
            [],
            |row| row.get(0),
        )
        .expect("chat reads");
    assert_eq!(title, "Before backup");
    let (rig_name, rig_worktree): (String, String) = state
        .db
        .lock()
        .expect("database lock")
        .query_row(
            "SELECT name,worktree_id FROM rigs WHERE id='rig'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("rig reads");
    assert_eq!(rig_name, "Before backup rig");
    assert_eq!(rig_worktree, "general");
    assert!(root.join(".sprite-studio-safety-backups").is_dir());
    drop(state);
    std::fs::remove_dir_all(root).expect("fixture removes");
}
