use super::test_support::*;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

#[test]
fn bundled_rig_mcp_rejects_invalid_fps_without_creating_artifacts() {
    let fixture = RigRendererFixture::new();
    let cases = [
        ("text", json!("bad")),
        ("boolean", json!(true)),
        ("zero", json!(0)),
        ("above-limit", json!(61)),
    ];
    let mut mcp = RigMcpSession::new(&fixture.root);
    mcp.initialize_legacy();

    for (index, (case, fps)) in cases.into_iter().enumerate() {
        let mut rig = good_biped_walk();
        rig["name"] = json!(format!("invalid_fps_{case}"));
        rig["fps"] = fps;
        let rig_name = format!("invalid-fps-{case}");
        let (rig_path, rig_bytes) = fixture.write_rig(&rig_name, &rig);
        let response = mcp.call_tool(
            index as u64 + 1,
            "sprite_rig_validate",
            &format!(".sprite-studio/rigs/{rig_name}.json"),
        );
        assert_eq!(
            response["result"]["isError"], true,
            "invalid fps case {case} must fail validation: {response}"
        );
        assert!(response["result"]["structuredContent"]["error"]
            .as_str()
            .expect("fps validation error should be text")
            .contains("fps"));
        assert_eq!(
            std::fs::read(&rig_path).expect("invalid rig should remain readable"),
            rig_bytes,
            "invalid fps validation must not rewrite the rig"
        );
        assert!(!fixture
            .root
            .join(format!("assets/characters/invalid_fps_{case}_01.png"))
            .exists());
    }
    assert!(!fixture
        .root
        .join(".sprite-studio/last-generation.json")
        .exists());
    assert!(!fixture.root.join(".sprite-studio/versions").exists());
}

#[test]
fn bundled_rig_mcp_rejects_nonfinite_json_and_invalid_master_hashes_without_artifacts() {
    let fixture = RigRendererFixture::new();
    let mut cases = vec![("top-level-nan", b"NaN\n".to_vec(), None)];
    for (case, master_hash) in [
        ("false", json!(false)),
        ("empty", json!("")),
        ("malformed", json!("not-a-sha256")),
    ] {
        let mut rig = good_biped_walk();
        let output_name = format!("invalid_master_hash_{case}");
        rig["name"] = json!(output_name.clone());
        rig["masterHash"] = master_hash;
        cases.push((
            case,
            serde_json::to_vec(&rig).expect("invalid hash rig should serialize"),
            Some(output_name),
        ));
    }

    let mut mcp = RigMcpSession::new(&fixture.root);
    mcp.initialize_legacy();
    for (index, (case, rig_bytes, output_name)) in cases.into_iter().enumerate() {
        let rig_name = format!("invalid-master-{case}");
        let rig_path = fixture
            .root
            .join(format!(".sprite-studio/rigs/{rig_name}.json"));
        std::fs::write(&rig_path, &rig_bytes).expect("invalid rig should write");

        let response = mcp.call_tool(
            index as u64 + 1,
            "sprite_rig_validate",
            &format!(".sprite-studio/rigs/{rig_name}.json"),
        );
        assert_eq!(
            response["result"]["isError"], true,
            "invalid {case} case must fail validation: {response}"
        );
        let diagnostic = response["result"]["structuredContent"]["error"]
            .as_str()
            .expect("validation diagnostic should be text");
        assert!(
            diagnostic.starts_with("sprite_rig: "),
            "invalid {case} should return a clean renderer diagnostic: {diagnostic}"
        );
        assert!(
            !diagnostic.contains("Traceback") && !diagnostic.contains('\n'),
            "invalid {case} should not expose a Python traceback: {diagnostic}"
        );
        if case == "top-level-nan" {
            assert!(diagnostic.contains("non-finite JSON constant"));
        } else {
            assert!(diagnostic.contains("masterHash"));
        }
        assert_eq!(
            std::fs::read(&rig_path).expect("invalid rig should remain readable"),
            rig_bytes,
            "invalid {case} validation must not rewrite the rig"
        );
        if let Some(output_name) = output_name {
            assert!(!fixture
                .root
                .join(format!("assets/characters/{output_name}_01.png"))
                .exists());
        }
    }

    let asset_names = std::fs::read_dir(fixture.root.join("assets/characters"))
        .expect("character assets should remain readable")
        .map(|entry| {
            entry
                .expect("asset directory entry should be readable")
                .file_name()
                .into_string()
                .expect("test asset names should be UTF-8")
        })
        .collect::<HashSet<_>>();
    assert_eq!(asset_names, HashSet::from(["walker.png".to_string()]));
    assert!(!fixture
        .root
        .join(".sprite-studio/last-generation.json")
        .exists());
    assert!(!fixture.root.join(".sprite-studio/versions").exists());
    assert!(!fixture.root.join(".sprite-studio/transactions").exists());
}

