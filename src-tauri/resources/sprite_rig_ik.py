"""Two-bone IK solving, planted-contact parsing, and world-rotation checks."""

import math

from sprite_rig_json import fail, finite_number, point_distance
from sprite_rig_matrix import (
    apply_matrix,
    invert_matrix,
    resolve_frame_matrices,
    translation_matrix,
)
from sprite_tool import slug

def parse_contacts(frame, frame_index, part_by_name):
    raw_contacts = frame.get("contacts", [])
    if not isinstance(raw_contacts, list):
        fail(f"frame {frame_index + 1} contacts must be an array")
    contacts = []
    seen = set()
    for contact_index, raw_contact in enumerate(raw_contacts):
        label = f"frame {frame_index + 1} contact {contact_index + 1}"
        if isinstance(raw_contact, str) and "." in raw_contact:
            raw_part, raw_anchor = raw_contact.rsplit(".", 1)
            contact = {"part": raw_part, "anchor": raw_anchor, "state": "planted"}
        elif isinstance(raw_contact, dict):
            unexpected = set(raw_contact) - {"part", "anchor", "state"}
            if unexpected:
                fail(f"{label} has unsupported keys: {', '.join(sorted(unexpected))}")
            contact = dict(raw_contact)
        else:
            fail(f"{label} must be an object with part, anchor, and planted state")
        part_name = slug(contact.get("part", ""))
        anchor_name = slug(contact.get("anchor", ""))
        state = str(contact.get("state", "planted")).strip().lower()
        if state != "planted":
            fail(f"{label} state must be 'planted'; omit lifted contacts")
        part = part_by_name.get(part_name)
        if part is None:
            fail(f"{label} refers to unknown part {part_name!r}")
        if anchor_name not in part.get("anchors", {}):
            fail(f"{label} refers to unknown anchor {part_name}.{anchor_name}")
        key = (part_name, anchor_name)
        if key in seen:
            fail(f"{label} duplicates {part_name}.{anchor_name}")
        seen.add(key)
        contacts.append({"part": part_name, "anchor": anchor_name, "state": "planted"})
    frame["contacts"] = contacts
    return contacts

def rotated_vector(vector, angle_degrees):
    angle = math.radians(float(angle_degrees))
    cosine, sine = math.cos(angle), math.sin(angle)
    return (
        cosine * vector[0] - sine * vector[1],
        sine * vector[0] + cosine * vector[1],
    )


