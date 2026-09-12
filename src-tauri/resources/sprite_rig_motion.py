"""Articulated motion energy, uniqueness, and v2 mechanical quality gates."""

import hashlib
import math
import statistics

from sprite_rig_gait import (
    validate_biped_run,
    validate_biped_walk_limits,
    validate_planted_contacts,
    validate_quadruped_locomotion,
    validate_winged_attachments,
)
from sprite_rig_json import exact_tokens, fail, point_distance
from sprite_rig_matrix import apply_matrix
from sprite_rig_profile import locomotion_context
from sprite_rig_raster import (
    alpha_component_sizes,
    frame_clipped_pixels,
    mesh_clipped_pixels,
    nearest_visible_distance,
    visible_pixel_count,
)

def transform_has_motion(transform):
    """Return whether a part transform changes its source pose."""
    return any(
        abs(float(transform.get(key, default)) - default) > 1e-6
        for key, default in (
            ("dx", 0), ("dy", 0), ("rotate", 0), ("worldRotate", 0),
            ("scaleX", 1), ("scaleY", 1),
        )
    )


def meaningfully_animated_parts(frames):
    """Return parts whose authored pose spans enough to survive pixel quantization."""
    names = {
        name
        for frame in frames
        for name in frame.get("transforms", {})
        if name != "base"
    }
    meaningful = set()
    for name in names:
        samples = [frame.get("transforms", {}).get(name, {}) for frame in frames]
        dx = [float(sample.get("dx", 0)) for sample in samples]
        dy = [float(sample.get("dy", 0)) for sample in samples]
        rotation = [float(sample.get("worldRotate", sample.get("rotate", 0))) for sample in samples]
        scale_x = [float(sample.get("scaleX", 1)) for sample in samples]
        scale_y = [float(sample.get("scaleY", 1)) for sample in samples]
        translation_span = math.hypot(max(dx) - min(dx), max(dy) - min(dy))
        rotation_span = max(rotation) - min(rotation)
        scale_span = max(max(scale_x) - min(scale_x), max(scale_y) - min(scale_y))
        if translation_span >= 1.5 or rotation_span >= 8.0 or scale_span >= 0.05:
            meaningful.add(name)
    return meaningful


def frame_signature(parts, matrices):
    signature = []
    for part in parts:
        matrix = matrices[part["name"]]
        points = [part.get("pivot", [0, 0]), *part.get("anchors", {}).values()]
        signature.extend(apply_matrix(matrix, point) for point in points)
    return signature


def signature_energy(first, second):
    if len(first) != len(second) or not first:
        return 0.0
    return sum(point_distance(a, b) for a, b in zip(first, second)) / len(first)