#[test]
fn bundled_rig_mcp_rejects_invalid_v1_colors_without_creating_artifacts() {
    let fixture = RigRendererFixture::new();
    let cases = [
        {
            let mut rig = legacy_color_rig();
            rig["palette"]["accent"] = json!("not-a-color");
            ("palette", rig)
        },
        {
            let mut rig = legacy_color_rig();
            rig["frames"][0]["overlay"][0]["color"] = json!("#gg0000");
            ("command", rig)
        },
    ];
    let mut mcp = RigMcpSession::new(&fixture.root);
    mcp.initialize_legacy();

    for (index, (case, mut rig)) in cases.into_iter().enumerate() {
        rig["name"] = json!(format!("invalid_v1_{case}"));
        let rig_name = format!("invalid-v1-{case}");
        let (rig_path, rig_bytes) = fixture.write_rig(&rig_name, &rig);
        let response = mcp.call_tool(
            index as u64 + 1,
            "sprite_rig_validate",
            &format!(".sprite-studio/rigs/{rig_name}.json"),
        );
        assert_eq!(
            response["result"]["isError"], true,
            "invalid v1 {case} color must fail validation: {response}"
        );
        assert!(response["result"]["structuredContent"]["error"]
            .as_str()
            .expect("color validation error should be text")
            .contains("color"));
        assert_eq!(
            std::fs::read(&rig_path).expect("invalid v1 rig should remain readable"),
            rig_bytes,
            "invalid v1 color validation must not rewrite the rig"
        );
        assert!(!fixture
            .root
            .join(format!("assets/characters/invalid_v1_{case}_01.png"))
            .exists());
    }
    assert!(!fixture
        .root
        .join(".sprite-studio/last-generation.json")
        .exists());
    assert!(!fixture.root.join(".sprite-studio/versions").exists());
}

#[test]
fn bundled_rig_mcp_preflights_every_output_before_changing_active_state() {
    let fixture = RigRendererFixture::new();
    let mut mcp = RigMcpSession::new(&fixture.root);
    mcp.initialize_legacy();

    let mut active_rig = ik_biped_walk();
    active_rig["name"] = json!("prior_active_walk");
    fixture.write_rig("prior-active-walk", &active_rig);
    let activated = mcp.call_tool(
        1,
        "sprite_rig_render",
        ".sprite-studio/rigs/prior-active-walk.json",
    );
    assert_eq!(activated["result"]["isError"], false);
    let active_files = activated["result"]["structuredContent"]["files"]
        .as_array()
        .expect("initial render should list active files")
        .iter()
        .map(|value| {
            fixture
                .root
                .join(value.as_str().expect("active file should be text"))
        })
        .collect::<Vec<_>>();
    let active_bytes = active_files
        .iter()
        .map(|path| std::fs::read(path).expect("active frame should be readable"))
        .collect::<Vec<_>>();
    let manifest_path = fixture.root.join(".sprite-studio/last-generation.json");
    let manifest_bytes = std::fs::read(&manifest_path).expect("active manifest should be readable");

    let mut colliding_rig = ik_biped_walk();
    colliding_rig["name"] = json!("collision_candidate");
    fixture.write_rig("collision-candidate", &colliding_rig);
    let collision = fixture
        .root
        .join("assets/characters/collision_candidate_02.png");
    std::fs::create_dir(&collision).expect("frame-two collision directory should exist");
    let rejected = mcp.call_tool(
        2,
        "sprite_rig_render",
        ".sprite-studio/rigs/collision-candidate.json",
    );
    assert_eq!(rejected["result"]["isError"], true);
    assert!(rejected["result"]["structuredContent"]["error"]
        .as_str()
        .expect("collision error should be text")
        .contains("output frame 2 must be a regular file"));

    assert!(!fixture
        .root
        .join("assets/characters/collision_candidate_01.png")
        .exists());
    assert!(collision.is_dir());
    assert!(!fixture
        .root
        .join("assets/characters/collision_candidate_03.png")
        .exists());
    assert_eq!(
        std::fs::read(&manifest_path).expect("prior manifest should survive"),
        manifest_bytes,
        "preflight failure must preserve the prior active manifest"
    );
    for (path, expected) in active_files.iter().zip(active_bytes) {
        assert_eq!(
            std::fs::read(path).expect("prior active frame should survive"),
            expected,
            "preflight failure must preserve every prior active frame"
        );
    }
    assert!(!fixture.root.join(".sprite-studio/versions").exists());
}

