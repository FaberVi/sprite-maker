"""Rig JSON field schemas: palette, mesh, and draw commands."""

from sprite_rig_json import fail, finite_number
from sprite_rig_matrix import barycentric_coordinates
from sprite_tool import color, slug

def validate_color_value(value, label):
    if value is not None and not isinstance(value, str):
        fail(f"{label} must be a hexadecimal color or 'transparent'")
    try:
        color(value)
    except SystemExit:
        fail(f"{label} must be a hexadecimal color or 'transparent'")


def validate_palette(value):
    if not isinstance(value, dict):
        fail("palette must be an object")
    for key, entry in value.items():
        if not isinstance(key, str) or not key:
            fail("palette keys must be non-empty strings")
        validate_color_value(entry, f"palette color {key!r}")
    return value



def validate_mesh(mesh, width, height, part_names, label):
    """Validate a compact weighted deformation mesh for one pixel layer."""
    if not isinstance(mesh, dict):
        fail(f"{label} mesh must be an object")
    unexpected = set(mesh) - {"vertices", "triangles", "weights"}
    if unexpected:
        fail(f"{label} mesh has unsupported keys: {', '.join(sorted(unexpected))}")
    vertices = mesh.get("vertices")
    triangles = mesh.get("triangles")
    weights = mesh.get("weights")
    if not isinstance(vertices, list) or not (3 <= len(vertices) <= 128):
        fail(f"{label} mesh requires between 3 and 128 vertices")
    normalized_vertices = []
    for index, vertex in enumerate(vertices):
        if not isinstance(vertex, list) or len(vertex) != 2:
            fail(f"{label} mesh vertex {index + 1} must be [x,y]")
        x = finite_number(vertex[0], f"{label} mesh vertex {index + 1} x")
        y = finite_number(vertex[1], f"{label} mesh vertex {index + 1} y")
        if not (0 <= x <= width and 0 <= y <= height):
            fail(f"{label} mesh vertex {index + 1} lies outside the locked master")
        normalized_vertices.append([x, y])
    if not isinstance(triangles, list) or not triangles:
        fail(f"{label} mesh requires at least one triangle")
    normalized_triangles = []
    for index, triangle in enumerate(triangles):
        if (
            not isinstance(triangle, list)
            or len(triangle) != 3
            or any(isinstance(value, bool) or not isinstance(value, int) for value in triangle)
            or len(set(triangle)) != 3
            or any(value < 0 or value >= len(vertices) for value in triangle)
        ):
            fail(f"{label} mesh triangle {index + 1} requires three distinct vertex indexes")
        points = [normalized_vertices[value] for value in triangle]
        if barycentric_coordinates(points[0], *points) is None:
            fail(f"{label} mesh triangle {index + 1} is degenerate")
        normalized_triangles.append(triangle)
    if not isinstance(weights, list) or len(weights) != len(vertices):
        fail(f"{label} mesh requires one weight list per vertex")
    normalized_weights = []
    for vertex_index, influences in enumerate(weights):
        if not isinstance(influences, list) or not (1 <= len(influences) <= 4):
            fail(f"{label} mesh vertex {vertex_index + 1} requires 1 to 4 bone weights")
        seen = set()
        normalized_influences = []
        total = 0.0
        for influence_index, influence in enumerate(influences):
            influence_label = (
                f"{label} mesh vertex {vertex_index + 1} influence {influence_index + 1}"
            )
            if not isinstance(influence, dict) or set(influence) != {"bone", "weight"}:
                fail(f"{influence_label} requires only bone and weight")
            bone = slug(influence.get("bone", ""))
            if bone not in part_names:
                fail(f"{influence_label} refers to unknown bone {bone!r}")
            if bone in seen:
                fail(f"{label} mesh vertex {vertex_index + 1} repeats bone {bone!r}")
            seen.add(bone)
            weight = finite_number(influence.get("weight"), f"{influence_label} weight")
            if weight <= 0:
                fail(f"{influence_label} weight must be positive")
            total += weight
            normalized_influences.append({"bone": bone, "weight": weight})
        if abs(total - 1.0) > 1e-4:
            fail(f"{label} mesh vertex {vertex_index + 1} weights must sum to 1")
        normalized_weights.append(normalized_influences)
    mesh["vertices"] = normalized_vertices
    mesh["triangles"] = normalized_triangles
    mesh["weights"] = normalized_weights


def mesh_contains_point(mesh, point):
    for triangle in mesh["triangles"]:
        points = [mesh["vertices"][index] for index in triangle]
        weights = barycentric_coordinates(point, *points)
        if weights is not None and min(weights) >= -1e-6 and max(weights) <= 1.0 + 1e-6:
            return True
    return False



def validate_draw_commands(commands, label, palette):
    schemas = {
        "pixel": ({"type", "x", "y", "color"}, {"x", "y"}),
        "rect": ({"type", "x", "y", "w", "h", "color"}, {"x", "y", "w", "h"}),
        "line": (
            {"type", "x1", "y1", "x2", "y2", "thickness", "color"},
            {"x1", "y1", "x2", "y2"},
        ),
        "ellipse": ({"type", "x", "y", "w", "h", "color"}, {"x", "y", "w", "h"}),
        "polygon": ({"type", "points", "color"}, set()),
    }
    for index, command in enumerate(commands):
        command_label = f"{label} command {index + 1}"
        if not isinstance(command, dict):
            fail(f"{command_label} must be an object")
        kind = command.get("type")
        if kind not in schemas:
            fail(f"{command_label} has unsupported type {kind!r}")
        allowed, required_numbers = schemas[kind]
        unexpected = set(command) - allowed
        if unexpected:
            fail(f"{command_label} has unsupported keys: {', '.join(sorted(unexpected))}")
        missing = required_numbers - set(command)
        if missing:
            fail(f"{command_label} is missing: {', '.join(sorted(missing))}")
        for key in required_numbers:
            finite_number(command[key], f"{command_label}.{key}")
        if "thickness" in command:
            if finite_number(command["thickness"], f"{command_label}.thickness") <= 0:
                fail(f"{command_label}.thickness must be positive")
        if kind in {"rect", "ellipse"} and (
            float(command["w"]) <= 0 or float(command["h"]) <= 0
        ):
            fail(f"{command_label} width and height must be positive")
        if kind == "polygon":
            points = command.get("points")
            if not isinstance(points, list) or len(points) < 3:
                fail(f"{command_label}.points requires at least three [x,y] points")
            for point_index, point in enumerate(points):
                if not isinstance(point, list) or len(point) != 2:
                    fail(f"{command_label}.points[{point_index}] must be [x,y]")
                finite_number(point[0], f"{command_label}.points[{point_index}].x")
                finite_number(point[1], f"{command_label}.points[{point_index}].y")
        raw_color = command.get("color")
        if raw_color is not None and not isinstance(raw_color, str):
            fail(f"{command_label}.color must be a palette key or hexadecimal color")
        resolved_color = palette.get(str(raw_color), raw_color)
        validate_color_value(resolved_color, f"{command_label}.color")



