"""Morphology-specific gait checks for biped, quadruped, and winged rigs."""

import math

from sprite_rig_json import exact_tokens, fail, point_distance
from sprite_rig_matrix import apply_matrix

def part_side(part):
    descriptor = exact_tokens(f"{part['name']}_{part.get('role', '')}")
    if "left" in descriptor:
        return "left"
    if "right" in descriptor:
        return "right"
    if descriptor.intersection({"fore", "front"}):
        return "front"
    if descriptor.intersection({"hind", "rear"}):
        return "hind"
    return part["name"]


def visible_bounds(pixels, width, height):
    """Return the inclusive alpha bounds without depending on palette colors."""
    xs = []
    ys = []
    for y in range(height):
        for x in range(width):
            if pixels[(y * width + x) * 4 + 3]:
                xs.append(x)
                ys.append(y)
    if not xs:
        return None
    return min(xs), min(ys), max(xs), max(ys)


def endpoint_anchor(part):
    """Choose the gameplay contact/end anchor for a hand, foot, or paw part."""
    preferred = ("toe", "sole", "paw", "hoof", "foot", "ground", "tip", "end")
    anchors = part.get("anchors", {})
    for token in preferred:
        for name, point in anchors.items():
            if token in exact_tokens(name):
                return point
    return next(reversed(list(anchors.values())), part.get("pivot", [0, 0]))


def part_rotation_span(part, frames):
    values = [
        float(frame.get("transforms", {}).get(part["name"], {}).get("rotate", 0))
        for frame in frames
    ]
    return max(values, default=0.0) - min(values, default=0.0)



