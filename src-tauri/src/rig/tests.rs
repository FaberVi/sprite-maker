use super::{
    capsule_coverage, detect_morphology, parse_rig_suggestion_text, render_frames,
    render_rig_animation_inner, save_rig_inner, suggest_points, validate_rig, Rig, RigBone,
    RigFrame, RigInput, RigPoint, RigTransform,
};
use crate::models::GenerationManifest;
use crate::{assets, AppState};
use image::RgbaImage;
use uuid::Uuid;

fn blank_rig() -> Rig {
    Rig {
        id: "rig-1".into(),
        workspace_id: "ws".into(),
        worktree_id: None,
        asset_id: None,
        name: "test".into(),
        morphology: "biped".into(),
        fps: 8.0,
        looping: true,
        points: Vec::new(),
        bones: Vec::new(),
        frames: Vec::new(),
        created_at: String::new(),
        updated_at: String::new(),
    }
}

fn point(name: &str, x: f64, y: f64) -> RigPoint {
    RigPoint {
        id: format!("p-{name}"),
        name: name.into(),
        kind: "joint".into(),
        x,
        y,
        confidence: 1.0,
        source: "user".into(),
        note: None,
    }
}

fn solid_square(size: u32) -> RgbaImage {
    let mut image = RgbaImage::new(size, size);
    for y in 0..size {
        for x in 0..size {
            image.put_pixel(x, y, image::Rgba([200, 100, 50, 255]));
        }
    }
    image
}

fn rect(image: &mut RgbaImage, x0: usize, y0: usize, x1: usize, y1: usize) {
    for y in y0..y1 {
        for x in x0..x1 {
            image.put_pixel(x as u32, y as u32, image::Rgba([180, 180, 180, 255]));
        }
    }
}

fn biped_master() -> RgbaImage {
    let mut image = RgbaImage::new(16, 24);
    rect(&mut image, 6, 1, 10, 6); // head
    rect(&mut image, 5, 6, 11, 14); // torso
    rect(&mut image, 5, 14, 8, 23); // left leg
    rect(&mut image, 9, 14, 12, 23); // right leg
    image
}

fn quadruped_master() -> RgbaImage {
    let mut image = RgbaImage::new(48, 24);
    rect(&mut image, 4, 6, 44, 14); // body ends before the leg band
    rect(&mut image, 6, 13, 9, 22);
    rect(&mut image, 14, 13, 17, 22);
    rect(&mut image, 30, 13, 33, 22);
    rect(&mut image, 38, 13, 41, 22);
    image
}

fn snake_master() -> RgbaImage {
    let mut image = RgbaImage::new(64, 14);
    for x in 2..61 {
        let wave = (x / 8) % 2;
        rect(&mut image, x, 5 + wave, x + 1, 9 + wave);
    }
    image
}

fn blob_master() -> RgbaImage {
    let mut image = RgbaImage::new(32, 32);
    let center = 15.5_f64;
    for y in 0..32usize {
        for x in 0..32usize {
            let distance = ((x as f64 - center).powi(2) + (y as f64 - center).powi(2)).sqrt();
            if distance <= 10.0 {
                image.put_pixel(x as u32, y as u32, image::Rgba([180, 180, 180, 255]));
            }
        }
    }
    image
}

#[test]
fn detects_biped_from_a_tall_two_legged_silhouette() {
    let detections = detect_morphology(&biped_master()).expect("detection should run");
    assert_eq!(detections[0].morphology, "biped", "ranking: {detections:?}");
    assert!(
        detections[0].confidence >= 0.6,
        "confidence: {detections:?}"
    );
    assert!(detections[0].reasoning.contains("two separated legs"));
}

#[test]
fn detects_quadruped_from_a_long_four_legged_silhouette() {
    let detections = detect_morphology(&quadruped_master()).expect("detection should run");
    assert_eq!(
        detections[0].morphology, "quadruped",
        "ranking: {detections:?}"
    );
    assert!(detections[0].confidence >= 0.6);
}

#[test]
fn detects_serpentine_from_a_long_thin_body() {
    let detections = detect_morphology(&snake_master()).expect("detection should run");
    assert_eq!(
        detections[0].morphology, "serpentine",
        "ranking: {detections:?}"
    );
}

#[test]
fn round_blobs_are_never_profiled_as_bipeds() {
    let detections = detect_morphology(&blob_master()).expect("detection should run");
    assert!(
        matches!(detections[0].morphology.as_str(), "amorphous" | "object"),
        "ranking: {detections:?}"
    );
}