def validate_articulated_motion(spec, names, frames, parts=None, matrices_by_frame=None):
    """Reject locomotion rigs that merely translate one rigid sprite."""
    morphology, motion_text, is_locomotion, _ = locomotion_context(spec)
    if not is_locomotion:
        return

    moved_parts = {
        name
        for frame in frames
        for name, transform in frame.get("transforms", {}).items()
        if name != "base" and transform_has_motion(transform)
    }
    if len(moved_parts) < 2:
        fail(
            "articulated locomotion cannot be root-only: animate at least two anatomical "
            "parts independently instead of lifting or sliding the complete sprite"
        )
    meaningful_parts = meaningfully_animated_parts(frames)
    if len(meaningful_parts) < 2:
        fail(
            "locomotion transforms are too small to survive sprite-pixel quantization: "
            "animate at least two anatomical parts across 8 degrees, 1.5 pixels, or 5% scale "
            "instead of publishing a nearly static root bob"
        )

    if int(spec.get("rigVersion", 1)) < 2:
        return
    if morphology not in {
        "biped", "quadruped", "hexapod", "segmented-many-leg", "serpentine", "winged"
    }:
        fail("rigVersion 2 locomotion requires a supported proposal.morphologyTag")
    if not parts or not matrices_by_frame:
        fail("rigVersion 2 locomotion could not resolve its articulated skeleton")
    descriptors = {
        part["name"]: f"{part['name']} {part.get('role', '')}".replace("-", "_").lower()
        for part in parts
    }
    core_tokens = {
        "biped": {"body", "torso", "pelvis", "spine", "chest", "hip"},
        "quadruped": {"body", "torso", "pelvis", "spine", "chest"},
        "hexapod": {"body", "thorax", "abdomen", "head"},
        "segmented-many-leg": {"body", "segment", "segments", "head", "tail"},
        "serpentine": {"body", "segment", "segments", "head", "tail", "spine"},
        "winged": {"body", "torso", "chest", "spine"},
    }
    required_core_parts = 2 if morphology in {"segmented-many-leg", "serpentine"} else 1
    core_parts = {
        name for name, descriptor in descriptors.items()
        if exact_tokens(descriptor).intersection(core_tokens.get(morphology, set()))
    }
    animated_core = meaningful_parts.intersection(core_parts)
    if core_parts and len(animated_core) < required_core_parts:
        fail(
            f"{morphology} locomotion ignores the body: animate at least "
            f"{required_core_parts} core body part{'s' if required_core_parts != 1 else ''} "
            "with visible compression, rotation, spine wave, or counter-motion; moving only "
            "limbs under a rigid body is not a finished rig"
        )
    if morphology == "biped":
        left_leg_parts = {
            name for name, descriptor in descriptors.items()
            if "left" in exact_tokens(descriptor)
            and exact_tokens(descriptor).intersection({"leg", "foot", "shin", "thigh"})
        }
        right_leg_parts = {
            name for name, descriptor in descriptors.items()
            if "right" in exact_tokens(descriptor)
            and exact_tokens(descriptor).intersection({"leg", "foot", "shin", "thigh"})
        }
        if len(left_leg_parts) < 2 or len(right_leg_parts) < 2:
            fail(
                "a rigVersion 2 biped walk requires articulated left and right leg chains "
                "with at least two parts per side"
            )
        moving_world_parts = set()
        for part in parts:
            points = [part.get("pivot", [0, 0]), *part.get("anchors", {}).values()]
            positions = [
                tuple(apply_matrix(matrices[part["name"]], point) for point in points)
                for matrices in matrices_by_frame
            ]
            if any(positions[index] != positions[0] for index in range(1, len(positions))):
                moving_world_parts.add(part["name"])
        if not (moving_world_parts & left_leg_parts) or not (moving_world_parts & right_leg_parts):
            fail("a biped locomotion cycle must visibly articulate both left and right leg chains")

    motion_tokens = exact_tokens(motion_text.replace(" ", "_"))
    if morphology == "quadruped" and motion_tokens.intersection(
        {"hop", "hopping", "bound", "bounding", "pounce"}
    ):
        normalized = {name.replace("-", "_").lower() for name in names}
        has_hind = any(any(token in name for token in ("hind", "rear", "haunch", "thigh")) for name in normalized)
        has_fore = any(any(token in name for token in ("fore", "front", "shoulder")) for name in normalized)
        if not has_hind or not has_fore or len(moved_parts) < 3:
            fail(
                "a quadruped hop requires independently animated hindlimb and forelimb masks "
                "plus at least one additional articulated body part; whole-body lift is not a hop"
            )


def validate_root_motion_limits(spec, frames, width, height, is_locomotion):
    looping = spec.get("looping", True)
    if not isinstance(looping, bool):
        fail("looping must be true or false")
    root_motion = str(spec.get("rootMotion", "")).strip().lower()
    if root_motion not in {"in-place", "baked"}:
        fail("rigVersion 2 requires rootMotion to be 'in-place' or 'baked'")
    if is_locomotion and not all(str(frame.get("phase", "")).strip() for frame in frames):
        fail("rigVersion 2 locomotion requires a non-empty phase on every frame")
    phases = [str(frame.get("phase", "")).strip().lower() for frame in frames]
    if is_locomotion and len(set(phases)) != len(phases):
        fail("locomotion phase names must be unique within one cycle")

    roots = [frame.get("root", {}) for frame in frames]
    root_x = [float(root.get("dx", 0)) for root in roots]
    root_y = [float(root.get("dy", 0)) for root in roots]
    if is_locomotion and root_motion == "in-place":
        root_amplitude_limit = max(1.0, min(width, height) / 64.0)
        root_span_limit = root_amplitude_limit * 2
        if max(root_x) - min(root_x) > root_span_limit + 1e-6:
            fail(
                f"in-place root drift spans {max(root_x) - min(root_x):.2f}px; "
                f"the limit is {root_span_limit:.2f}px"
            )
        if looping and abs(root_x[-1] - root_x[0]) > root_amplitude_limit + 1e-6:
            fail("in-place root motion snaps horizontally across the loop seam")
    vertical_limit = max(2.0, min(width, height) / 24.0)
    if is_locomotion and max(root_y) - min(root_y) > vertical_limit + 1e-6:
        fail("root bob is too large for a stable locomotion cycle")
    return looping, root_motion, root_x, root_y

