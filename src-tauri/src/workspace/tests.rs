use super::test_support::*;
use super::*;

#[test]
fn missing_workspace_returns_a_typed_error() {
    let missing = std::env::temp_dir().join(format!("missing-sprite-workspace-{}", Uuid::new_v4()));
    let error = normalized_path(missing.to_string_lossy().as_ref(), false)
        .expect_err("missing directory must fail");
    assert_eq!(error.code, "workspace_missing");
}

#[test]
fn sidebar_snapshot_loads_projects_worktrees_and_chats_together() {
    let (root, state) = fixture();
    let project = root.join("project-a");
    std::fs::create_dir_all(&project).expect("project should exist");
    let workspace =
        register_workspace("Project A", &project, &state).expect("workspace should register");
    let other = root.join("project-b");
    std::fs::create_dir_all(&other).expect("project should exist");
    register_workspace("Project B", &other, &state).expect("workspace should register");
    {
        let connection = state.db.lock().expect("database lock");
        connection
                .execute(
                    "INSERT INTO conversations(id, workspace_id, worktree_id, title, provider, created_at, updated_at) VALUES ('c1', ?1, NULL, 'General chat', 'codex', 'now', 'now')",
                    [&workspace.id],
                )
                .expect("general chat should insert");
        connection
                .execute(
                    "INSERT INTO conversations(id, workspace_id, worktree_id, title, provider, created_at, updated_at, archived_at) VALUES ('c2', ?1, NULL, 'Archived chat', 'codex', 'now', 'now', 'archived')",
                    [&workspace.id],
                )
                .expect("archived chat should insert");
    }
    let snapshot =
        load_sidebar_state_inner(&state, Some(&workspace.id)).expect("snapshot should load");
    assert_eq!(snapshot.workspaces.len(), 2, "both projects should return");
    assert!(
        snapshot
            .worktrees
            .iter()
            .any(|worktree| worktree.kind == "general"),
        "the registered project's worktrees should be included"
    );
    assert_eq!(
        snapshot.conversations.len(),
        1,
        "only active chats of the requested project should return"
    );
    assert_eq!(snapshot.conversations[0].title, "General chat");

    let bare = load_sidebar_state_inner(&state, None).expect("bare snapshot should load");
    assert_eq!(bare.workspaces.len(), 2);
    assert!(bare.worktrees.is_empty());
    assert!(bare.conversations.is_empty());
    drop(state);
    std::fs::remove_dir_all(root).expect("temporary fixture should be removable");
}

#[test]
fn duplicate_workspace_open_reuses_the_registration() {
    let (root, state) = fixture();
    let project = root.join("project");
    std::fs::create_dir_all(&project).expect("project should exist");
    let first = register_workspace("Test", &project, &state).expect("workspace should register");
    let second = register_workspace("Test again", &project, &state)
        .expect("duplicate open should be graceful");
    assert_eq!(first.id, second.id);
    let count: i64 = state
        .db
        .lock()
        .expect("database lock")
        .query_row("SELECT COUNT(*) FROM projects", [], |row| row.get(0))
        .expect("count should work");
    assert_eq!(count, 1);
    let worktree_count: i64 = state
        .db
        .lock()
        .expect("database lock")
        .query_row(
            "SELECT COUNT(*) FROM worktrees WHERE project_id = ?1",
            [&first.id],
            |row| row.get(0),
        )
        .expect("default worktree count should work");
    assert_eq!(worktree_count, 1);
    drop(state);
    std::fs::remove_dir_all(root).expect("temporary fixture should be removable");
}

#[test]
fn initialization_installs_the_codex_sprite_renderers() {
    let root = std::env::temp_dir().join(format!("sprite-studio-renderer-test-{}", Uuid::new_v4()));
    initialize_workspace(&root).expect("workspace should initialize");
    for &(filename, _) in super::bundled::BUNDLED_PYTHON {
        let installed = root.join(".sprite-studio").join(filename);
        assert!(
            installed.is_file(),
            "{filename} should be installed into .sprite-studio"
        );
    }
    let tool = root.join(".sprite-studio/sprite_tool.py");
    assert!(std::fs::read_to_string(tool)
        .expect("tool should be readable")
        .contains("Small dependency-free pixel sprite renderer"));
    let rig_tool = root.join(".sprite-studio/sprite_rig.py");
    assert!(std::fs::read_to_string(&rig_tool)
        .expect("rig tool should be readable")
        .contains("layered pixel-rig renderer"));
    let rig_mcp = root.join(".sprite-studio/sprite_rig_mcp.py");
    assert!(std::fs::read_to_string(rig_mcp)
        .expect("rig MCP server should be readable")
        .contains("Workspace-scoped MCP server"));
    let polish_tool = root.join(".sprite-studio/sprite_polish.py");
    assert!(std::fs::read_to_string(polish_tool)
        .expect("polish tool should be readable")
        .contains("Normalize one ImageGen frame repair"));
    let terrain_cleanup = root.join(".sprite-studio/terrain_cleanup.py");
    assert!(std::fs::read_to_string(terrain_cleanup)
        .expect("terrain cleanup tool should be readable")
        .contains("Remove accidental magenta chroma fringe"));
    assert!(root.join(".sprite-studio/packs").is_dir());
    assert!(root.join("assets/characters").is_dir());
    assert!(root.join("assets/creatures").is_dir());
    std::fs::remove_dir_all(root).expect("temporary fixture should be removable");
}