def validate_quadruped_locomotion(
    spec, parts, frames, source, width, height, matrices_by_frame, part_by_name,
    morphology, is_locomotion,
):
    if is_locomotion and morphology == "quadruped":
        limb_groups = set()
        limb_group_counts = {}
        locomotor_parts = []
        for part in parts:
            descriptor = exact_tokens(f"{part['name']}_{part.get('role', '')}")
            axis = "hind" if descriptor.intersection({"hind", "rear"}) else (
                "fore" if descriptor.intersection({"fore", "front"}) else None
            )
            side = "near" if "near" in descriptor else ("far" if "far" in descriptor else None)
            if axis and descriptor.intersection({"leg", "limb", "paw", "hoof"}):
                locomotor_parts.append(part)
                if side:
                    limb_groups.add((axis, side))
                    limb_group_counts[(axis, side)] = limb_group_counts.get((axis, side), 0) + 1
        required_groups = {("hind", "near"), ("hind", "far"), ("fore", "near"), ("fore", "far")}
        if not required_groups.issubset(limb_groups):
            missing = ", ".join("_".join(group) for group in sorted(required_groups - limb_groups))
            fail(f"quadruped locomotion is missing independently described limb groups: {missing}")
        if int(spec.get("rigVersion", 1)) >= 3:
            underspecified = [
                group for group in sorted(required_groups)
                if limb_group_counts.get(group, 0) < 3
            ]
            if underspecified:
                names = ", ".join("_".join(group) for group in underspecified)
                fail(
                    f"four_leg_sprite_rig requires upper/lower/end anatomy for: {names}; "
                    "a weighted silhouette cannot replace real joints"
                )
        else:
            for part in locomotor_parts:
                child_is_limb = any(
                    candidate.get("parent") == part["name"]
                    and exact_tokens(candidate.get("role", "")).intersection({"leg", "limb", "paw", "hoof"})
                    for candidate in parts
                )
                if not part.get("mesh") and not child_is_limb:
                    fail(
                        f"quadruped limb {part['name']!r} is one rigid cutout; split it into an "
                        "articulated chain or give it a weighted mesh"
                    )
        motion_tokens = exact_tokens(spec.get("proposal", {}).get("motionIntent", ""))
        if motion_tokens.intersection({"run", "running", "gallop", "sprint", "bound", "bounding"}):
            suspension_count = sum(not frame.get("contacts") for frame in frames)
            if suspension_count < 2:
                fail(
                    "a quadruped run or gallop requires both extended and gathered "
                    "suspension phases"
                )
            contact_axes = set()
            for frame in frames:
                for contact in frame.get("contacts", []):
                    descriptor = exact_tokens(
                        f"{contact['part']}_{part_by_name[contact['part']].get('role', '')}"
                    )
                    if descriptor.intersection({"hind", "rear"}):
                        contact_axes.add("hind")
                    if descriptor.intersection({"fore", "front"}):
                        contact_axes.add("fore")
            if contact_axes != {"hind", "fore"}:
                fail("a quadruped run or gallop must show both hind-drive and fore-impact contacts")

            upper_by_group = {}
            paw_parts = []
            for part in locomotor_parts:
                descriptor = exact_tokens(f"{part['name']}_{part.get('role', '')}")
                axis = "hind" if descriptor.intersection({"hind", "rear"}) else (
                    "fore" if descriptor.intersection({"fore", "front"}) else None
                )
                side = "near" if "near" in descriptor else ("far" if "far" in descriptor else None)
                if axis and side and descriptor.intersection({"upper", "shoulder", "thigh", "haunch"}):
                    upper_by_group[(axis, side)] = part
                if descriptor.intersection({"paw", "hoof", "foot"}):
                    paw_parts.append(part)
            shallow_groups = [
                (group, part_rotation_span(part, frames) if part else 0.0)
                for group in sorted(required_groups)
                for part in [upper_by_group.get(group)]
                if part is None or part_rotation_span(part, frames) < 22.0
            ]
            if shallow_groups:
                detail = ", ".join(
                    f"{'_'.join(group)}={span:.1f}°" for group, span in shallow_groups
                )
                fail(
                    "quadruped run limb strokes are too shallow to read as running "
                    f"({detail}); each fore/hind upper limb needs at least a 22° cycle span"
                )

            pose_tokens = [exact_tokens(frame.get("pose", "")) for frame in frames]
            extended_indices = [
                index for index, tokens in enumerate(pose_tokens)
                if "extended" in tokens and tokens.intersection({"flight", "suspension"})
            ]
            gathered_indices = [
                index for index, tokens in enumerate(pose_tokens)
                if "gathered" in tokens and tokens.intersection({"flight", "suspension", "recovery"})
            ]
            if not extended_indices or not gathered_indices:
                fail("a quadruped run requires distinct extended_flight and gathered_flight poses")
            if len(paw_parts) >= 4:
                def paw_spread(frame_index):
                    matrices = matrices_by_frame[frame_index]
                    xs = [
                        apply_matrix(matrices[part["name"]], endpoint_anchor(part))[0]
                        for part in paw_parts
                    ]
                    return max(xs) - min(xs)

                extended_spread = max(paw_spread(index) for index in extended_indices)
                gathered_spread = min(paw_spread(index) for index in gathered_indices)
                bounds = visible_bounds(source, width, height)
                source_width = bounds[2] - bounds[0] + 1 if bounds else width
                required_difference = max(3.0, source_width * 0.08)
                if extended_spread - gathered_spread < required_difference:
                    fail(
                        "quadruped flight poses keep the same standing silhouette: "
                        f"extended paw spread {extended_spread:.2f}px versus gathered "
                        f"{gathered_spread:.2f}px; increase extension/compression by at least "
                        f"{required_difference:.2f}px"
                    )

