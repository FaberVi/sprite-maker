"""Rig profile, morphology context, and bone/anchor normalization."""

from sprite_rig_json import exact_tokens, fail, finite_number
from sprite_rig_raster import nearest_visible_distance
from sprite_tool import slug

def locomotion_context(spec):
    proposal = spec.get("proposal", {})
    if not isinstance(proposal, dict):
        proposal = {}
    morphology = str(proposal.get("morphologyTag", "")).strip().lower()
    motion_text = " ".join((
        str(spec.get("name", "")),
        str(proposal.get("motionIntent", "")),
    )).lower()
    locomotion_verbs = {
        "walk", "walking", "run", "running", "sprint", "gallop", "trot",
        "hop", "hopping", "bound", "bounding", "pounce", "crawl", "slither",
        "swim", "fly", "flap", "takeoff", "take-off",
    }
    motion_tokens = exact_tokens(motion_text.replace(" ", "_"))
    is_locomotion = bool(motion_tokens.intersection(locomotion_verbs))
    is_grounded_walk = bool(
        motion_tokens.intersection({"walk", "walking", "trot", "crawl", "slither"})
    )
    return morphology, motion_text, is_locomotion, is_grounded_walk


def normalize_anchors(part, width, height, label):
    anchors = part.get("anchors", {})
    if not isinstance(anchors, dict) or not anchors:
        fail(f"{label} requires at least one named anchor in rigVersion 2")
    normalized = {}
    for raw_name, point in anchors.items():
        anchor_name = slug(raw_name)
        if anchor_name in normalized:
            fail(f"{label} anchor names must be unique")
        if not isinstance(point, list) or len(point) != 2:
            fail(f"{label} anchor {anchor_name!r} must be [x,y]")
        x = finite_number(point[0], f"{label} anchor {anchor_name!r} x")
        y = finite_number(point[1], f"{label} anchor {anchor_name!r} y")
        if not (0 <= x <= width and 0 <= y <= height):
            fail(f"{label} anchor {anchor_name!r} lies outside the locked master")
        normalized[anchor_name] = [x, y]
    part["anchors"] = normalized
    return normalized


RIG_PROFILE_MORPHOLOGIES = {
    "human_sprite_rig": {"biped"},
    "four_leg_sprite_rig": {"quadruped"},
    "multi_leg_sprite_rig": {"hexapod", "segmented-many-leg"},
    "serpentine_sprite_rig": {"serpentine"},
    "winged_sprite_rig": {"winged"},
}


def normalize_bone(part, label):
    bone = part.get("bone")
    if not isinstance(bone, dict) or set(bone) != {"startAnchor", "endAnchor", "radius"}:
        fail(f"{label} in rigVersion 3 requires bone with startAnchor, endAnchor, and radius")
    start_anchor = slug(bone.get("startAnchor", ""))
    end_anchor = slug(bone.get("endAnchor", ""))
    if start_anchor == end_anchor or start_anchor not in part["anchors"] or end_anchor not in part["anchors"]:
        fail(f"{label} bone endpoints must name two distinct anchors on that part")
    radius = finite_number(bone.get("radius"), f"{label} bone radius")
    if not (0.5 <= radius <= 32):
        fail(f"{label} bone radius must be between 0.5 and 32 pixels")
    part["bone"] = {
        "startAnchor": start_anchor,
        "endAnchor": end_anchor,
        "radius": radius,
    }