#[test]
fn bundled_rig_mcp_rejects_a_forged_extra_active_frame_without_mutation() {
    let fixture = RigRendererFixture::new();
    let mut mcp = RigMcpSession::new(&fixture.root);
    mcp.initialize_legacy();

    let mut rig = ik_biped_walk();
    rig["name"] = json!("forged_manifest_walk");
    let (rig_path, _) = fixture.write_rig("forged-manifest-walk", &rig);
    let first = mcp.call_tool(
        1,
        "sprite_rig_render",
        ".sprite-studio/rigs/forged-manifest-walk.json",
    );
    assert_eq!(first["result"]["isError"], false);
    let active_paths = first["result"]["structuredContent"]["files"]
        .as_array()
        .expect("initial render should list active files")
        .iter()
        .map(|value| {
            fixture
                .root
                .join(value.as_str().expect("active frame should be text"))
        })
        .collect::<Vec<_>>();
    let active_bytes = active_paths
        .iter()
        .map(|path| std::fs::read(path).expect("active frame should be readable"))
        .collect::<Vec<_>>();

    let manifest_path = fixture.root.join(".sprite-studio/last-generation.json");
    let mut forged_manifest: Value = serde_json::from_slice(
        &std::fs::read(&manifest_path).expect("active manifest should be readable"),
    )
    .expect("active manifest should be JSON");
    forged_manifest["files"]
        .as_array_mut()
        .expect("active manifest files should be an array")
        .push(json!("assets/characters/forged_manifest_walk_99.png"));
    let forged_manifest_bytes =
        serde_json::to_vec_pretty(&forged_manifest).expect("forged manifest should serialize");
    std::fs::write(&manifest_path, &forged_manifest_bytes).expect("forged manifest should write");
    let sentinel_path = fixture
        .root
        .join("assets/characters/forged_manifest_walk_99.png");
    let sentinel_bytes = b"forged frame-list sentinel".to_vec();
    std::fs::write(&sentinel_path, &sentinel_bytes).expect("sentinel should write");
    let active_rig_bytes =
        std::fs::read(&rig_path).expect("active normalized rig should be readable");

    let rejected = mcp.call_tool(
        2,
        "sprite_rig_render",
        ".sprite-studio/rigs/forged-manifest-walk.json",
    );
    assert_eq!(rejected["result"]["isError"], true);
    let diagnostic = rejected["result"]["structuredContent"]["error"]
        .as_str()
        .expect("forged-manifest error should be text");
    assert!(
        diagnostic.contains("active rig manifest frames must be the exact ordered output sequence"),
        "forged manifest should fail exact sequence validation: {diagnostic}"
    );
    assert!(!diagnostic.contains("Traceback"));

    assert_eq!(
        std::fs::read(&sentinel_path).expect("sentinel should survive rejection"),
        sentinel_bytes,
        "the unowned sentinel must not be archived or replaced"
    );
    assert_eq!(
        std::fs::read(&manifest_path).expect("forged manifest should survive rejection"),
        forged_manifest_bytes,
        "manifest validation failure must not rewrite active metadata"
    );
    assert_eq!(
        std::fs::read(&rig_path).expect("active rig should survive rejection"),
        active_rig_bytes,
        "manifest validation failure must not rewrite the active rig"
    );
    for (path, expected) in active_paths.iter().zip(active_bytes) {
        assert_eq!(
            std::fs::read(path).expect("active frame should survive rejection"),
            expected,
            "manifest validation failure must preserve every active frame"
        );
    }
    assert!(!fixture.root.join(".sprite-studio/versions").exists());
    assert_eq!(
        std::fs::read_dir(fixture.root.join(".sprite-studio/transactions"))
            .expect("transaction root should remain readable")
            .count(),
        0,
        "forged manifest rejection must not retain a transaction"
    );
}

