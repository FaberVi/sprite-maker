"""Shared JSON parsing, numeric checks, and fail-fast helpers for the rig engine."""

import json
import math


def fail(message):
    raise SystemExit(f"sprite_rig: {message}")


def reject_json_constant(value):
    raise ValueError(f"non-finite JSON constant is not allowed: {value}")


def strict_json_loads(value):
    return json.loads(value, parse_constant=reject_json_constant)


def strict_json_dumps(value, **kwargs):
    return json.dumps(value, allow_nan=False, **kwargs)


def finite_number(value, label):
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(value):
        fail(f"{label} must be a finite number")
    return float(value)


def exact_tokens(value):
    return {token for token in str(value).replace("-", "_").lower().split("_") if token}


def point_distance(first, second):
    return math.hypot(first[0] - second[0], first[1] - second[1])
