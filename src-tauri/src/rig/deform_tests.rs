use super::ik::{bone_world_affine, solve_contact_ik};
use super::mesh::build_deform_mesh;
use super::validate::{validate_perceptible_rig_motion, validate_rig_loop_motion};
use super::{
    build_ownership, point_positions, render_frame, render_frames, Affine, Rig, RigBone,
    RigContact, RigFrame, RigPoint, RigTransform,
};
use image::RgbaImage;
use std::collections::HashMap;

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

#[test]
fn affine_inverse_round_trips() {
    let matrix = Affine::identity()
        .chain(&Affine::translation(5.0, -3.0))
        .chain(&Affine::rotation_degrees(37.0))
        .chain(&Affine::scaling(1.5, 0.75));
    let inverse = matrix.inverse();
    for (x, y) in [(0.0, 0.0), (12.0, 4.0), (-7.0, 33.5)] {
        let (tx, ty) = matrix.apply(x, y);
        let (rx, ry) = inverse.apply(tx, ty);
        assert!((rx - x).abs() < 1e-9, "x should round trip");
        assert!((ry - y).abs() < 1e-9, "y should round trip");
    }
}

#[test]
fn ownership_covers_every_opaque_pixel_once() {
    let mut rig = blank_rig();
    rig.points = vec![point("a", 8.0, 0.0), point("b", 8.0, 16.0)];
    rig.bones = vec![RigBone {
        id: "b1".into(),
        name: "limb".into(),
        start_point: "a".into(),
        end_point: "b".into(),
        radius: 8.0,
        parent: None,
        z: 1,
    }];
    let master = solid_square(16);
    let positions = point_positions(&rig, 16, 16);
    let ownership = build_ownership(&master, &rig, &positions);
    for value in ownership {
        assert_eq!(
            value, 0,
            "every opaque pixel should belong to the only bone"
        );
    }
}

#[test]
fn deform_mesh_is_deterministic_and_weights_are_normalized() {
    let mut rig = blank_rig();
    rig.points = vec![
        point("chest", 8.0, 2.0),
        point("hip", 8.0, 9.0),
        point("foot", 8.0, 15.0),
    ];
    rig.bones = vec![
        RigBone {
            id: "body".into(),
            name: "torso".into(),
            start_point: "chest".into(),
            end_point: "hip".into(),
            radius: 5.0,
            parent: None,
            z: 0,
        },
        RigBone {
            id: "leg".into(),
            name: "leg".into(),
            start_point: "hip".into(),
            end_point: "foot".into(),
            radius: 3.0,
            parent: Some("torso".into()),
            z: 1,
        },
    ];
    let master = solid_square(16);
    let positions = point_positions(&rig, 16, 16);
    let first = build_deform_mesh(&master, &rig, &positions);
    let second = build_deform_mesh(&master, &rig, &positions);
    assert_eq!(first.vertices.len(), second.vertices.len());
    assert_eq!(first.triangles.len(), second.triangles.len());
    for (left, right) in first.vertices.iter().zip(&second.vertices) {
        assert_eq!(left.source, right.source);
        assert_eq!(left.influences, right.influences);
        let total: f64 = left.influences.iter().map(|(_, weight)| *weight).sum();
        assert!((total - 1.0).abs() < 1e-9, "weights must sum to one");
        assert!(
            left.influences.len() <= 4,
            "only four influences are retained"
        );
    }
}