#[test]
fn bundled_rig_mcp_rerender_archives_the_exact_prior_active_frames() {
    let fixture = RigRendererFixture::new();
    let mut mcp = RigMcpSession::new(&fixture.root);
    mcp.initialize_legacy();

    let mut rig = ik_biped_walk();
    rig["name"] = json!("archive_walk");
    let (rig_path, _) = fixture.write_rig("archive-walk", &rig);
    let first = mcp.call_tool(
        1,
        "sprite_rig_render",
        ".sprite-studio/rigs/archive-walk.json",
    );
    assert_eq!(first["result"]["isError"], false);
    let first_manifest = &first["result"]["structuredContent"];
    let prior_frames = first_manifest["files"]
        .as_array()
        .expect("first render should list files")
        .iter()
        .map(|value| {
            let relative = value.as_str().expect("rendered file should be text");
            let path = fixture.root.join(relative);
            let name = path
                .file_name()
                .and_then(|value| value.to_str())
                .expect("rendered frame should have a UTF-8 name")
                .to_string();
            let bytes = std::fs::read(path).expect("prior frame should be readable");
            (name, bytes)
        })
        .collect::<HashMap<_, _>>();
    assert_eq!(prior_frames.len(), 8);

    let mut revised: Value = serde_json::from_slice(
        &std::fs::read(&rig_path).expect("normalized rig should be readable"),
    )
    .expect("normalized rig should remain JSON");
    revised["fps"] = json!(10);
    std::fs::write(
        &rig_path,
        serde_json::to_vec_pretty(&revised).expect("revised rig should serialize"),
    )
    .expect("revised rig should write");
    let second = mcp.call_tool(
        2,
        "sprite_rig_render",
        ".sprite-studio/rigs/archive-walk.json",
    );
    assert_eq!(second["result"]["isError"], false);
    let second_manifest = &second["result"]["structuredContent"];
    assert_eq!(second_manifest["fps"], 10);
    assert_ne!(second_manifest["rigHash"], first_manifest["rigHash"]);

    let manifest_path = fixture.root.join(".sprite-studio/last-generation.json");
    let active_manifest: Value = serde_json::from_slice(
        &std::fs::read(&manifest_path).expect("active manifest should be readable"),
    )
    .expect("active manifest should remain JSON");
    assert_eq!(&active_manifest, second_manifest);
    let active_files = active_manifest["files"]
        .as_array()
        .expect("active manifest should list files");
    assert_eq!(active_files.len(), 8);
    assert_eq!(
        active_manifest["quality"]["renderedFrameHashes"]
            .as_array()
            .map(Vec::len),
        Some(8)
    );
    assert!(active_files.iter().all(|value| fixture
        .root
        .join(value.as_str().expect("active file should be text"))
        .is_file()));

    let archive_parent = fixture
        .root
        .join(".sprite-studio/versions/rigs/archive_walk");
    let archives = std::fs::read_dir(&archive_parent)
        .expect("rerender should create an archive directory")
        .map(|entry| entry.expect("archive entry should be readable").path())
        .collect::<Vec<_>>();
    assert_eq!(archives.len(), 1, "one rerender should create one archive");
    assert!(archives[0].is_dir());
    let archived_frames = std::fs::read_dir(&archives[0])
        .expect("archive should list prior frames")
        .map(|entry| {
            let path = entry.expect("archived frame should be readable").path();
            let name = path
                .file_name()
                .and_then(|value| value.to_str())
                .expect("archived frame should have a UTF-8 name")
                .to_string();
            let bytes = std::fs::read(path).expect("archived frame should be readable");
            (name, bytes)
        })
        .collect::<HashMap<_, _>>();
    assert_eq!(archived_frames, prior_frames);

    let transactions = fixture.root.join(".sprite-studio/transactions");
    assert_eq!(
        std::fs::read_dir(transactions)
            .expect("transaction root should be readable")
            .count(),
        0,
        "a successful rerender must clean its transaction journal"
    );
}

