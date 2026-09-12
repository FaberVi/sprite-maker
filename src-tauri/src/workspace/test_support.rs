use super::*;
use crate::database;
use image::{Rgba, RgbaImage};
use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader, Write},
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
};

pub(super) struct RigRendererFixture {
    pub(super) root: PathBuf,
}

pub(super) struct RigMcpSession {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
}

impl RigMcpSession {
    pub(super) fn new(root: &Path) -> Self {
        let mut child = Command::new("python3")
            .current_dir(root)
            .arg(".sprite-studio/sprite_rig_mcp.py")
            .arg("--workspace")
            .arg(root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("python3 should start the installed rig MCP server");
        let stdin = child.stdin.take().expect("MCP stdin should be piped");
        let stdout = child.stdout.take().expect("MCP stdout should be piped");
        Self {
            child,
            stdin: Some(stdin),
            stdout: BufReader::new(stdout),
        }
    }

    pub(super) fn send(&mut self, message: &Value) {
        let stdin = self.stdin.as_mut().expect("MCP stdin should stay open");
        serde_json::to_writer(&mut *stdin, message).expect("MCP message should serialize");
        stdin
            .write_all(b"\n")
            .expect("MCP message should terminate with a newline");
        stdin.flush().expect("MCP message should flush");
    }

    pub(super) fn send_raw(&mut self, message: &[u8]) {
        let stdin = self.stdin.as_mut().expect("MCP stdin should stay open");
        stdin
            .write_all(message)
            .expect("raw MCP message should write");
        stdin
            .write_all(b"\n")
            .expect("raw MCP message should terminate with a newline");
        stdin.flush().expect("raw MCP message should flush");
    }

    pub(super) fn read_response(&mut self) -> Value {
        let mut line = String::new();
        let bytes = self
            .stdout
            .read_line(&mut line)
            .expect("MCP response should be readable");
        assert!(bytes > 0, "MCP server exited before responding");
        serde_json::from_str(&line).unwrap_or_else(|error| {
            panic!("MCP stdout must contain only JSON-RPC: {error}: {line:?}")
        })
    }

    pub(super) fn request(&mut self, message: &Value) -> Value {
        self.send(message);
        self.read_response()
    }

    pub(super) fn initialize_legacy(&mut self) {
        let initialized = self.request(&json!({
            "jsonrpc": "2.0",
            "id": "test-initialize",
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": {"name": "sprite-studio-test", "version": "1"},
            },
        }));
        assert_eq!(initialized["result"]["protocolVersion"], "2025-11-25");
        self.send(&json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized",
        }));
    }

    pub(super) fn call_tool(&mut self, id: u64, name: &str, rig: &str) -> Value {
        self.request(&json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tools/call",
            "params": {
                "name": name,
                "arguments": {"rig": rig},
            },
        }))
    }
}

impl Drop for RigMcpSession {
    fn drop(&mut self) {
        self.stdin.take();
        let _ = self.child.wait();
    }
}

impl RigRendererFixture {
    pub(super) fn new() -> Self {
        let root =
            std::env::temp_dir().join(format!("sprite-studio-rig-v2-test-{}", Uuid::new_v4()));
        initialize_workspace(&root).expect("workspace should initialize");
        std::fs::create_dir_all(root.join(".sprite-studio/rigs"))
            .expect("rig directory should exist");

        let mut master = RgbaImage::new(32, 32);
        for y in 4..14 {
            for x in 8..24 {
                master.put_pixel(x, y, Rgba([62, 96, 148, 255]));
            }
        }
        for y in 12..27 {
            for x in 9..13 {
                master.put_pixel(x, y, Rgba([214, 132, 78, 255]));
            }
            for x in 19..23 {
                master.put_pixel(x, y, Rgba([196, 105, 66, 255]));
            }
        }
        master
            .save(root.join("assets/characters/walker.png"))
            .expect("rig master should save");
        Self { root }
    }

    pub(super) fn validate(&self, name: &str, spec: &Value) -> std::process::Output {
        self.run(name, spec, true)
    }

    pub(super) fn write_rig(&self, name: &str, spec: &Value) -> (PathBuf, Vec<u8>) {
        let path = self.root.join(format!(".sprite-studio/rigs/{name}.json"));
        let bytes = serde_json::to_vec(spec).expect("rig should serialize");
        std::fs::write(&path, &bytes).expect("rig should write");
        (path, bytes)
    }

    pub(super) fn render(&self, name: &str, spec: &Value) -> std::process::Output {
        self.run(name, spec, false)
    }