#[test]
fn torso_transform_deforms_a_multi_bone_sprite() {
    let mut rig = blank_rig();
    rig.points = vec![
        point("chest", 8.0, 3.0),
        point("hip", 8.0, 10.0),
        point("foot", 8.0, 15.0),
    ];
    rig.bones = vec![
        RigBone {
            id: "body".into(),
            name: "torso".into(),
            start_point: "chest".into(),
            end_point: "hip".into(),
            radius: 6.0,
            parent: None,
            z: 0,
        },
        RigBone {
            id: "leg".into(),
            name: "leg".into(),
            start_point: "hip".into(),
            end_point: "foot".into(),
            radius: 3.0,
            parent: Some("torso".into()),
            z: 1,
        },
    ];
    let identity = RigFrame {
        phase: None,
        hold: false,
        root_dx: 0.0,
        root_dy: 0.0,
        transforms: vec![],
        contacts: vec![],
    };
    let moving = RigFrame {
        transforms: vec![RigTransform {
            bone: "torso".into(),
            dx: 2.0,
            dy: 0.0,
            rotate: 15.0,
            scale_x: 1.08,
            scale_y: 0.92,
        }],
        ..identity.clone()
    };
    let master = solid_square(16);
    let positions = point_positions(&rig, 16, 16);
    let ownership = build_ownership(&master, &rig, &positions);
    let still = render_frame(&master, &rig, &identity, &ownership, &positions);
    let animated = render_frame(&master, &rig, &moving, &ownership, &positions);
    assert_eq!(
        still.as_raw(),
        master.as_raw(),
        "an identity pose must preserve the source sprite exactly"
    );
    assert_ne!(
        still.as_raw(),
        animated.as_raw(),
        "a visible torso transform must deform the rendered body"
    );
    assert!(
        animated.pixels().any(|pixel| pixel[3] > 0),
        "smooth deformation must keep visible source pixels"
    );
}

#[test]
fn rotation_render_preserves_pixel_count() {
    let mut rig = blank_rig();
    rig.points = vec![point("a", 8.0, 8.0), point("b", 8.0, 1.0)];
    rig.bones = vec![RigBone {
        id: "b1".into(),
        name: "arm".into(),
        start_point: "a".into(),
        end_point: "b".into(),
        radius: 3.0,
        parent: None,
        z: 1,
    }];
    rig.frames = vec![RigFrame {
        phase: None,
        hold: false,
        root_dx: 0.0,
        root_dy: 0.0,
        transforms: vec![RigTransform {
            bone: "arm".into(),
            dx: 0.0,
            dy: 0.0,
            rotate: 90.0,
            scale_x: 1.0,
            scale_y: 1.0,
        }],
        contacts: vec![],
    }];
    let master = solid_square(16);
    let frames = render_frames(&master, &rig);
    assert_eq!(frames.len(), 1);
    let opaque_source = master.pixels().filter(|pixel| pixel[3] > 0).count();
    let opaque_rendered = frames[0].pixels().filter(|pixel| pixel[3] > 0).count();
    assert_eq!(
        opaque_source, opaque_rendered,
        "a 90° rotation should not drop pixels"
    );
}

#[test]
fn render_is_deterministic() {
    let mut rig = blank_rig();
    rig.points = vec![point("a", 8.0, 2.0), point("b", 8.0, 14.0)];
    rig.bones = vec![RigBone {
        id: "b1".into(),
        name: "limb".into(),
        start_point: "a".into(),
        end_point: "b".into(),
        radius: 4.0,
        parent: None,
        z: 1,
    }];
    rig.frames = vec![
        RigFrame {
            phase: None,
            hold: false,
            root_dx: 0.0,
            root_dy: 0.0,
            transforms: vec![RigTransform {
                bone: "limb".into(),
                dx: 1.0,
                dy: 0.0,
                rotate: 12.0,
                scale_x: 1.0,
                scale_y: 1.0,
            }],
            contacts: vec![],
        },
        RigFrame {
            phase: None,
            hold: false,
            root_dx: 0.0,
            root_dy: 1.0,
            transforms: vec![RigTransform {
                bone: "limb".into(),
                dx: -1.0,
                dy: 0.0,
                rotate: -12.0,
                scale_x: 1.0,
                scale_y: 1.0,
            }],
            contacts: vec![],
        },
    ];
    let master = solid_square(16);
    let first = render_frames(&master, &rig);
    let second = render_frames(&master, &rig);
    for (a, b) in first.iter().zip(second.iter()) {
        assert_eq!(
            a.as_raw(),
            b.as_raw(),
            "identical rigs must render identical bytes"
        );
    }
}

