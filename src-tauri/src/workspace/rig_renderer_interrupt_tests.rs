use super::test_support::*;
use image::{Rgba, RgbaImage};
use serde_json::{json, Value};
use std::{collections::HashMap, process::Command};

#[test]
fn bundled_rig_renderer_rolls_back_post_replace_interrupts() {
    let fixture = RigRendererFixture::new();
    let mut rig = ik_biped_walk();
    rig["name"] = json!("rollback_walk");
    let first = fixture.render("rollback-walk", &rig);
    assert!(
        first.status.success(),
        "baseline rollback fixture should render: {}",
        String::from_utf8_lossy(&first.stderr)
    );
    let first_manifest: Value =
        serde_json::from_slice(&first.stdout).expect("baseline render should return a manifest");
    let active_frames = first_manifest["files"]
        .as_array()
        .expect("baseline manifest should list active frames")
        .iter()
        .map(|value| {
            fixture
                .root
                .join(value.as_str().expect("frame should be text"))
        })
        .collect::<Vec<_>>();
    let active_bytes = active_frames
        .iter()
        .map(|path| std::fs::read(path).expect("baseline frame should be readable"))
        .collect::<Vec<_>>();
    let manifest_path = fixture.root.join(".sprite-studio/last-generation.json");
    let manifest_bytes =
        std::fs::read(&manifest_path).expect("baseline manifest should be readable");
    let rig_path = fixture.root.join(".sprite-studio/rigs/rollback-walk.json");

    let mut revised: Value = serde_json::from_slice(
        &std::fs::read(&rig_path).expect("normalized rollback rig should be readable"),
    )
    .expect("normalized rollback rig should remain JSON");
    revised["fps"] = json!(10);
    for index in 0..8 {
        let dx = revised["frames"][index]["transforms"]["torso"]["dx"]
            .as_i64()
            .expect("torso dx should be an integer");
        revised["frames"][index]["transforms"]["torso"]["dx"] = json!(dx + 1);
    }
    let revised_rig_bytes =
        serde_json::to_vec_pretty(&revised).expect("revised rollback rig should serialize");
    std::fs::write(&rig_path, &revised_rig_bytes).expect("revised rollback rig should write");

    let injection = r#"
import importlib.util
import sys
from pathlib import Path

failure_kind = sys.argv[1]
workspace = Path.cwd().resolve()
engine = workspace / ".sprite-studio" / "sprite_rig.py"
sys.path.insert(0, str(engine.parent))
spec = importlib.util.spec_from_file_location("sprite_rig_injected_rollback", engine)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)

failure_targets = {
    "frame-1": workspace / "assets" / "characters" / "rollback_walk_01.png",
    "spec": workspace / ".sprite-studio" / "rigs" / "rollback-walk.json",
    "manifest": workspace / ".sprite-studio" / "last-generation.json",
    "rollback-interrupted": workspace / "assets" / "characters" / "rollback_walk_01.png",
}
failure_target = failure_targets[failure_kind]
original_replace = module.os.replace
state = {"target_calls": 0}

def injected_replace(source, destination):
    target = Path(destination).resolve()
    if target == failure_target:
        state["target_calls"] += 1
        if state["target_calls"] == 1:
            original_replace(source, destination)
            raise OSError(f"injected post-replace {failure_kind} failure")
        if failure_kind == "rollback-interrupted" and state["target_calls"] == 2:
            raise OSError("injected rollback restore interruption")
    return original_replace(source, destination)