#[test]
fn bundled_rig_mcp_rejects_an_output_that_aliases_its_source_master() {
    let fixture = RigRendererFixture::new();
    let source = fixture.root.join("assets/characters/source_alias_01.png");
    std::fs::copy(fixture.root.join("assets/characters/walker.png"), &source)
        .expect("aliased source fixture should copy");
    let source_bytes = std::fs::read(&source).expect("aliased source should be readable");

    let mut rig = ik_biped_walk();
    rig["name"] = json!("source_alias");
    rig["source"] = json!("assets/characters/source_alias_01.png");
    fixture.write_rig("source-alias", &rig);
    let mut mcp = RigMcpSession::new(&fixture.root);
    mcp.initialize_legacy();
    let rejected = mcp.call_tool(
        1,
        "sprite_rig_render",
        ".sprite-studio/rigs/source-alias.json",
    );
    assert_eq!(rejected["result"]["isError"], true);
    assert!(rejected["result"]["structuredContent"]["error"]
        .as_str()
        .expect("source alias rejection should be text")
        .contains("cannot replace the locked source master"));
    assert_eq!(
        std::fs::read(&source).expect("source should survive alias rejection"),
        source_bytes
    );
    assert!(!fixture
        .root
        .join("assets/characters/source_alias_02.png")
        .exists());
    assert!(!fixture
        .root
        .join(".sprite-studio/last-generation.json")
        .exists());
    assert!(!fixture.root.join(".sprite-studio/versions").exists());
}

#[cfg(unix)]
#[test]
fn bundled_rig_mcp_rejects_a_symlinked_external_asset_root() {
    let fixture = RigRendererFixture::new();
    let external = std::env::temp_dir().join(format!(
        "sprite-studio-external-assets-test-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(external.join("characters"))
        .expect("external asset fixture should exist");
    std::fs::copy(
        fixture.root.join("assets/characters/walker.png"),
        external.join("characters/walker.png"),
    )
    .expect("external master should copy");
    std::fs::remove_dir_all(fixture.root.join("assets"))
        .expect("temporary in-workspace assets should be removable");
    std::os::unix::fs::symlink(&external, fixture.root.join("assets"))
        .expect("external asset symlink should be created");
    fixture.write_rig("external-source", &good_biped_walk());

    let mut mcp = RigMcpSession::new(&fixture.root);
    let initialized = mcp.request(&json!({
        "jsonrpc": "2.0",
        "id": 0,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-11-25",
            "capabilities": {},
            "clientInfo": {"name": "sprite-studio-test", "version": "1"},
        },
    }));
    assert_eq!(initialized["result"]["protocolVersion"], "2025-11-25");
    mcp.send(&json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized",
    }));
    let rejected = mcp.request(&json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "sprite_rig_validate",
            "arguments": {"rig": ".sprite-studio/rigs/external-source.json"},
        },
    }));
    assert_eq!(rejected["result"]["isError"], true);
    assert!(rejected["result"]["structuredContent"]["error"]
        .as_str()
        .expect("external source rejection should be text")
        .contains("source must be an existing PNG under assets/ or .sprite-studio/"));

    drop(mcp);
    std::fs::remove_dir_all(external).expect("external asset fixture should be removable");
}