#[test]
fn final_render_rejects_numerical_but_imperceptible_pose_changes() {
    let mut rig = blank_rig();
    rig.morphology = "biped".into();
    rig.bones = vec![
        RigBone {
            id: "torso".into(),
            name: "torso".into(),
            start_point: "chest".into(),
            end_point: "hip".into(),
            radius: 3.0,
            parent: None,
            z: 0,
        },
        RigBone {
            id: "left".into(),
            name: "left_leg".into(),
            start_point: "hip".into(),
            end_point: "left_foot".into(),
            radius: 2.0,
            parent: None,
            z: 1,
        },
        RigBone {
            id: "right".into(),
            name: "right_leg".into(),
            start_point: "hip".into(),
            end_point: "right_foot".into(),
            radius: 2.0,
            parent: None,
            z: 2,
        },
    ];
    let pose = |angle: f64, body_angle: f64| RigFrame {
        phase: None,
        hold: false,
        root_dx: 0.0,
        root_dy: 0.0,
        transforms: vec![
            RigTransform {
                bone: "torso".into(),
                dx: 0.0,
                dy: 0.0,
                rotate: body_angle,
                scale_x: 1.0,
                scale_y: 1.0,
            },
            RigTransform {
                bone: "left_leg".into(),
                dx: 0.0,
                dy: 0.0,
                rotate: angle,
                scale_x: 1.0,
                scale_y: 1.0,
            },
            RigTransform {
                bone: "right_leg".into(),
                dx: 0.0,
                dy: 0.0,
                rotate: -angle,
                scale_x: 1.0,
                scale_y: 1.0,
            },
        ],
        contacts: vec![],
    };
    rig.frames = vec![pose(-2.0, 0.0), pose(2.0, 0.0)];
    let error = validate_perceptible_rig_motion(&rig).expect_err("tiny transforms must fail");
    assert_eq!(error.code, "imperceptible_rig_motion");

    rig.frames = vec![pose(-12.0, 0.0), pose(12.0, 0.0)];
    let error = validate_perceptible_rig_motion(&rig).expect_err("rigid body must fail");
    assert_eq!(error.code, "missing_body_motion");

    rig.frames = vec![pose(-12.0, -6.0), pose(12.0, 6.0)];
    validate_perceptible_rig_motion(&rig).expect("readable opposing poses should pass");
}

#[test]
fn loop_validation_rejects_adjacent_duplicate_poses_and_root_drift() {
    let mut rig = blank_rig();
    rig.points = vec![point("a", 8.0, 8.0), point("b", 8.0, 1.0)];
    rig.bones = vec![RigBone {
        id: "b1".into(),
        name: "arm".into(),
        start_point: "a".into(),
        end_point: "b".into(),
        radius: 3.0,
        parent: None,
        z: 1,
    }];
    let frame = RigFrame {
        phase: None,
        hold: false,
        root_dx: 0.0,
        root_dy: 0.0,
        transforms: vec![RigTransform {
            bone: "arm".into(),
            dx: 0.0,
            dy: 0.0,
            rotate: 18.0,
            scale_x: 1.0,
            scale_y: 1.0,
        }],
        contacts: vec![],
    };
    rig.frames = vec![frame.clone(), frame];
    let duplicate = validate_rig_loop_motion(&rig).expect_err("duplicate poses must fail");
    assert_eq!(duplicate.code, "duplicate_rig_frames");

    rig.frames = vec![
        RigFrame {
            phase: None,
            hold: false,
            root_dx: 0.0,
            root_dy: 0.0,
            transforms: vec![RigTransform {
                bone: "arm".into(),
                dx: 0.0,
                dy: 0.0,
                rotate: 0.0,
                scale_x: 1.0,
                scale_y: 1.0,
            }],
            contacts: vec![],
        },
        RigFrame {
            phase: None,
            hold: false,
            root_dx: 20.0,
            root_dy: 0.0,
            transforms: vec![RigTransform {
                bone: "arm".into(),
                dx: 0.0,
                dy: 0.0,
                rotate: 12.0,
                scale_x: 1.0,
                scale_y: 1.0,
            }],
            contacts: vec![],
        },
    ];
    let drift = validate_rig_loop_motion(&rig).expect_err("loop drift must fail");
    assert_eq!(drift.code, "root_loop_drift");
}

