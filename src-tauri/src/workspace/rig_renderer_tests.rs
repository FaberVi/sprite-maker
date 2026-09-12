use super::test_support::*;
use serde_json::{json, Value};

#[test]
fn bundled_rig_renderer_accepts_a_connected_eight_frame_biped_walk() {
    let fixture = RigRendererFixture::new();
    let output = fixture.render("good-walk", &good_biped_walk());
    assert!(
        output.status.success(),
        "good biped walk should render: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let manifest: Value = serde_json::from_slice(&output.stdout)
        .expect("successful rig rendering should print a JSON manifest");
    let files = manifest["files"]
        .as_array()
        .expect("render manifest should list output files");
    assert_eq!(files.len(), 8, "walk should render all eight planned poses");
    assert!(files.iter().all(|file| fixture
        .root
        .join(file.as_str().expect("manifest file should be a string"))
        .is_file()));
}

#[test]
fn bundled_rig_renderer_deforms_a_weighted_pixel_mesh_deterministically() {
    let fixture = RigRendererFixture::new();
    let mut rig = good_biped_walk();
    rig["name"] = json!("weighted_mesh_walk");
    rig["parts"][1]["mesh"] = json!({
        "vertices": [[9, 12], [13, 12], [9, 19], [13, 19]],
        "triangles": [[0, 1, 2], [1, 3, 2]],
        "weights": [
            [{"bone": "left_upper", "weight": 1.0}],
            [{"bone": "left_upper", "weight": 1.0}],
            [{"bone": "left_lower", "weight": 1.0}],
            [{"bone": "left_lower", "weight": 1.0}],
        ],
    });
    let first = fixture.render("weighted-mesh-walk", &rig);
    assert!(
        first.status.success(),
        "weighted mesh walk should render: {}",
        String::from_utf8_lossy(&first.stderr)
    );
    let first_manifest: Value = serde_json::from_slice(&first.stdout)
        .expect("weighted mesh render should print a manifest");
    let first_hashes = first_manifest["quality"]["renderedFrameHashes"].clone();
    let second = fixture.render("weighted-mesh-walk", &rig);
    assert!(
        second.status.success(),
        "weighted mesh rerender should succeed: {}",
        String::from_utf8_lossy(&second.stderr)
    );
    let second_manifest: Value = serde_json::from_slice(&second.stdout)
        .expect("weighted mesh rerender should print a manifest");
    assert_eq!(
        first_hashes, second_manifest["quality"]["renderedFrameHashes"],
        "weighted mesh rasterization must be reproducible byte-for-byte"
    );
}

#[test]
fn bundled_rig_renderer_enforces_profiled_joints_poses_and_residual_ownership() {
    let fixture = RigRendererFixture::new();
    let rig = good_v3_human_walk();
    let rendered = fixture.render("profiled-human-walk", &rig);
    assert!(
        rendered.status.success(),
        "version 3 human rig should render: {}",
        String::from_utf8_lossy(&rendered.stderr)
    );
    let manifest: Value =
        serde_json::from_slice(&rendered.stdout).expect("profiled render should print a manifest");
    assert_eq!(manifest["rigVersion"], 3);

    let mut mcp = RigMcpSession::new(&fixture.root);
    mcp.initialize_legacy();
    let analyzed = mcp.request(&json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "sprite_rig_analyze_motion",
            "arguments": {"rig": ".sprite-studio/rigs/profiled-human-walk.json"},
        },
    }));
    assert_eq!(analyzed["result"]["isError"], false);
    let anatomy = &analyzed["result"]["structuredContent"];
    assert_eq!(anatomy["rigProfile"], "human_sprite_rig");
    assert_eq!(anatomy["visibleJointCount"], 6);
    assert_eq!(anatomy["joints"].as_array().map(Vec::len), Some(6));
    assert_eq!(anatomy["poses"].as_array().map(Vec::len), Some(8));

    let mut wrong_animal_harness = rig.clone();
    wrong_animal_harness["name"] = json!("profiled_quadruped_without_joints");
    wrong_animal_harness["rigProfile"] = json!("four_leg_sprite_rig");
    wrong_animal_harness["proposal"]["morphologyTag"] = json!("quadruped");
    let animal_rejected =
        fixture.validate("profiled-quadruped-without-joints", &wrong_animal_harness);
    assert_validation_failed(
        &animal_rejected,
        "four-leg profile without observed animal joints",
        "missing observed joints",
    );

    let mut residual = rig;
    residual["name"] = json!("profiled_human_residual");
    residual["parts"][1]["mask"] = json!({"rect": [10, 14, 3, 5]});
    let rejected = fixture.validate("profiled-human-residual", &residual);
    assert_validation_failed(
        &rejected,
        "profiled human residual",
        "unclaimed source pixels",
    );
}

