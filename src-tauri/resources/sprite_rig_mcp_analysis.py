"""Motion-analysis payload derived from a validated sprite_rig engine report."""

import math


def analyze_motion(report):
    """Return solved gait mechanics from a successful --check engine report."""
    if not isinstance(report.get("rigVersion"), int) or report["rigVersion"] < 2:
        raise ValueError("motion analysis requires a rigVersion 2 or newer articulated rig")
    analysis = report.get("analysis")
    if not isinstance(analysis, dict):
        raise ValueError("validated rig did not return solved motion analysis")
    energies = analysis.get("transitionEnergyPx")
    contacts = analysis.get("plantedContacts")
    phases = analysis.get("phases")
    poses = analysis.get("poses", [])
    joints = analysis.get("joints", [])
    if not isinstance(energies, list) or not energies:
        raise ValueError("validated rig has no solved transition-energy series")
    if not isinstance(contacts, list) or not contacts:
        raise ValueError("validated rig has no solved planted contacts")
    if not isinstance(phases, list) or not all(isinstance(value, str) and value for value in phases):
        raise ValueError("validated rig has incomplete gait phases")
    if not isinstance(poses, list) or not all(isinstance(value, str) for value in poses):
        raise ValueError("validated rig returned invalid named poses")
    if not isinstance(joints, list) or not all(isinstance(value, dict) for value in joints):
        raise ValueError("validated rig returned an invalid observed-joint map")
    master_hash = report.get("masterHash")
    rig_hash = report.get("rigHash")
    frame_hashes = report.get("quality", {}).get("renderedFrameHashes")
    if not all(
        isinstance(value, str)
        and len(value) == 64
        and all(character in "0123456789abcdef" for character in value)
        for value in (master_hash, rig_hash)
    ):
        raise ValueError("validated rig did not return source and rig identity hashes")
    if (
        not isinstance(frame_hashes, list)
        or len(frame_hashes) != report.get("frames")
        or not all(
            isinstance(value, str)
            and len(value) == 64
            and all(character in "0123456789abcdef" for character in value)
            for value in frame_hashes
        )
    ):
        raise ValueError("validated rig did not return one raster hash per frame")
    numeric = [float(value) for value in energies]
    if not all(math.isfinite(value) and value >= 0 for value in numeric):
        raise ValueError("validated rig returned invalid transition energies")
    positive = [value for value in numeric if value > 1e-9]
    cadence_ratio = max(positive) / min(positive) if positive else None
    contact_groups = {}
    for contact in contacts:
        if not isinstance(contact, dict):
            raise ValueError("validated rig returned an invalid planted contact")
        key = f"{contact.get('part')}.{contact.get('anchor')}"
        contact_groups.setdefault(key, []).append({
            "frame": contact.get("frame"),
            "x": contact.get("x"),
            "y": contact.get("y"),
        })
    return {
        "valid": report.get("valid") is True,
        "name": report.get("name"),
        "masterHash": master_hash,
        "rigHash": rig_hash,
        "rigVersion": report.get("rigVersion"),
        "frameCount": report.get("frames"),
        "uniqueRenderedFrames": report.get("quality", {}).get("uniqueRenderedFrames"),
        "renderedFrameHashes": frame_hashes,
        "looping": analysis.get("looping"),
        "rootMotion": analysis.get("rootMotion"),
        "rigProfile": analysis.get("rigProfile"),
        "joints": joints,
        "visibleJointCount": sum(
            1 for joint in joints if joint.get("visibility") == "visible"
        ),
        "phases": phases,
        "poses": poses,
        "transitionEnergyPx": numeric,
        "cadenceRatio": round(cadence_ratio, 6) if cadence_ratio is not None else None,
        "plantedContactGroups": contact_groups,
        "warnings": report.get("warnings", []),
    }