def validate_winged_attachments(parts, frames, part_by_name, morphology):
    if morphology == "winged":
        wing_roots = []
        for part in parts:
            descriptor = exact_tokens(f"{part['name']}_{part.get('role', '')}")
            if "wing" not in descriptor or not descriptor.intersection({"root", "upper", "shoulder"}):
                continue
            wing_roots.append(part)
            parent_name = part.get("parent")
            if not parent_name:
                fail(f"wing root {part['name']!r} must attach to a torso, chest, or body part")
            parent = part_by_name[parent_name]
            parent_descriptor = exact_tokens(f"{parent['name']}_{parent.get('role', '')}")
            if "wing" in parent_descriptor or not parent_descriptor.intersection(
                {"body", "torso", "chest", "spine", "shoulder"}
            ):
                fail(
                    f"wing root {part['name']!r} is parented to {parent_name!r}; "
                    "each wing root must attach independently to the torso/chest, never to the other wing"
                )
            rotations = [
                float(frame.get("transforms", {}).get(part["name"], {}).get("rotate", 0))
                for frame in frames
            ]
            rotation_span = max(rotations) - min(rotations)
            if rotation_span > 30.0 + 1e-6:
                fail(
                    f"wing root {part['name']!r} rotates across {rotation_span:.2f} degrees; "
                    "keep the root seated within a 30-degree stroke and put additional folding in the membrane"
                )
            translations = [
                math.hypot(
                    float(frame.get("transforms", {}).get(part["name"], {}).get("dx", 0)),
                    float(frame.get("transforms", {}).get(part["name"], {}).get("dy", 0)),
                )
                for frame in frames
            ]
            if max(translations, default=0.0) > 1.0 + 1e-6:
                fail(
                    f"wing root {part['name']!r} translates away from its shoulder; "
                    "animate wing rotation and membrane folding around the fixed attachment instead"
                )
        if len(wing_roots) < 2:
            fail("winged_sprite_rig requires two independently attached wing roots")

def validate_biped_walk_limits(frames, parts, is_grounded_walk, morphology):
    if is_grounded_walk and morphology == "biped":
        for frame_index, frame in enumerate(frames):
            for part in parts:
                role_tokens = exact_tokens(part.get("role", ""))
                transform = frame.get("transforms", {}).get(part["name"], {})
                if "leg" in role_tokens and "upper" in role_tokens:
                    limit = 60.0
                    value = abs(float(transform.get("rotate", 0)))
                elif "leg" in role_tokens and "lower" in role_tokens:
                    limit = 75.0
                    value = abs(float(transform.get("rotate", 0)))
                elif "foot" in role_tokens:
                    limit = 30.0
                    value = abs(float(transform.get("worldRotate", transform.get("rotate", 0))))
                else:
                    continue
                if value > limit + 1e-6:
                    fail(
                        f"frame {frame_index + 1} {part['name']!r} bends {value:.2f} degrees; "
                        f"the biped walk limit for role {part['role']!r} is {limit:.2f} degrees"
                    )

def validate_planted_contacts(
    frames, parts, matrices_by_frame, part_by_name, is_grounded_walk, morphology,
    looping, width, height,
):
    contacts_by_anchor = {}
    support_sides = []
    for frame_index, (frame, matrices) in enumerate(zip(frames, matrices_by_frame)):
        contacts = frame.get("contacts", [])
        if is_grounded_walk and not contacts:
            fail(f"grounded locomotion frame {frame_index + 1} has no planted support contact")
        sides = set()
        for contact in contacts:
            part = part_by_name[contact["part"]]
            if is_grounded_walk and morphology == "biped":
                descriptor = exact_tokens(f"{part['name']}_{part.get('role', '')}")
                anchor_descriptor = exact_tokens(contact["anchor"])
                is_lower_leg = "lower" in descriptor and "leg" in descriptor
                if not (is_lower_leg or descriptor.intersection({"foot", "shin", "paw", "hoof"})):
                    fail(
                        f"biped contact {part['name']}.{contact['anchor']} is not on a "
                        "lower-leg or foot support part"
                    )
                if not anchor_descriptor.intersection({"foot", "sole", "toe", "paw", "hoof", "ground"}):
                    fail(
                        f"biped contact {part['name']}.{contact['anchor']} is not a "
                        "ground-contact anchor such as sole, toe, or foot"
                    )
            anchor = part["anchors"][contact["anchor"]]
            world = apply_matrix(matrices[part["name"]], anchor)
            key = (part["name"], contact["anchor"])
            contacts_by_anchor.setdefault(key, []).append((frame_index, world))
            sides.add(part_side(part))
        support_sides.append(sides)
    contact_limit = max(1.0, min(width, height) / 64.0)
    for (part_name, anchor_name), samples in contacts_by_anchor.items():
        runs = []
        for sample in sorted(samples):
            if not runs or sample[0] != runs[-1][-1][0] + 1:
                runs.append([sample])
            else:
                runs[-1].append(sample)
        if looping and len(runs) > 1 and runs[0][0][0] == 0 and runs[-1][-1][0] == len(frames) - 1:
            runs[0] = runs[-1] + runs[0]
            runs.pop()
        for run in runs:
            positions = [sample[1] for sample in run]
            span = max(
                (point_distance(first, second) for first in positions for second in positions),
                default=0.0,
            )
            if span > contact_limit + 1e-6:
                frames_text = ", ".join(str(sample[0] + 1) for sample in run)
                fail(
                    f"planted contact {part_name}.{anchor_name} slides {span:.2f}px "
                    f"across frames {frames_text}; the limit is {contact_limit:.2f}px"
                )
    if is_grounded_walk and morphology == "biped":
        observed = set().union(*support_sides) if support_sides else set()
        if not {"left", "right"}.issubset(observed):
            fail("a biped walk must plant both left and right support groups during the cycle")
        exchanges = 0
        pairs = list(zip(support_sides, support_sides[1:]))
        if looping and support_sides:
            pairs.append((support_sides[-1], support_sides[0]))
        for current, following in pairs:
            if current != following and (
                ("left" in current and "right" in following)
                or ("right" in current and "left" in following)
            ):
                exchanges += 1
        minimum_exchanges = 2 if looping else 1
        if exchanges < minimum_exchanges:
            fail(
                "a biped walk must alternate support "
                f"at least {minimum_exchanges} time(s) across the sequence"
            )
    return support_sides