#[test]
fn bundled_rig_renderer_solves_two_bone_ik_with_locked_contacts() {
    let fixture = RigRendererFixture::new();
    let output = fixture.render("ik-walk", &ik_biped_walk());
    assert!(
        output.status.success(),
        "IK biped walk should render: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let manifest: Value = serde_json::from_slice(&output.stdout)
        .expect("successful IK rig rendering should print a JSON manifest");
    assert_eq!(manifest["rigVersion"], 2);
    assert_eq!(manifest["quality"]["mechanics"], "passed");
    assert_eq!(manifest["quality"]["uniqueRenderedFrames"], 8);
}

#[test]
fn bundled_rig_renderer_rejects_an_unreachable_ik_target() {
    let fixture = RigRendererFixture::new();
    let mut rig = ik_biped_walk();
    rig["frames"][0]["ik"][0]["target"] = json!([0, 0]);
    let output = fixture.validate("unreachable-ik", &rig);
    assert_validation_failed(&output, "unreachable IK target", "target is unreachable");
}

#[test]
fn bundled_rig_renderer_accepts_a_two_pixel_in_place_weight_shift() {
    let fixture = RigRendererFixture::new();
    let mut rig = ik_biped_walk();
    let root_x = [-1, -1, 0, 1, 1, 1, 0, -1];
    for (index, value) in root_x.into_iter().enumerate() {
        rig["frames"][index]["root"]["dx"] = json!(value);
    }
    let output = fixture.validate("weight-shift", &rig);
    assert!(
        output.status.success(),
        "a one-pixel lateral root amplitude should validate: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn bundled_rig_renderer_accepts_monotonic_non_looping_baked_travel() {
    let fixture = RigRendererFixture::new();
    let mut rig = ik_biped_walk();
    rig["looping"] = json!(false);
    rig["rootMotion"] = json!("baked");
    let root_x = [0, 0, 1, 1, 2, 2, 3, 3];
    for (index, value) in root_x.into_iter().enumerate() {
        rig["frames"][index]["root"]["dx"] = json!(value);
    }
    let output = fixture.validate("baked-travel", &rig);
    assert!(
        output.status.success(),
        "non-looping baked travel should not be forced through loop-seam gates: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn bundled_rig_renderer_accepts_one_adjacent_explicit_hold() {
    let fixture = RigRendererFixture::new();
    let mut rig = good_biped_walk();
    rig["frames"][1]["transforms"] = rig["frames"][0]["transforms"].clone();
    rig["frames"][1]["root"] = rig["frames"][0]["root"].clone();
    rig["frames"][1]["contacts"] = rig["frames"][0]["contacts"].clone();
    rig["frames"][1]["hold"] = json!(true);
    let output = fixture.validate("explicit-hold", &rig);
    assert!(
        output.status.success(),
        "one adjacent explicit hold should validate: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn bundled_rig_renderer_rejects_a_non_adjacent_repeated_pose() {
    let fixture = RigRendererFixture::new();
    let mut rig = good_biped_walk();
    rig["frames"][4]["transforms"] = rig["frames"][0]["transforms"].clone();
    rig["frames"][4]["root"] = rig["frames"][0]["root"].clone();
    let output = fixture.validate("repeated-pose", &rig);
    assert_validation_failed(
        &output,
        "non-adjacent repeated pose",
        "repeats non-adjacent",
    );
}

#[test]
fn bundled_rig_renderer_rejects_a_hip_declared_as_ground_contact() {
    let fixture = RigRendererFixture::new();
    let mut rig = good_biped_walk();
    for index in 0..8 {
        let side = if index <= 3 || index == 7 {
            "left"
        } else {
            "right"
        };
        rig["frames"][index]["contacts"] = json!([{
            "part": format!("{side}_upper"),
            "anchor": "hip",
            "state": "planted",
        }]);
    }
    let output = fixture.validate("hip-contact", &rig);
    assert_validation_failed(&output, "hip ground contact", "not on a lower-leg or foot");
}

#[test]
fn bundled_rig_renderer_rejects_an_oversized_joint_cap() {
    let fixture = RigRendererFixture::new();
    let mut rig = good_biped_walk();
    rig["parts"][4]["mask"] = rig["parts"][3]["mask"].clone();
    rig["parts"][4]["overlapMode"] = json!("joint-cap");
    let output = fixture.validate("oversized-cap", &rig);
    assert_validation_failed(&output, "oversized joint cap", "bounded cap limit");
}

#[test]
fn bundled_rig_renderer_rejects_a_duplicate_loop_endpoint() {
    let fixture = RigRendererFixture::new();
    let mut rig = good_biped_walk();
    let first_transforms = rig["frames"][0]["transforms"].clone();
    let first_root = rig["frames"][0]["root"].clone();
    let first_contacts = rig["frames"][0]["contacts"].clone();
    rig["frames"][7]["transforms"] = first_transforms;
    rig["frames"][7]["root"] = first_root;
    rig["frames"][7]["contacts"] = first_contacts;
    let output = fixture.validate("duplicate-endpoint", &rig);
    assert_validation_failed(
        &output,
        "duplicate loop endpoint",
        "duplicate loop endpoint",
    );
}

#[test]
fn bundled_rig_renderer_rejects_in_place_root_drift() {
    let fixture = RigRendererFixture::new();
    let mut rig = good_biped_walk();
    for index in 0..8 {
        rig["frames"][index]["root"]["dx"] = json!(index);
    }
    let output = fixture.validate("root-drift", &rig);
    assert_validation_failed(&output, "in-place root drift", "in-place root drift");
}

#[test]
fn bundled_rig_renderer_rejects_a_grounded_walk_frame_without_contact() {
    let fixture = RigRendererFixture::new();
    let mut rig = good_biped_walk();
    rig["frames"][2]["contacts"] = json!([]);
    let output = fixture.validate("missing-contact", &rig);
    assert_validation_failed(
        &output,
        "missing planted contact",
        "no planted support contact",
    );
}

#[test]
fn bundled_rig_renderer_rejects_a_separated_child_joint() {
    let fixture = RigRendererFixture::new();
    let mut rig = good_biped_walk();
    rig["frames"][4]["transforms"]["left_lower"]["dx"] = json!(4);
    let output = fixture.validate("joint-separation", &rig);
    assert_validation_failed(&output, "separated child joint", "separates by");
}