#[test]
fn template_capsules_tile_a_matching_biped_well() {
    let master = biped_master();
    let suggestion = suggest_points(&master, "biped").expect("suggestion should build");
    let coverage = capsule_coverage(&master, &suggestion);
    assert!(
        coverage > 0.55,
        "capsules should claim most of the silhouette, got {coverage}"
    );
}

#[test]
fn suggest_points_snaps_biped_template_to_silhouette() {
    // A 16x24 figure: head, torso, and two legs.
    let mut master = RgbaImage::new(16, 24);
    for y in 1..6usize {
        for x in 6..10usize {
            master.put_pixel(x as u32, y as u32, image::Rgba([255, 0, 0, 255]));
        }
    }
    for y in 6..14usize {
        for x in 5..11usize {
            master.put_pixel(x as u32, y as u32, image::Rgba([0, 255, 0, 255]));
        }
    }
    for y in 14..23usize {
        for x in 5..8usize {
            master.put_pixel(x as u32, y as u32, image::Rgba([0, 0, 255, 255]));
        }
        for x in 9..12usize {
            master.put_pixel(x as u32, y as u32, image::Rgba([0, 0, 255, 255]));
        }
    }
    let suggestion = suggest_points(&master, "biped").expect("suggestion should succeed");
    assert_eq!(suggestion.source, "auto");
    assert!(
        suggestion.points.len() >= 10,
        "biped template should propose a full joint set"
    );
    assert!(!suggestion.bones.is_empty());
    let head_top = suggestion
        .points
        .iter()
        .find(|p| p.name == "head_top")
        .unwrap();
    let hip = suggestion.points.iter().find(|p| p.name == "hip").unwrap();
    assert!(
        head_top.y < 6.0,
        "head_top should sit inside the head region"
    );
    assert!(
        hip.y > 6.0 && hip.y < 14.0,
        "hip should sit inside the torso region"
    );
    for point in &suggestion.points {
        assert!((0.0..16.0).contains(&point.x) && (0.0..24.0).contains(&point.y));
    }
}

#[test]
fn parse_rig_suggestion_extracts_and_normalizes() {
    let text = "Here is my analysis.\n\n```rig-suggestion\n{\"morphology\":\"quadruped\",\"points\":[{\"name\":\"head\",\"x\":4,\"y\":10},{\"name\":\"tail\",\"x\":900,\"y\":12},{\"name\":\"hip\",\"x\":20,\"y\":14}],\"bones\":[{\"name\":\"body\",\"start\":\"head\",\"end\":\"hip\",\"radius\":4,\"z\":2},{\"name\":\"ghost\",\"start\":\"missing\",\"end\":\"hip\",\"radius\":2}],\"reasoning\":\"quadruped anatomy\"}\n```\nDone.";
    let suggestion = parse_rig_suggestion_text(text, 32, 32).expect("block should parse");
    assert_eq!(suggestion.morphology, "quadruped");
    assert_eq!(suggestion.points.len(), 3);
    let tail = suggestion.points.iter().find(|p| p.name == "tail").unwrap();
    assert_eq!(tail.x, 31.0, "out-of-canvas x clamps to the last pixel");
    assert_eq!(
        suggestion.bones.len(),
        1,
        "bones with unknown points are dropped"
    );
    assert_eq!(suggestion.bones[0].name, "body");
    assert!(suggestion.reasoning.contains("quadruped"));
}

#[test]
fn parse_rig_suggestion_accepts_plain_json_block() {
    let text = "```json\n{\"morphology\":\"biped\",\"points\":[{\"name\":\"a\",\"x\":1,\"y\":1},{\"name\":\"b\",\"x\":9,\"y\":9}],\"bones\":[{\"name\":\"bone\",\"start\":\"a\",\"end\":\"b\",\"radius\":3}],\"frames\":[{\"phase\":\"contact\",\"transforms\":[{\"bone\":\"bone\",\"rotate\":15}]}]}\n```";
    let suggestion = parse_rig_suggestion_text(text, 16, 16).expect("json block should parse");
    assert_eq!(suggestion.frames.len(), 1);
    assert_eq!(suggestion.frames[0].transforms[0].rotate, 15.0);
}

#[test]
fn validate_rig_reports_cycle_and_missing_references() {
    let mut rig = blank_rig();
    rig.points = vec![point("a", 1.0, 1.0)];
    rig.bones = vec![
        RigBone {
            id: "b1".into(),
            name: "one".into(),
            start_point: "a".into(),
            end_point: "missing".into(),
            radius: 2.0,
            parent: Some("two".into()),
            z: 1,
        },
        RigBone {
            id: "b2".into(),
            name: "two".into(),
            start_point: "a".into(),
            end_point: "a".into(),
            radius: 2.0,
            parent: Some("one".into()),
            z: 1,
        },
    ];
    let warnings = validate_rig(&rig, 16, 16).expect("validation should succeed");
    assert!(
        warnings.iter().any(|w| w.contains("cycle")),
        "warnings: {warnings:?}"
    );
    assert!(warnings.iter().any(|w| w.contains("does not exist")));
}