    pub(super) fn run(
        &self,
        name: &str,
        spec: &Value,
        validate_only: bool,
    ) -> std::process::Output {
        let relative = format!(".sprite-studio/rigs/{name}.json");
        std::fs::write(
            self.root.join(&relative),
            serde_json::to_vec_pretty(spec).expect("rig should serialize"),
        )
        .expect("rig should write");
        let mut command = Command::new("python3");
        command
            .current_dir(&self.root)
            .arg(".sprite-studio/sprite_rig.py");
        if validate_only {
            command.arg("--validate");
        }
        command
            .arg(&relative)
            .output()
            .expect("python3 should execute the bundled rig renderer")
    }
}

impl Drop for RigRendererFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn leg_transform(upper_rotate: i32, lower_rotate: i32) -> Value {
    json!({
        "left_upper": {"rotate": upper_rotate},
        "left_lower": {"rotate": lower_rotate},
    })
}

pub(super) fn legacy_color_rig() -> Value {
    json!({
        "rigVersion": 1,
        "name": "legacy_color_validation",
        "category": "characters",
        "source": "assets/characters/walker.png",
        "fps": 8,
        "palette": {"accent": "#ff8844"},
        "parts": [{
            "name": "leg",
            "mask": {"rect": [9, 12, 4, 15]},
            "pivot": [11, 12],
            "z": 1,
        }],
        "frames": [
            {
                "transforms": {"leg": {"rotate": -5}},
                "overlay": [{"type": "pixel", "x": 1, "y": 1, "color": "accent"}],
            },
            {
                "transforms": {"leg": {"rotate": 5}},
                "overlay": [{"type": "pixel", "x": 2, "y": 1, "color": "accent"}],
            },
        ],
    })
}

pub(super) fn good_biped_walk() -> Value {
    let phases = [
        "left_contact",
        "left_down",
        "left_passing",
        "left_to_right_double_support",
        "right_contact",
        "right_down",
        "right_passing",
        "right_to_left_double_support",
    ];
    let left_angles = [
        (0, 0),
        (0, 0),
        (0, 0),
        (0, 0),
        (-8, 8),
        (-4, 4),
        (8, -8),
        (0, 0),
    ];
    let right_angles = [
        (-8, 8),
        (-4, 4),
        (8, -8),
        (0, 0),
        (0, 0),
        (0, 0),
        (0, 0),
        (0, 0),
    ];
    let frames = (0..8)
        .map(|index| {
            let mut transforms = leg_transform(left_angles[index].0, left_angles[index].1);
            let object = transforms
                .as_object_mut()
                .expect("transforms should be an object");
            object.insert(
                "right_upper".into(),
                json!({"rotate": right_angles[index].0}),
            );
            object.insert(
                "right_lower".into(),
                json!({"rotate": right_angles[index].1}),
            );
            object.insert(
                "torso".into(),
                json!({
                    "dx": if index == 3 { -1 } else if index == 7 { 1 } else { 0 },
                    "dy": if index == 5 { 1 } else { 0 },
                }),
            );
            let mut contacts = Vec::new();
            if index <= 3 || index == 7 {
                contacts.push(json!({
                    "part": "left_lower",
                    "anchor": "foot",
                    "state": "planted",
                }));
            }
            if index >= 3 {
                contacts.push(json!({
                    "part": "right_lower",
                    "anchor": "foot",
                    "state": "planted",
                }));
            }
            json!({
                "phase": phases[index],
                "contacts": contacts,
                "root": {"dx": 0, "dy": 0},
                "transforms": transforms,
            })
        })
        .collect::<Vec<_>>();

    json!({
        "rigVersion": 2,
        "name": "regression_biped_walk",
        "category": "characters",
        "source": "assets/characters/walker.png",
        "fps": 8,
        "looping": true,
        "rootMotion": "in-place",
        "proposal": {
            "morphologyTag": "biped",
            "motionIntent": "walking in a seamless in-place loop",
        },
        "parts": [
            {
                "name": "torso",
                "mask": {"rect": [8, 4, 16, 8]},
                "pivot": [16, 11],
                "z": 0,
                "role": "torso",
                "anchors": {"center": [16, 8]},
            },
            {
                "name": "left_upper",
                "mask": {"rect": [9, 12, 4, 7]},
                "pivot": [11, 12],
                "z": 2,
                "role": "left_upper_leg",
                "anchors": {"hip": [11, 12], "knee": [11, 19]},
            },
            {
                "name": "left_lower",
                "mask": {"rect": [9, 19, 4, 8]},
                "pivot": [11, 19],
                "z": 3,
                "role": "left_lower_leg",
                "parent": "left_upper",
                "anchors": {"knee": [11, 19], "foot": [11, 27]},
                "attach": {"parentAnchor": "knee", "selfAnchor": "knee"},
            },
            {
                "name": "right_upper",
                "mask": {"rect": [19, 12, 4, 7]},
                "pivot": [21, 12],
                "z": 1,
                "role": "right_upper_leg",
                "anchors": {"hip": [21, 12], "knee": [21, 19]},
            },
            {
                "name": "right_lower",
                "mask": {"rect": [19, 19, 4, 8]},
                "pivot": [21, 19],
                "z": 2,
                "role": "right_lower_leg",
                "parent": "right_upper",
                "anchors": {"knee": [21, 19], "foot": [21, 27]},
                "attach": {"parentAnchor": "knee", "selfAnchor": "knee"},
            },
        ],
        "frames": frames,
    })
}