def validate_anchor_and_joint_gaps(
    parts, source, width, height, matrices_by_frame, part_by_name,
):
    for part in parts:
        for anchor_name, anchor in part.get("anchors", {}).items():
            distance = nearest_visible_distance(source, width, height, part["mask"], anchor)
            if distance > 2.25:
                fail(
                    f"part {part['name']!r} anchor {anchor_name!r} is {distance:.2f}px "
                    "from its visible pixels"
                )
        parent_name = part.get("parent")
        if not parent_name:
            continue
        parent = part_by_name[parent_name]
        attach = part["attach"]
        parent_anchor = parent["anchors"][attach["parentAnchor"]]
        self_anchor = part["anchors"][attach["selfAnchor"]]
        bind_gap = point_distance(parent_anchor, self_anchor)
        if bind_gap > 1.0:
            fail(
                f"joint {parent_name}->{part['name']} bind anchors are {bind_gap:.2f}px apart; "
                "attachment anchors must coincide"
            )
        for frame_index, matrices in enumerate(matrices_by_frame):
            parent_world = apply_matrix(matrices[parent_name], parent_anchor)
            child_world = apply_matrix(matrices[part["name"]], self_anchor)
            gap = point_distance(parent_world, child_world)
            if gap > 1.25:
                fail(
                    f"joint {parent_name}->{part['name']} separates by {gap:.2f}px "
                    f"in frame {frame_index + 1}"
                )

def validate_canvas_clipping(matrices_by_frame, base, layers, parts, width, height):
    clipped_by_name = []
    layer_by_name = {layer["name"]: layer for layer in layers}
    for frame_index, matrices in enumerate(matrices_by_frame):
        clipped = frame_clipped_pixels(base, width, height, matrices["base"])
        for part in parts:
            layer = layer_by_name[part["name"]]
            if layer.get("mesh"):
                clipped += mesh_clipped_pixels(
                    layer["pixels"], width, height, layer["mesh"], matrices,
                )
            else:
                clipped += frame_clipped_pixels(
                    layer["pixels"], width, height, matrices[part["name"]],
                )
        owned_visible_pixels = visible_pixel_count(base) + sum(
            visible_pixel_count(layer["pixels"]) for layer in layers
        )
        clipping_limit = max(1, int(math.floor(owned_visible_pixels * 0.01)))
        if clipped > clipping_limit:
            clipped_by_name.append((frame_index + 1, clipped))
    if clipped_by_name:
        frame_index, clipped = clipped_by_name[0]
        fail(
            f"frame {frame_index} clips {clipped} transformed source pixels at the canvas edge; "
            f"the tolerance is {clipping_limit}"
        )

def validate_alpha_fragments(is_locomotion, source, width, height, canvases):
    if is_locomotion:
        source_components = alpha_component_sizes(source, width, height)
        source_component_count = len(source_components)
        for frame_index, canvas in enumerate(canvases):
            rendered_components = alpha_component_sizes(canvas.data, width, height)
            if len(rendered_components) > source_component_count:
                extras = rendered_components[source_component_count:]
                fail(
                    f"frame {frame_index + 1} creates detached alpha fragments "
                    f"with sizes {extras}; moving masks left source pixels behind or a joint opened"
                )

def validate_unique_rendered_frames(canvases, looping, frames):
    rendered_hashes = [hashlib.sha256(bytes(canvas.data)).hexdigest() for canvas in canvases]
    if looping and rendered_hashes[-1] == rendered_hashes[0]:
        fail("duplicate loop endpoint: the final rendered frame repeats the first frame")
    for index in range(1, len(rendered_hashes)):
        if rendered_hashes[index] == rendered_hashes[index - 1] and not frames[index].get("hold", False):
            fail(
                f"rendered frames {index} and {index + 1} are identical; "
                "mark an intentional internal hold or create a distinct pose"
            )
    first_seen_hash = {}
    for index, rendered_hash in enumerate(rendered_hashes):
        previous = first_seen_hash.get(rendered_hash)
        if previous is not None:
            is_adjacent_hold = index == previous + 1 and frames[index].get("hold", False)
            if not is_adjacent_hold:
                fail(
                    f"rendered frame {index + 1} repeats non-adjacent frame {previous + 1}; "
                    "every repeated pose must be an adjacent intentional hold"
                )
        else:
            first_seen_hash[rendered_hash] = index