module.os.replace = injected_replace
sys.argv = [str(engine), ".sprite-studio/rigs/rollback-walk.json"]
module.main()
"#;
    for failure_kind in ["frame-1", "spec", "manifest"] {
        let failed = Command::new("python3")
            .current_dir(&fixture.root)
            .arg("-c")
            .arg(injection)
            .arg(failure_kind)
            .output()
            .expect("python3 should execute the injected rollback render");
        assert!(
            !failed.status.success(),
            "post-replace {failure_kind} interrupt must fail"
        );
        let diagnostic = String::from_utf8_lossy(&failed.stderr);
        assert!(
            diagnostic.contains("render commit failed before activation and was rolled back")
                && diagnostic.contains(&format!("injected post-replace {failure_kind} failure")),
            "renderer should report a clean {failure_kind} rollback: {diagnostic}"
        );
        assert!(!diagnostic.contains("Traceback"));

        for ((path, expected), index) in active_frames.iter().zip(&active_bytes).zip(1..) {
            assert_eq!(
                std::fs::read(path).expect("rolled-back frame should remain readable"),
                *expected,
                "active frame {index} must be byte-identical after {failure_kind} rollback"
            );
        }
        assert_eq!(
            std::fs::read(&manifest_path).expect("rolled-back manifest should remain readable"),
            manifest_bytes,
            "{failure_kind} rollback must restore the prior active manifest"
        );
        assert_eq!(
            std::fs::read(&rig_path).expect("rolled-back rig should remain readable"),
            revised_rig_bytes,
            "{failure_kind} rollback must restore the exact pre-render rig revision"
        );
        assert!(!fixture.root.join(".sprite-studio/versions").exists());
        assert_eq!(
            std::fs::read_dir(fixture.root.join(".sprite-studio/transactions"))
                .expect("transaction root should remain readable")
                .count(),
            0,
            "successful {failure_kind} rollback must clean its transaction journal"
        );
        assert_eq!(
            std::fs::read_dir(fixture.root.join("assets/characters"))
                .expect("output directory should remain readable")
                .filter_map(Result::ok)
                .filter(|entry| entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".rollback_walk-stage-"))
                .count(),
            0,
            "successful {failure_kind} rollback must remove staged frame directories"
        );
    }

    let interrupted = Command::new("python3")
        .current_dir(&fixture.root)
        .arg("-c")
        .arg(injection)
        .arg("rollback-interrupted")
        .output()
        .expect("python3 should execute the interrupted rollback render");
    assert!(!interrupted.status.success());
    let diagnostic = String::from_utf8_lossy(&interrupted.stderr);
    assert!(
        diagnostic.contains("render commit failed and rollback needs manual recovery")
            && diagnostic.contains("injected rollback restore interruption"),
        "interrupted rollback should identify retained recovery state: {diagnostic}"
    );
    assert!(!diagnostic.contains("Traceback"));

    let transactions = fixture.root.join(".sprite-studio/transactions");
    let retained = std::fs::read_dir(&transactions)
        .expect("transaction root should remain readable")
        .map(|entry| {
            entry
                .expect("retained transaction should be readable")
                .path()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        retained.len(),
        1,
        "an interrupted rollback must retain exactly its recovery transaction"
    );
    let journal: Value = serde_json::from_slice(
        &std::fs::read(retained[0].join("journal.json"))
            .expect("recovery journal should be readable"),
    )
    .expect("recovery journal should remain JSON");
    assert_eq!(journal["state"], "committing");
    let frame_stage = journal["frameStage"]
        .as_str()
        .expect("recovery journal should identify its frame stage");
    assert!(
        !fixture.root.join(frame_stage).exists(),
        "failed activation should still remove expendable staged frames"
    );

    let retained_backups = std::fs::read_dir(retained[0].join("backups/frames"))
        .expect("recovery transaction should retain frame backups")
        .map(|entry| {
            let path = entry.expect("frame backup should be readable").path();
            let name = path
                .file_name()
                .and_then(|value| value.to_str())
                .expect("frame backup should have a UTF-8 name")
                .to_string();
            let bytes = std::fs::read(path).expect("frame backup should be readable");
            (name, bytes)
        })
        .collect::<HashMap<_, _>>();
    assert_eq!(retained_backups.len(), 8);
    for (path, expected) in active_frames.iter().zip(&active_bytes) {
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .expect("active frame should have a UTF-8 name");
        assert_eq!(
            retained_backups.get(name),
            Some(expected),
            "manual recovery must retain the exact prior bytes for {name}"
        );
    }
    assert_eq!(
        std::fs::read(&manifest_path).expect("prior manifest should remain readable"),
        manifest_bytes,
        "rollback should restore the manifest even when one frame restore is interrupted"
    );
    assert_eq!(
        std::fs::read(&rig_path).expect("prior rig should remain readable"),
        revised_rig_bytes,
        "rollback should restore the rig even when one frame restore is interrupted"
    );
    assert_ne!(
        std::fs::read(&active_frames[0]).expect("partially activated frame should exist"),
        active_bytes[0],
        "the retained transaction must correspond to a real unresolved frame mutation"
    );
}

