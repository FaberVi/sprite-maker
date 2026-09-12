use super::test_support::*;
use serde_json::{json, Value};
use std::collections::HashSet;

#[test]
fn bundled_rig_mcp_negotiates_legacy_and_current_protocols() {
    let fixture = RigRendererFixture::new();
    let mut mcp = RigMcpSession::new(&fixture.root);

    let before_initialize = mcp.request(&json!({
        "jsonrpc": "2.0",
        "id": "before-initialize",
        "method": "tools/list",
        "params": {},
    }));
    assert_eq!(before_initialize["error"]["code"], -32003);
    assert_eq!(
        before_initialize["error"]["message"],
        "MCP server is not initialized"
    );

    let malformed_initialize = mcp.request(&json!({
        "jsonrpc": "2.0",
        "id": "malformed-initialize",
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-11-25",
            "capabilities": {},
        },
    }));
    assert_eq!(malformed_initialize["error"]["code"], -32602);
    assert!(malformed_initialize["error"]["message"]
        .as_str()
        .expect("malformed initialize error should be text")
        .contains("clientInfo"));

    let initialized = mcp.request(&json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-11-25",
            "capabilities": {},
            "clientInfo": {"name": "sprite-studio-test", "version": "1"},
        },
    }));
    assert_eq!(initialized["id"], 1);
    assert_eq!(initialized["result"]["protocolVersion"], "2025-11-25");
    assert_eq!(
        initialized["result"]["capabilities"]["tools"]["listChanged"],
        false
    );
    assert!(initialized["result"]["instructions"]
        .as_str()
        .expect("server instructions should be text")
        .contains("sprite_rig_validate"));

    let before_notification = mcp.request(&json!({
        "jsonrpc": "2.0",
        "id": "before-initialized-notification",
        "method": "tools/list",
        "params": {},
    }));
    assert_eq!(before_notification["error"]["code"], -32003);

    let repeated_initialize = mcp.request(&json!({
        "jsonrpc": "2.0",
        "id": "repeated-initialize",
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-11-25",
            "capabilities": {},
            "clientInfo": {"name": "sprite-studio-test", "version": "1"},
        },
    }));
    assert_eq!(repeated_initialize["error"]["code"], -32600);
    assert_eq!(
        repeated_initialize["error"]["message"],
        "server is already initialized"
    );

    mcp.send(&json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized",
    }));
    let listed = mcp.request(&json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list",
        "params": {},
    }));
    assert_eq!(listed["id"], 2, "notifications must not receive responses");
    let tools = listed["result"]["tools"]
        .as_array()
        .expect("tools/list should return an array");
    let names = tools
        .iter()
        .map(|tool| tool["name"].as_str().expect("tool name should be text"))
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        [
            "sprite_rig_validate",
            "sprite_rig_render",
            "sprite_rig_analyze_motion",
            "sprite_rig_analyze_walk",
        ]
    );
    assert_eq!(tools[0]["annotations"]["readOnlyHint"], true);
    assert_eq!(tools[1]["annotations"]["readOnlyHint"], false);
    assert_eq!(
        tools[1]["annotations"]["idempotentHint"], false,
        "render reruns archive prior frames and are not idempotent"
    );
    assert_eq!(tools[2]["annotations"]["readOnlyHint"], true);
    assert_eq!(tools[3]["annotations"]["readOnlyHint"], true);
    assert!(tools.iter().all(|tool| {
        tool["annotations"]["destructiveHint"] == false
            && tool["annotations"]["openWorldHint"] == false
    }));

    let discovered = mcp.request(&json!({
        "jsonrpc": "2.0",
        "id": "discover",
        "method": "server/discover",
        "params": {
            "_meta": {
                "io.modelcontextprotocol/protocolVersion": "2026-07-28",
                "io.modelcontextprotocol/clientInfo": {
                    "name": "sprite-studio-test",
                    "version": "1",
                },
                "io.modelcontextprotocol/clientCapabilities": {},
            },
        },
    }));
    assert_eq!(discovered["id"], "discover");
    assert_eq!(discovered["result"]["resultType"], "complete");
    assert!(discovered["result"]["supportedVersions"]
        .as_array()
        .expect("discovery should list supported protocol versions")
        .contains(&json!("2026-07-28")));
    assert_eq!(discovered["result"]["cacheScope"], "public");
    assert!(
        discovered["result"]["ttlMs"]
            .as_u64()
            .expect("discovery should provide a nonnegative TTL")
            > 0
    );
    let modern_list = mcp.request(&json!({
        "jsonrpc": "2.0",
        "id": "modern-list",
        "method": "tools/list",
        "params": {
            "_meta": {
                "io.modelcontextprotocol/protocolVersion": "2026-07-28",
                "io.modelcontextprotocol/clientCapabilities": {},
            },
        },
    }));
    assert_eq!(modern_list["result"]["resultType"], "complete");
    assert_eq!(modern_list["result"]["cacheScope"], "public");
    assert_eq!(
        modern_list["result"]["_meta"]["io.modelcontextprotocol/serverInfo"]["name"],
        "sprite-studio-rig"
    );
    let unsupported = mcp.request(&json!({
        "jsonrpc": "2.0",
        "id": "unsupported-version",
        "method": "ping",
        "params": {
            "_meta": {
                "io.modelcontextprotocol/protocolVersion": "2099-01-01",
                "io.modelcontextprotocol/clientCapabilities": {},
            },
        },
    }));
    assert_eq!(unsupported["error"]["code"], -32022);
    assert_eq!(unsupported["error"]["data"]["requested"], "2099-01-01");
    assert!(unsupported["error"]["data"]["supported"]
        .as_array()
        .expect("unsupported-version error should list supported versions")
        .contains(&json!("2026-07-28")));

    mcp.send_raw(b"{");
    let parse_error = mcp.read_response();
    assert_eq!(parse_error["error"]["code"], -32700);
    let ping = mcp.request(&json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "ping",
        "params": {},
    }));
    assert_eq!(ping["id"], 3, "server should recover after malformed input");
    assert!(ping["result"].is_object());
    mcp.send_raw(br#"{"jsonrpc":"2.0","id":"nan","method":"ping","params":{"value":NaN}}"#);
    let nonfinite = mcp.read_response();
    assert_eq!(nonfinite["error"]["code"], -32600);
    let ping_after_nonfinite = mcp.request(&json!({
        "jsonrpc": "2.0",
        "id": 4,
        "method": "ping",
        "params": {},
    }));
    assert_eq!(
        ping_after_nonfinite["id"], 4,
        "server should recover after a non-finite JSON constant"
    );
}