#[test]
fn rig_persists_renders_into_assets_and_creates_an_animation() {
    let root = std::env::temp_dir().join(format!("sprite-studio-rig-pipeline-{}", Uuid::new_v4()));
    // Match the scaffold workspace_path() guarantees for initialized projects.
    for folder in ["assets/props", "animations", ".sprite-studio"] {
        std::fs::create_dir_all(root.join(folder)).expect("fixture folders");
    }
    let connection = crate::database::open(&root.join("app.sqlite3")).expect("database");
    let state = AppState::from_connection(connection);
    state
        .db
        .lock()
        .expect("database lock")
        .execute(
            "INSERT INTO projects(id,name,path,created_at,last_opened_at) VALUES ('w1','Test',?1,'now','now')",
            [root.to_string_lossy().into_owned()],
        )
        .expect("project row");
    let master_path = root.join("assets/props/master.png");
    solid_square(16).save(&master_path).expect("master png");
    let asset = assets::inspect("w1", &root, &master_path, None).expect("asset inspect");
    assets::upsert(&state, &asset, "import").expect("asset upsert");
    let input = RigInput {
        id: None,
        workspace_id: "w1".into(),
        worktree_id: None,
        asset_id: Some(asset.id.clone()),
        name: "rig walk".into(),
        morphology: "biped".into(),
        fps: 8.0,
        looping: true,
        points: vec![point("a", 8.0, 2.0), point("b", 8.0, 14.0)],
        bones: vec![RigBone {
            id: "b1".into(),
            name: "limb".into(),
            start_point: "a".into(),
            end_point: "b".into(),
            radius: 4.0,
            parent: None,
            z: 1,
        }],
        frames: vec![
            RigFrame {
                phase: Some("swing".into()),
                hold: false,
                root_dx: 0.0,
                root_dy: 0.0,
                transforms: vec![RigTransform {
                    bone: "limb".into(),
                    dx: 0.0,
                    dy: 0.0,
                    rotate: 10.0,
                    scale_x: 1.0,
                    scale_y: 1.0,
                }],
                contacts: vec![],
            },
            RigFrame {
                phase: Some("swing back".into()),
                hold: false,
                root_dx: 0.0,
                root_dy: 0.0,
                transforms: vec![RigTransform {
                    bone: "limb".into(),
                    dx: 0.0,
                    dy: 0.0,
                    rotate: -10.0,
                    scale_x: 1.0,
                    scale_y: 1.0,
                }],
                contacts: vec![],
            },
        ],
    };
    let saved = save_rig_inner(input, &state).expect("rig save");
    let rig_rows: i64 = state
        .db
        .lock()
        .expect("database lock")
        .query_row("SELECT COUNT(*) FROM rigs", [], |row| row.get(0))
        .expect("rig count");
    assert_eq!(rig_rows, 1, "the rig should persist with its spec");

    let master = image::open(&master_path).expect("master reload").to_rgba8();
    let frames = render_frames(&master, &saved);
    assert_eq!(frames.len(), 2);
    let result =
        render_rig_animation_inner(&frames, &saved, &asset, &root, &state).expect("render");
    assert_eq!(result.animation.frames.len(), 2);
    assert_eq!(result.animation.fps, 8.0);
    assert!(root.join("assets/props/rig-walk_01.png").exists());
    assert!(root.join("assets/props/rig-walk_02.png").exists());
    let manifest_text = std::fs::read_to_string(root.join(".sprite-studio/last-generation.json"))
        .expect("manifest file");
    let manifest: GenerationManifest = serde_json::from_str(&manifest_text).expect("manifest json");
    assert_eq!(manifest.files.len(), 2);
    assert_eq!(manifest.category, "props");
    let asset_rows: i64 = state
        .db
        .lock()
        .expect("database lock")
        .query_row("SELECT COUNT(*) FROM assets", [], |row| row.get(0))
        .expect("asset count");
    assert_eq!(asset_rows, 3, "master plus two rendered frames");
    let animation_rows: i64 = state
        .db
        .lock()
        .expect("database lock")
        .query_row("SELECT COUNT(*) FROM animations", [], |row| row.get(0))
        .expect("animation count");
    assert_eq!(animation_rows, 1, "rendered rigs become animations");
    drop(state);
    std::fs::remove_dir_all(root).expect("temporary fixture should be removable");
}