#[test]
fn bundled_rig_renderer_rejects_a_source_swap_after_decoding() {
    let fixture = RigRendererFixture::new();
    let source_path = fixture.root.join("assets/characters/walker.png");
    let original_source_bytes =
        std::fs::read(&source_path).expect("original race source should be readable");
    let alternate_path = fixture
        .root
        .join(".sprite-studio/alternate-race-source.png");
    let mut alternate = RgbaImage::new(32, 32);
    for y in 3..29 {
        for x in 3..29 {
            alternate.put_pixel(x, y, Rgba([28, 188, 122, 255]));
        }
    }
    alternate
        .save(&alternate_path)
        .expect("alternate race source should save");
    let alternate_bytes =
        std::fs::read(&alternate_path).expect("alternate race source should be readable");
    assert_ne!(alternate_bytes, original_source_bytes);

    let mut rig = ik_biped_walk();
    rig["name"] = json!("source_race_walk");
    let (rig_path, rig_bytes) = fixture.write_rig("source-race-walk", &rig);
    let injection = r#"
import importlib.util
import shutil
import sys
from pathlib import Path

workspace = Path.cwd().resolve()
engine = workspace / ".sprite-studio" / "sprite_rig.py"
source = workspace / "assets" / "characters" / "walker.png"
alternate = workspace / ".sprite-studio" / "alternate-race-source.png"
sys.path.insert(0, str(engine.parent))
spec = importlib.util.spec_from_file_location("sprite_rig_injected_source_race", engine)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)

original_load_png = module.load_png
state = {"swapped": False}

def raced_load_png(path):
    decoded = original_load_png(path)
    if Path(path).resolve() == source and not state["swapped"]:
        state["swapped"] = True
        shutil.copy2(alternate, source)
    return decoded

module.load_png = raced_load_png
sys.argv = [str(engine), ".sprite-studio/rigs/source-race-walk.json"]
module.main()
"#;
    let failed = Command::new("python3")
        .current_dir(&fixture.root)
        .arg("-c")
        .arg(injection)
        .output()
        .expect("python3 should execute the injected source-race render");
    assert!(!failed.status.success(), "source identity race must fail");
    let diagnostic = String::from_utf8_lossy(&failed.stderr);
    assert!(
        diagnostic.contains("source master changed while the render was being prepared; retry"),
        "source race should fail before activation: {diagnostic}"
    );
    assert!(!diagnostic.contains("Traceback"));
    assert_eq!(
        std::fs::read(&source_path).expect("swapped source should remain readable"),
        alternate_bytes,
        "the fixture must prove the on-disk source actually changed after decoding"
    );
    assert_eq!(
        std::fs::read(&rig_path).expect("race rig should remain readable"),
        rig_bytes,
        "source identity failure must not normalize or rewrite the rig"
    );
    for index in 1..=8 {
        assert!(!fixture
            .root
            .join(format!("assets/characters/source_race_walk_{index:02}.png"))
            .exists());
    }
    assert!(!fixture
        .root
        .join(".sprite-studio/last-generation.json")
        .exists());
    assert!(!fixture.root.join(".sprite-studio/versions").exists());
    let transactions = fixture.root.join(".sprite-studio/transactions");
    assert!(transactions.is_dir());
    assert_eq!(
        std::fs::read_dir(transactions)
            .expect("transaction root should be readable")
            .count(),
        0,
        "pre-activation source race must clean its transaction"
    );
    assert_eq!(
        std::fs::read_dir(fixture.root.join("assets/characters"))
            .expect("asset directory should remain readable")
            .filter_map(Result::ok)
            .filter(|entry| entry
                .file_name()
                .to_string_lossy()
                .starts_with(".source_race_walk-stage-"))
            .count(),
        0,
        "pre-activation source race must remove staged frames"
    );
}