def apply_ik_constraints(parts, frame, frame_index, width, height):
    """Solve optional two-bone IK chains into deterministic local rotations."""
    raw_constraints = frame.get("ik", [])
    if not isinstance(raw_constraints, list):
        fail(f"frame {frame_index + 1} ik must be an array")
    part_by_name = {part["name"]: part for part in parts}
    normalized_constraints = []
    constrained_parts = set()
    for constraint_index, raw_constraint in enumerate(raw_constraints):
        label = f"frame {frame_index + 1} ik {constraint_index + 1}"
        if not isinstance(raw_constraint, dict):
            fail(f"{label} must be an object")
        unexpected = set(raw_constraint) - {
            "chain", "endAnchor", "target", "bend", "endRotation",
        }
        if unexpected:
            fail(f"{label} has unsupported keys: {', '.join(sorted(unexpected))}")
        raw_chain = raw_constraint.get("chain")
        if not isinstance(raw_chain, list) or len(raw_chain) not in {2, 3}:
            fail(f"{label} chain must contain upper, lower, and optionally foot parts")
        chain = [slug(name) for name in raw_chain]
        if len(set(chain)) != len(chain) or any(name not in part_by_name for name in chain):
            fail(f"{label} chain contains duplicate or unknown parts")
        upper, lower = (part_by_name[chain[0]], part_by_name[chain[1]])
        shared = constrained_parts.intersection(chain)
        if shared:
            fail(f"{label} reuses already constrained parts: {', '.join(sorted(shared))}")
        constrained_parts.update(chain)
        if lower.get("parent") != upper["name"]:
            fail(f"{label} lower part must be a direct child of its upper part")
        if len(chain) == 3 and part_by_name[chain[2]].get("parent") != lower["name"]:
            fail(f"{label} foot part must be a direct child of its lower part")
        upper_start_name = upper.get("attach", {}).get("selfAnchor")
        if not upper_start_name:
            candidates = [
                name for name in upper["anchors"]
                if any(token in name for token in ("hip", "shoulder", "root", "start"))
            ]
            if len(candidates) != 1:
                fail(
                    f"{label} cannot infer the upper start anchor; attach the upper part "
                    "to its body or name one anchor hip/shoulder/root/start"
                )
            upper_start_name = candidates[0]
        lower_attach = lower.get("attach")
        if not lower_attach:
            fail(f"{label} lower part requires attachment anchors")
        upper_joint_name = lower_attach["parentAnchor"]
        lower_joint_name = lower_attach["selfAnchor"]
        upper_start = upper["anchors"][upper_start_name]
        upper_joint = upper["anchors"][upper_joint_name]
        lower_joint = lower["anchors"][lower_joint_name]
        if point_distance(upper.get("pivot", [0, 0]), upper_start) > 0.25:
            fail(f"{label} upper pivot must coincide with its chain start anchor")
        if point_distance(lower.get("pivot", [0, 0]), lower_joint) > 0.25:
            fail(f"{label} lower pivot must coincide with its attachment anchor")
        end_part = part_by_name[chain[-1]]
        end_anchor_name = slug(raw_constraint.get("endAnchor", ""))
        if end_anchor_name not in end_part["anchors"]:
            fail(f"{label} refers to missing endpoint anchor {end_part['name']}.{end_anchor_name}")
        end_anchor = end_part["anchors"][end_anchor_name]
        if len(chain) == 3:
            foot = end_part
            foot_attach = foot.get("attach")
            if not foot_attach:
                fail(f"{label} foot part requires attachment anchors")
            lower_end_name = foot_attach["parentAnchor"]
            foot_start_name = foot_attach["selfAnchor"]
            lower_end = lower["anchors"][lower_end_name]
            foot_start = foot["anchors"][foot_start_name]
            if point_distance(foot.get("pivot", [0, 0]), foot_start) > 0.25:
                fail(f"{label} foot pivot must coincide with its attachment anchor")
        else:
            lower_end_name = end_anchor_name
            lower_end = lower["anchors"][lower_end_name]
            foot_start = None

        target = raw_constraint.get("target")
        if not isinstance(target, list) or len(target) != 2:
            fail(f"{label} target must be [x,y] in locked-canvas coordinates")
        target_x = finite_number(target[0], f"{label} target x")
        target_y = finite_number(target[1], f"{label} target y")
        if not (0 <= target_x <= width and 0 <= target_y <= height):
            fail(f"{label} target lies outside the locked canvas")
        bend = finite_number(raw_constraint.get("bend", 1), f"{label} bend")
        if bend not in {-1, 1}:
            fail(f"{label} bend must be -1 or 1")
        end_rotation = finite_number(
            raw_constraint.get("endRotation", 0), f"{label} endRotation",
        )

        transforms = frame.setdefault("transforms", {})
        for part_name in chain[:2]:
            transform = transforms.setdefault(part_name, {})
            for key, default in (("dx", 0), ("dy", 0), ("scaleX", 1), ("scaleY", 1)):
                if abs(float(transform.get(key, default)) - default) > 1e-6:
                    fail(f"{label} cannot solve a chain with translated or scaled bone {part_name!r}")

        current_matrices = resolve_frame_matrices(parts, frame)
        parent_name = upper.get("parent")
        if parent_name:
            parent_world = current_matrices[parent_name]
        else:
            root = frame.get("root", {})
            parent_world = translation_matrix(root.get("dx", 0), root.get("dy", 0))
        desired_lower_end_world = (target_x, target_y)
        if len(chain) == 3:
            endpoint_vector = (
                end_anchor[0] - foot_start[0],
                end_anchor[1] - foot_start[1],
            )
            endpoint_vector = rotated_vector(endpoint_vector, end_rotation)
            desired_lower_end_world = (
                target_x - endpoint_vector[0],
                target_y - endpoint_vector[1],
            )
        desired_lower_end = apply_matrix(invert_matrix(parent_world), desired_lower_end_world)

        first_vector = (
            upper_joint[0] - upper_start[0],
            upper_joint[1] - upper_start[1],
        )
        second_vector = (
            lower_end[0] - lower_joint[0],
            lower_end[1] - lower_joint[1],
        )
        first_length = math.hypot(*first_vector)
        second_length = math.hypot(*second_vector)
        target_vector = (
            desired_lower_end[0] - upper_start[0],
            desired_lower_end[1] - upper_start[1],
        )
        target_distance = math.hypot(*target_vector)
        if first_length < 0.5 or second_length < 0.5:
            fail(f"{label} bone anchors must describe two non-zero segment lengths")
        minimum_reach = abs(first_length - second_length)
        maximum_reach = first_length + second_length
        if target_distance < minimum_reach - 1e-5 or target_distance > maximum_reach + 1e-5:
            fail(
                f"{label} target is unreachable at {target_distance:.2f}px; "
                f"the chain reach is {minimum_reach:.2f}–{maximum_reach:.2f}px"
            )
        if target_distance < 1e-6:
            fail(f"{label} target collapses the two-bone chain onto its start anchor")
        relative_cosine = (
            target_distance * target_distance - first_length * first_length - second_length * second_length
        ) / (2 * first_length * second_length)
        relative_angle = bend * math.acos(max(-1.0, min(1.0, relative_cosine)))
        target_angle = math.atan2(target_vector[1], target_vector[0])
        first_angle = target_angle - math.atan2(
            second_length * math.sin(relative_angle),
            first_length + second_length * math.cos(relative_angle),
        )
        bind_first_angle = math.atan2(first_vector[1], first_vector[0])
        bind_second_angle = math.atan2(second_vector[1], second_vector[0])
        upper_rotation = math.degrees(first_angle - bind_first_angle)
        lower_rotation = math.degrees(relative_angle - (bind_second_angle - bind_first_angle))
        transforms[upper["name"]] = {**transforms[upper["name"]], "rotate": upper_rotation}
        transforms[upper["name"]].pop("worldRotate", None)
        transforms[lower["name"]] = {**transforms[lower["name"]], "rotate": lower_rotation}
        transforms[lower["name"]].pop("worldRotate", None)
        if len(chain) == 3:
            foot_transform = transforms.setdefault(end_part["name"], {})
            for key, default in (("dx", 0), ("dy", 0), ("scaleX", 1), ("scaleY", 1)):
                if abs(float(foot_transform.get(key, default)) - default) > 1e-6:
                    fail(f"{label} cannot lock a translated or scaled foot")
            transforms[end_part["name"]] = {
                **foot_transform,
                "worldRotate": end_rotation,
            }
            transforms[end_part["name"]].pop("rotate", None)
        normalized_constraints.append({
            "chain": chain,
            "endAnchor": end_anchor_name,
            "target": [target_x, target_y],
            "bend": int(bend),
            "endRotation": end_rotation,
        })
        solved_matrices = resolve_frame_matrices(parts, frame)
        solved_endpoint = apply_matrix(solved_matrices[end_part["name"]], end_anchor)
        residual = point_distance(solved_endpoint, (target_x, target_y))
        if residual > 0.25:
            fail(f"{label} solver missed its target by {residual:.2f}px")
    frame["ik"] = normalized_constraints


def validate_world_rotation_support(parts, frame, frame_index):
    transforms = frame.get("transforms", {})
    part_by_name = {part["name"]: part for part in parts}
    if "worldRotate" in transforms.get("base", {}):
        fail(f"frame {frame_index + 1} base cannot use worldRotate")
    for part_name, transform in transforms.items():
        if part_name == "base" or "worldRotate" not in transform:
            continue
        current_name = part_name
        while current_name:
            current_transform = transforms.get(current_name, {})
            scale_x = float(current_transform.get("scaleX", 1))
            scale_y = float(current_transform.get("scaleY", 1))
            if scale_x <= 0 or scale_y <= 0 or abs(scale_x - scale_y) > 1e-6:
                fail(
                    f"frame {frame_index + 1} worldRotate on {part_name!r} requires "
                    f"positive uniform scale throughout its ancestor chain; {current_name!r} "
                    f"uses scaleX={scale_x:g}, scaleY={scale_y:g}"
                )
            current_name = part_by_name[current_name].get("parent")