#[test]
fn bundled_rig_mcp_exercises_real_tools_and_confines_rig_paths() {
    let fixture = RigRendererFixture::new();
    let (rig_path, original_bytes) = fixture.write_rig("mcp-ik-walk", &ik_biped_walk());
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
    let call = |id: u64, name: &str, rig: &str| {
        json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tools/call",
            "params": {
                "name": name,
                "arguments": {"rig": rig},
            },
        })
    };
    let relative_rig = ".sprite-studio/rigs/mcp-ik-walk.json";

    let validated = mcp.request(&call(1, "sprite_rig_validate", relative_rig));
    assert_eq!(validated["result"]["isError"], false);
    assert_eq!(validated["result"]["structuredContent"]["valid"], true);
    assert_eq!(validated["result"]["structuredContent"]["rigVersion"], 2);
    assert_eq!(
        std::fs::read(&rig_path).expect("validated rig should remain readable"),
        original_bytes,
        "MCP validation must use the non-mutating --check path"
    );
    assert!(!fixture
        .root
        .join(".sprite-studio/last-generation.json")
        .exists());
    assert!(!fixture.root.join(".sprite-studio/versions").exists());
    assert!(!fixture
        .root
        .join("assets/characters/regression_biped_walk_01.png")
        .exists());

    let forged_manifest = b"{\"analysis\":{\"phases\":[\"forged\"]}}\n";
    let manifest_path = fixture.root.join(".sprite-studio/last-generation.json");
    std::fs::write(&manifest_path, forged_manifest).expect("forged stale manifest should write");
    let analyzed = mcp.request(&call(2, "sprite_rig_analyze_motion", relative_rig));
    assert_eq!(analyzed["result"]["isError"], false);
    let analysis = &analyzed["result"]["structuredContent"];
    assert_eq!(analysis["frameCount"], 8);
    assert_eq!(analysis["rootMotion"], "in-place");
    assert_eq!(analysis["phases"].as_array().map(Vec::len), Some(8));
    assert_eq!(
        analysis["transitionEnergyPx"].as_array().map(Vec::len),
        Some(8),
        "a looping eight-frame gait should include the solved loop seam"
    );
    assert!(analysis["plantedContactGroups"]
        .as_object()
        .expect("analysis should group solved contacts")
        .contains_key("left_lower.foot"));
    let left_contacts = analysis["plantedContactGroups"]["left_lower.foot"]
        .as_array()
        .expect("left planted contacts should be an array");
    assert!(left_contacts
        .iter()
        .all(|contact| contact["x"] == 11.0 && contact["y"] == 26.0));
    let is_sha256 = |value: &Value| {
        value.as_str().is_some_and(|hash| {
            hash.len() == 64
                && hash
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        })
    };
    assert!(is_sha256(&analysis["masterHash"]));
    assert!(is_sha256(&analysis["rigHash"]));
    let analyzed_frame_hashes = analysis["renderedFrameHashes"]
        .as_array()
        .expect("analysis should return ordered rendered-frame hashes");
    assert_eq!(analyzed_frame_hashes.len(), 8);
    assert!(analyzed_frame_hashes.iter().all(is_sha256));
    assert_eq!(
        analyzed_frame_hashes
            .iter()
            .map(|hash| hash.as_str().expect("frame hash should be text"))
            .collect::<HashSet<_>>()
            .len(),
        8,
        "the accepted walk should have eight unique rendered frames"
    );
    let analyzed_master_hash = analysis["masterHash"].clone();
    let analyzed_rig_hash = analysis["rigHash"].clone();
    let analyzed_frame_hashes = analysis["renderedFrameHashes"].clone();
    assert_eq!(
        std::fs::read(&rig_path).expect("analyzed rig should remain readable"),
        original_bytes,
        "walk analysis must not normalize the user's rig"
    );
    assert_eq!(
        std::fs::read(&manifest_path).expect("stale manifest should remain readable"),
        forged_manifest,
        "walk analysis must come from the current rig, not mutate or trust a manifest"
    );
    assert!(!fixture.root.join(".sprite-studio/versions").exists());
    assert!(!fixture
        .root
        .join("assets/characters/regression_biped_walk_01.png")
        .exists());

    #[cfg(unix)]
    {
        let output_frame = fixture
            .root
            .join("assets/characters/regression_biped_walk_01.png");
        std::os::unix::fs::symlink(
            fixture.root.join("missing-external-frame.png"),
            &output_frame,
        )
        .expect("broken output-frame symlink should be created");
        let rejected_output = mcp.request(&call(3, "sprite_rig_render", relative_rig));
        assert_eq!(rejected_output["result"]["isError"], true);
        assert!(rejected_output["result"]["structuredContent"]["error"]
            .as_str()
            .expect("output symlink rejection should be text")
            .contains("output frame 1 cannot be a symbolic link"));
        std::fs::remove_file(output_frame)
            .expect("broken output-frame symlink should be removable");
    }

    let rendered = mcp.request(&call(4, "sprite_rig_render", relative_rig));
    assert_eq!(rendered["result"]["isError"], false);
    let manifest = &rendered["result"]["structuredContent"];
    let files = manifest["files"]
        .as_array()
        .expect("render should return output files");
    assert_eq!(files.len(), 8);
    assert_eq!(manifest["masterHash"], analyzed_master_hash);
    assert_eq!(manifest["rigHash"], analyzed_rig_hash);
    assert_eq!(
        manifest["quality"]["renderedFrameHashes"], analyzed_frame_hashes,
        "analysis and render must identify the same ordered frame sequence"
    );
    assert!(files.iter().all(|file| fixture
        .root
        .join(file.as_str().expect("rendered file should be text"))
        .is_file()));

    let mut invalid = good_biped_walk();
    invalid["frames"][2]["contacts"] = json!([]);
    fixture.write_rig("mcp-invalid", &invalid);
    let rejected = mcp.request(&call(
        5,
        "sprite_rig_validate",
        ".sprite-studio/rigs/mcp-invalid.json",
    ));
    assert_eq!(rejected["result"]["isError"], true);
    assert_eq!(rejected["result"]["structuredContent"]["ok"], false);
    assert!(rejected["result"]["structuredContent"]["error"]
        .as_str()
        .expect("validation error should be text")
        .contains("no planted support contact"));

    let escaped_path = fixture.root.join("outside-rig.json");
    std::fs::write(&escaped_path, b"{}\n").expect("outside fixture should write");
    let escaped = mcp.request(&call(6, "sprite_rig_validate", "outside-rig.json"));
    assert_eq!(escaped["result"]["isError"], true);
    assert!(escaped["result"]["structuredContent"]["error"]
        .as_str()
        .expect("path error should be text")
        .contains("must stay under .sprite-studio/rigs"));

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(
            &escaped_path,
            fixture.root.join(".sprite-studio/rigs/symlinked-rig.json"),
        )
        .expect("rig symlink should be created");
        let symlinked = mcp.request(&call(
            7,
            "sprite_rig_validate",
            ".sprite-studio/rigs/symlinked-rig.json",
        ));
        assert_eq!(symlinked["result"]["isError"], true);
        assert!(symlinked["result"]["structuredContent"]["error"]
            .as_str()
            .expect("symlink escape should be text")
            .contains("must stay under .sprite-studio/rigs"));
    }
}