def validate_biped_run(
    spec, frames, parts, matrices_by_frame, support_sides, is_locomotion, morphology,
    width, height, source,
):
    motion_tokens = exact_tokens(spec.get("proposal", {}).get("motionIntent", ""))
    if is_locomotion and morphology == "biped" and motion_tokens.intersection(
        {"run", "running", "sprint"}
    ):
        if sum(not frame.get("contacts") for frame in frames) < 2:
            fail("a biped run requires two flight phases, one after each leg drives")
        observed = set().union(*support_sides) if support_sides else set()
        if not {"left", "right"}.issubset(observed):
            fail("a biped run must include distinct left and right contact frames")
        foot_by_side = {}
        for part in parts:
            descriptor = exact_tokens(f"{part['name']}_{part.get('role', '')}")
            side = "left" if "left" in descriptor else ("right" if "right" in descriptor else None)
            if side and descriptor.intersection({"foot", "paw", "hoof"}):
                foot_by_side[side] = part
        if set(foot_by_side) != {"left", "right"}:
            fail("a biped run requires independently anchored left and right feet")
        bounds = visible_bounds(source, width, height)
        source_height = bounds[3] - bounds[1] + 1 if bounds else height
        minimum_split = max(4.0, source_height * 0.20)
        wide_contact_sides = set()
        wide_contact_indices = []
        for index, matrices in enumerate(matrices_by_frame):
            frame_sides = support_sides[index]
            if not frame_sides:
                continue
            left = apply_matrix(
                matrices[foot_by_side["left"]["name"]], endpoint_anchor(foot_by_side["left"]),
            )
            right = apply_matrix(
                matrices[foot_by_side["right"]["name"]], endpoint_anchor(foot_by_side["right"]),
            )
            split = abs(left[0] - right[0])
            if split >= minimum_split:
                wide_contact_sides.update(frame_sides.intersection({"left", "right"}))
                wide_contact_indices.append(index)
        if wide_contact_sides != {"left", "right"}:
            fail(
                "biped run contact poses are visually near-static: require a wide split stance "
                f"of at least {minimum_split:.2f}px for both left and right contacts"
            )
        opposite_extremes = any(
            min((second - first) % len(frames), (first - second) % len(frames))
            >= max(2, len(frames) // 3)
            for first in wide_contact_indices
            for second in wide_contact_indices
            if first != second
        )
        if not opposite_extremes:
            fail("biped run needs two wide split contact extremes spaced across the cycle")