pub(super) fn ik_biped_walk() -> Value {
    let mut rig = good_biped_walk();
    let left_targets = [
        (11, 26),
        (11, 26),
        (11, 26),
        (11, 26),
        (14, 26),
        (13, 24),
        (11, 25),
        (11, 26),
    ];
    let right_targets = [
        (18, 26),
        (19, 24),
        (21, 25),
        (21, 26),
        (21, 26),
        (21, 26),
        (21, 26),
        (21, 26),
    ];
    for index in 0..8 {
        let torso = rig["frames"][index]["transforms"]["torso"].clone();
        rig["frames"][index]["transforms"] = json!({"torso": torso});
        rig["frames"][index]["ik"] = json!([
            {
                "chain": ["left_upper", "left_lower"],
                "endAnchor": "foot",
                "target": [left_targets[index].0, left_targets[index].1],
                "bend": 1,
            },
            {
                "chain": ["right_upper", "right_lower"],
                "endAnchor": "foot",
                "target": [right_targets[index].0, right_targets[index].1],
                "bend": -1,
            },
        ]);
    }
    rig
}

pub(super) fn good_v3_human_walk() -> Value {
    let mut rig = good_biped_walk();
    rig["rigVersion"] = json!(3);
    rig["name"] = json!("profiled_human_walk");
    rig["rigProfile"] = json!("human_sprite_rig");
    rig["parts"][0]["mask"] = json!({"rect": [8, 4, 16, 8]});
    rig["parts"][0]["anchors"] = json!({
        "top": [16, 4], "bottom": [16, 12],
        "left_hip": [11, 12], "right_hip": [21, 12]
    });
    rig["parts"][0]["bone"] = json!({"startAnchor": "top", "endAnchor": "bottom", "radius": 8});
    rig["parts"][1]["mask"] = json!({"rect": [9, 14, 4, 5]});
    rig["parts"][1]["pivot"] = json!([11, 14]);
    rig["parts"][1]["anchors"] = json!({"hip": [11, 14], "knee": [11, 19]});
    rig["parts"][1]["parent"] = json!("pelvis");
    rig["parts"][1]["attach"] = json!({"parentAnchor": "left_hip", "selfAnchor": "hip"});
    rig["parts"][3]["mask"] = json!({"rect": [19, 14, 4, 5]});
    rig["parts"][3]["pivot"] = json!([21, 14]);
    rig["parts"][3]["anchors"] = json!({"hip": [21, 14], "knee": [21, 19]});
    rig["parts"][3]["parent"] = json!("pelvis");
    rig["parts"][3]["attach"] = json!({"parentAnchor": "right_hip", "selfAnchor": "hip"});
    for (index, start, end) in [
        (1, "hip", "knee"),
        (2, "knee", "ankle"),
        (3, "hip", "knee"),
        (4, "knee", "ankle"),
    ] {
        rig["parts"][index]["bone"] = json!({"startAnchor": start, "endAnchor": end, "radius": 2});
    }
    rig["parts"][2]["mask"] = json!({"rect": [9, 19, 4, 5]});
    rig["parts"][2]["anchors"] = json!({"knee": [11, 19], "ankle": [11, 24]});
    rig["parts"][4]["mask"] = json!({"rect": [19, 19, 4, 5]});
    rig["parts"][4]["anchors"] = json!({"knee": [21, 19], "ankle": [21, 24]});
    let left_foot = json!({
        "name": "left_foot",
        "mask": {"rect": [9, 24, 4, 3]},
        "pivot": [11, 24],
        "z": 4,
        "role": "left_foot",
        "parent": "left_lower",
        "anchors": {"ankle": [11, 24], "sole": [11, 27]},
        "attach": {"parentAnchor": "ankle", "selfAnchor": "ankle"},
        "bone": {"startAnchor": "ankle", "endAnchor": "sole", "radius": 2}
    });
    let right_foot = json!({
        "name": "right_foot",
        "mask": {"rect": [19, 24, 4, 3]},
        "pivot": [21, 24],
        "z": 3,
        "role": "right_foot",
        "parent": "right_lower",
        "anchors": {"ankle": [21, 24], "sole": [21, 27]},
        "attach": {"parentAnchor": "ankle", "selfAnchor": "ankle"},
        "bone": {"startAnchor": "ankle", "endAnchor": "sole", "radius": 2}
    });
    let pelvis = json!({
        "name": "pelvis",
        "mask": {"rect": [8, 12, 16, 2]},
        "pivot": [16, 12],
        "z": 1,
        "role": "pelvis",
        "parent": "torso",
        "anchors": {
            "top": [16, 12], "bottom": [16, 14],
            "left_hip": [11, 14], "right_hip": [21, 14]
        },
        "attach": {"parentAnchor": "bottom", "selfAnchor": "top"},
        "bone": {"startAnchor": "top", "endAnchor": "bottom", "radius": 8}
    });
    rig["parts"]
        .as_array_mut()
        .expect("parts should be an array")
        .extend([pelvis, left_foot, right_foot]);
    rig["joints"] = json!([
        {"name":"left_hip","kind":"hip","position":[11,14],"visibility":"visible","parts":["pelvis","left_upper"]},
        {"name":"left_knee","kind":"knee","position":[11,19],"visibility":"visible","parts":["left_upper","left_lower"]},
        {"name":"left_ankle","kind":"ankle","position":[11,24],"visibility":"visible","parts":["left_lower","left_foot"]},
        {"name":"right_hip","kind":"hip","position":[21,14],"visibility":"visible","parts":["pelvis","right_upper"]},
        {"name":"right_knee","kind":"knee","position":[21,19],"visibility":"visible","parts":["right_upper","right_lower"]},
        {"name":"right_ankle","kind":"ankle","position":[21,24],"visibility":"visible","parts":["right_lower","right_foot"]}
    ]);
    let poses = [
        "left_contact",
        "left_down",
        "left_passing",
        "left_up",
        "right_contact",
        "right_down",
        "right_passing",
        "right_up",
    ];
    for (index, pose) in poses.into_iter().enumerate() {
        rig["frames"][index]["pose"] = json!(pose);
        rig["frames"][index]["transforms"]["pelvis"] = json!({"dx": 0});
        if index <= 2 {
            // Keep the planted side locked while making the passing leg's pose
            // materially distinct from contact. This avoids a visual pause
            // without introducing root or planted-foot slide.
            let swing = [-15, 0, 15][index];
            rig["frames"][index]["transforms"]["right_upper"]["rotate"] = json!(swing);
            rig["frames"][index]["transforms"]["right_lower"]["rotate"] = json!(-swing);
        }
        let torso_dx = if index == 3 {
            -1
        } else if index == 7 {
            1
        } else {
            0
        };
        rig["frames"][index]["transforms"]["torso"]["dx"] = json!(torso_dx);
        if index == 3 || index == 7 {
            let compensation = -torso_dx;
            rig["frames"][index]["transforms"]["left_foot"] = json!({"dx": compensation});
            rig["frames"][index]["transforms"]["right_foot"] = json!({"dx": compensation});
        }
        let contacts = rig["frames"][index]["contacts"]
            .as_array_mut()
            .expect("contacts should be an array");
        for contact in contacts {
            if contact["part"] == "left_lower" {
                contact["part"] = json!("left_foot");
                contact["anchor"] = json!("sole");
            } else if contact["part"] == "right_lower" {
                contact["part"] = json!("right_foot");
                contact["anchor"] = json!("sole");
            }
        }
    }
    rig
}

pub(super) fn assert_validation_failed(
    output: &std::process::Output,
    case: &str,
    expected_diagnostic: &str,
) {
    assert!(
        !output.status.success(),
        "{case} unexpectedly passed validation: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    let diagnostic = String::from_utf8_lossy(&output.stderr);
    assert!(
        diagnostic.contains("sprite_rig:"),
        "{case} did not emit a renderer diagnostic: {diagnostic}"
    );
    assert!(
        diagnostic.contains(expected_diagnostic),
        "{case} failed for the wrong reason; expected {expected_diagnostic:?}: {diagnostic}"
    );
    assert!(
        !diagnostic.contains("Traceback"),
        "{case} crashed instead of producing a validation diagnostic: {diagnostic}"
    );
}

pub(super) fn fixture() -> (PathBuf, AppState) {
    let root =
        std::env::temp_dir().join(format!("sprite-studio-workspace-test-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&root).expect("temporary directory should be created");
    let connection = database::open(&root.join("app.sqlite3")).expect("database should open");
    (root, AppState::from_connection(connection))
}