def validate_rig_profile(spec, parts, frames, width, height, source):
    """Validate rigVersion 3 anatomy, visible joints, and named key poses."""
    morphology, _, is_locomotion, _ = locomotion_context(spec)
    profile = str(spec.get("rigProfile", "")).strip().lower()
    if profile not in RIG_PROFILE_MORPHOLOGIES:
        fail(
            "rigVersion 3 requires rigProfile: human_sprite_rig, four_leg_sprite_rig, "
            "multi_leg_sprite_rig, serpentine_sprite_rig, or winged_sprite_rig"
        )
    if morphology not in RIG_PROFILE_MORPHOLOGIES[profile]:
        fail(f"rigProfile {profile!r} does not match morphology {morphology!r}")
    spec["rigProfile"] = profile

    part_by_name = {part["name"]: part for part in parts}
    raw_joints = spec.get("joints")
    if not isinstance(raw_joints, list) or not raw_joints:
        fail("rigVersion 3 requires an observed joints array")
    joints = []
    joint_names = set()
    for index, raw_joint in enumerate(raw_joints):
        label = f"joint {index + 1}"
        if not isinstance(raw_joint, dict) or set(raw_joint) != {
            "name", "kind", "position", "visibility", "parts",
        }:
            fail(f"{label} requires only name, kind, position, visibility, and parts")
        name = slug(raw_joint.get("name", ""))
        kind = slug(raw_joint.get("kind", ""))
        visibility = str(raw_joint.get("visibility", "")).strip().lower()
        raw_parts = raw_joint.get("parts")
        if not name or name in joint_names:
            fail("joint names must be unique and non-empty")
        joint_names.add(name)
        if visibility not in {"visible", "occluded"}:
            fail(f"{label} visibility must be 'visible' or 'occluded'")
        if (
            not isinstance(raw_parts, list) or len(raw_parts) != 2
            or any(slug(value) not in part_by_name for value in raw_parts)
        ):
            fail(f"{label} parts must name two existing anatomical parts")
        joint_parts = [slug(value) for value in raw_parts]
        if joint_parts[0] == joint_parts[1]:
            fail(f"{label} must connect two different parts")
        position = raw_joint.get("position")
        if not isinstance(position, list) or len(position) != 2:
            fail(f"{label} position must be [x,y]")
        joint_x = finite_number(position[0], f"{label} position x")
        joint_y = finite_number(position[1], f"{label} position y")
        if not (0 <= joint_x <= width and 0 <= joint_y <= height):
            fail(f"{label} position lies outside the locked master")
        normalized = {
            "name": name,
            "kind": kind,
            "position": [joint_x, joint_y],
            "visibility": visibility,
            "parts": joint_parts,
        }
        for part_name in joint_parts:
            part = part_by_name[part_name]
            distance = nearest_visible_distance(
                source, width, height, part["mask"], normalized["position"],
            )
            limit = 1.6 if visibility == "visible" else 3.0
            if distance > limit:
                fail(
                    f"joint {name!r} is marked {visibility} but sits {distance:.2f}px "
                    f"from part {part_name!r}; the anatomy is not actually segmented at that joint"
                )
        joints.append(normalized)
    spec["joints"] = joints

    required = set()
    required_visible = set()
    if profile == "human_sprite_rig" and is_locomotion:
        required = {
            "left_hip", "left_knee", "left_ankle",
            "right_hip", "right_knee", "right_ankle",
        }
        required_visible = required
    elif profile == "four_leg_sprite_rig" and is_locomotion:
        required = {
            f"{side}_{limb}_{joint}"
            for side in ("near", "far")
            for limb, joints_for_limb in (
                ("hind", ("hip", "knee", "ankle")),
                ("fore", ("shoulder", "elbow", "wrist")),
            )
            for joint in joints_for_limb
        }
        required_visible = {
            "near_hind_hip", "near_hind_knee", "near_hind_ankle",
            "near_fore_shoulder", "near_fore_elbow", "near_fore_wrist",
        }
    missing = required - joint_names
    if missing:
        fail(f"{profile} is missing observed joints: {', '.join(sorted(missing))}")
    visibility_by_name = {joint["name"]: joint["visibility"] for joint in joints}
    hidden_required = {
        name for name in required_visible if visibility_by_name.get(name) != "visible"
    }
    if hidden_required:
        fail(
            f"{profile} requires visible gameplay-side joints: "
            f"{', '.join(sorted(hidden_required))}; create a motion-ready source revision"
        )

    pose_names = []
    for index, frame in enumerate(frames):
        pose = slug(frame.get("pose", ""))
        if is_locomotion and not pose:
            fail(f"rigVersion 3 locomotion frame {index + 1} requires a named key pose")
        frame["pose"] = pose
        pose_names.append(pose)
    if is_locomotion:
        pose_tokens = set().union(*(exact_tokens(pose) for pose in pose_names))
        motion_tokens = exact_tokens(spec.get("proposal", {}).get("motionIntent", ""))
        if profile in {"human_sprite_rig", "four_leg_sprite_rig"}:
            if not {"contact", "passing"}.issubset(pose_tokens) and not motion_tokens.intersection(
                {"run", "running", "gallop", "bound", "bounding", "sprint"}
            ):
                fail("a walk cycle requires named contact and passing key poses")
        if profile == "four_leg_sprite_rig" and motion_tokens.intersection(
            {"run", "running", "gallop", "bound", "bounding", "sprint"}
        ):
            required_poses = {"hind", "fore", "extended", "gathered", "contact", "flight"}
            if not required_poses.issubset(pose_tokens):
                fail(
                    "a four-leg run requires named hind_contact, extended_flight, "
                    "fore_contact, and gathered_flight poses"
                )