def validate_locomotion_cadence(
    is_locomotion, parts, matrices_by_frame, frames, looping, width, height,
    root_motion, root_x,
):
    if not is_locomotion:
        return

    signatures = [frame_signature(parts, matrices) for matrices in matrices_by_frame]
    ordinary = []
    analyzed_ordinary = []
    hold_count = 0
    for index in range(1, len(signatures)):
        energy = signature_energy(signatures[index - 1], signatures[index])
        ordinary.append(energy)
        if frames[index].get("hold", False):
            hold_count += 1
        elif energy > 1e-6:
            analyzed_ordinary.append(energy)
    if hold_count > max(2, len(frames) // 4):
        fail("locomotion contains too many held poses for one readable cycle")
    if not analyzed_ordinary:
        fail("locomotion has no measurable articulated transition energy")
    median_step = statistics.median(analyzed_ordinary)
    scale_unit = max(1.0, min(width, height) / 64.0)
    spike_limit = max(3.5 * scale_unit, median_step * 2.75)
    for index, energy in enumerate(ordinary, start=1):
        if frames[index].get("hold", False):
            continue
        if energy < median_step * 0.25:
            fail(
                f"locomotion nearly pauses between frames {index} and {index + 1}: "
                f"{energy:.2f}px is below 25% of the ordinary transition cadence"
            )
        if energy > spike_limit:
            fail(
                f"limb motion pops between frames {index} and {index + 1}: "
                f"{energy:.2f}px exceeds the {spike_limit:.2f}px transition limit"
            )
    if looping:
        seam_energy = signature_energy(signatures[-1], signatures[0])
        seam_limit = max(2.0 * scale_unit, median_step * 1.75)
        if seam_energy > seam_limit:
            fail(
                f"loop discontinuity is {seam_energy:.2f}px at the final-to-first seam; "
                f"the limit is {seam_limit:.2f}px"
            )
        if is_locomotion and seam_energy < median_step * 0.25:
            fail("loop seam is a near-duplicate pose that will create a visible playback pause")
    elif root_motion == "baked" and is_locomotion:
        deltas = [root_x[index] - root_x[index - 1] for index in range(1, len(root_x))]
        nonzero_signs = {1 if delta > 0 else -1 for delta in deltas if abs(delta) > 1e-6}
        if len(nonzero_signs) > 1:
            fail("non-looping baked root travel must progress monotonically")


def validate_v2_motion_quality(
    spec, width, height, source, parts, frames, base, layers, matrices_by_frame, canvases,
):
    """Hard-gate mechanical failures that whole-frame image metrics cannot see."""
    morphology, _, is_locomotion, is_grounded_walk = locomotion_context(spec)
    looping, root_motion, root_x, _root_y = validate_root_motion_limits(
        spec, frames, width, height, is_locomotion,
    )
    part_by_name = {part["name"]: part for part in parts}
    validate_quadruped_locomotion(
        spec, parts, frames, source, width, height, matrices_by_frame, part_by_name,
        morphology, is_locomotion,
    )
    validate_winged_attachments(parts, frames, part_by_name, morphology)
    validate_anchor_and_joint_gaps(
        parts, source, width, height, matrices_by_frame, part_by_name,
    )
    validate_biped_walk_limits(frames, parts, is_grounded_walk, morphology)
    support_sides = validate_planted_contacts(
        frames, parts, matrices_by_frame, part_by_name, is_grounded_walk, morphology,
        looping, width, height,
    )
    validate_biped_run(
        spec, frames, parts, matrices_by_frame, support_sides, is_locomotion, morphology,
        width, height, source,
    )
    validate_canvas_clipping(matrices_by_frame, base, layers, parts, width, height)
    validate_alpha_fragments(is_locomotion, source, width, height, canvases)
    validate_unique_rendered_frames(canvases, looping, frames)
    validate_locomotion_cadence(
        is_locomotion, parts, matrices_by_frame, frames, looping, width, height,
        root_motion, root_x,
    )