#[test]
fn parent_rotation_carries_child_pixels() {
    let mut rig = blank_rig();
    rig.points = vec![
        point("root", 8.0, 8.0),
        point("mid", 8.0, 4.0),
        point("tip", 8.0, 0.0),
    ];
    rig.bones = vec![
        RigBone {
            id: "b1".into(),
            name: "parent".into(),
            start_point: "root".into(),
            end_point: "mid".into(),
            radius: 2.0,
            parent: None,
            z: 1,
        },
        RigBone {
            id: "b2".into(),
            name: "child".into(),
            start_point: "mid".into(),
            end_point: "tip".into(),
            radius: 2.0,
            parent: Some("parent".into()),
            z: 2,
        },
    ];
    rig.frames = vec![RigFrame {
        phase: None,
        hold: false,
        root_dx: 0.0,
        root_dy: 0.0,
        transforms: vec![RigTransform {
            bone: "parent".into(),
            dx: 0.0,
            dy: 0.0,
            rotate: -90.0,
            scale_x: 1.0,
            scale_y: 1.0,
        }],
        contacts: vec![],
    }];
    let master = solid_square(16);
    let positions = point_positions(&rig, 16, 16);
    let ownership = build_ownership(&master, &rig, &positions);
    let rendered = render_frame(&master, &rig, &rig.frames[0], &ownership, &positions);
    // The child tip (8,0) rotates around the parent start (8,8) by -90°
    // (counter-clockwise on screen) to land at (0,8).
    let child_owner = ownership[8];
    assert_eq!(child_owner, 1, "the tip pixel belongs to the child bone");
    let expected = master.get_pixel(8, 0);
    let landed = rendered.get_pixel(0, 8);
    assert_eq!(
        expected, landed,
        "the child tip should follow the parent rotation"
    );
}

#[test]
fn contact_ik_reaches_target() {
    let mut rig = blank_rig();
    rig.points = vec![
        point("hip", 8.0, 4.0),
        point("knee", 8.0, 9.0),
        point("foot", 8.0, 14.0),
    ];
    rig.bones = vec![
        RigBone {
            id: "b1".into(),
            name: "thigh".into(),
            start_point: "hip".into(),
            end_point: "knee".into(),
            radius: 2.0,
            parent: None,
            z: 1,
        },
        RigBone {
            id: "b2".into(),
            name: "shin".into(),
            start_point: "knee".into(),
            end_point: "foot".into(),
            radius: 2.0,
            parent: Some("thigh".into()),
            z: 1,
        },
    ];
    // Plant the foot one pixel to the side of its bind position.
    rig.frames = vec![RigFrame {
        phase: None,
        hold: false,
        root_dx: 0.0,
        root_dy: 0.0,
        transforms: vec![],
        contacts: vec![RigContact {
            bone: "shin".into(),
            x: 10.0,
            y: 14.0,
            bend: 1.0,
        }],
    }];
    let master = solid_square(16);
    let positions = point_positions(&rig, 16, 16);
    let ownership = build_ownership(&master, &rig, &positions);
    let frame = &rig.frames[0];
    // Derive the effective transforms exactly like render_frame does.
    let mut transforms: HashMap<String, RigTransform> = HashMap::new();
    for transform in &frame.transforms {
        transforms.insert(transform.bone.clone(), transform.clone());
    }
    let index = rig
        .bones
        .iter()
        .position(|bone| bone.name == "shin")
        .unwrap();
    let (parent_delta, child_delta) =
        solve_contact_ik(&rig.bones, index, &positions, (10.0, 14.0), 1.0);
    transforms.insert(
        "thigh".into(),
        RigTransform {
            bone: "thigh".into(),
            dx: 0.0,
            dy: 0.0,
            rotate: parent_delta,
            scale_x: 1.0,
            scale_y: 1.0,
        },
    );
    transforms.insert(
        "shin".into(),
        RigTransform {
            bone: "shin".into(),
            dx: 0.0,
            dy: 0.0,
            rotate: child_delta,
            scale_x: 1.0,
            scale_y: 1.0,
        },
    );
    let mut cache = HashMap::new();
    let thigh = bone_world_affine(0, &rig.bones, &positions, &transforms, &mut cache, 0);
    let shin = bone_world_affine(1, &rig.bones, &positions, &transforms, &mut cache, 0);
    let (fx, fy) = shin.apply(8.0, 14.0);
    let distance = ((fx - 10.5).powi(2) + (fy - 14.5).powi(2)).sqrt();
    assert!(
        distance < 1.2,
        "IK should plant the foot near the target, got distance {distance}"
    );
    let rendered = render_frame(&master, &rig, frame, &ownership, &positions);
    let opaque = rendered.pixels().filter(|pixel| pixel[3] > 0).count();
    assert!(opaque > 0, "the IK frame should still render pixels");
    let _ = thigh;
}
