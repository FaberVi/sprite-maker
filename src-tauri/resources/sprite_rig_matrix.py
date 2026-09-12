"""Affine matrices, mesh deformation, and per-frame world matrix resolution."""

import math

from sprite_rig_json import fail
from sprite_tool import slug

IDENTITY_MATRIX = (1.0, 0.0, 0.0, 1.0, 0.0, 0.0)


def multiply_matrix(left, right):
    """Return the affine matrix left ∘ right."""
    la, lb, lc, ld, ltx, lty = left
    ra, rb, rc, rd, rtx, rty = right
    return (
        la * ra + lc * rb,
        lb * ra + ld * rb,
        la * rc + lc * rd,
        lb * rc + ld * rd,
        la * rtx + lc * rty + ltx,
        lb * rtx + ld * rty + lty,
    )


def translation_matrix(dx, dy):
    return (1.0, 0.0, 0.0, 1.0, float(dx), float(dy))


def local_matrix(pivot, transform):
    pivot_x, pivot_y = map(float, pivot)
    dx = float(transform.get("dx", 0))
    dy = float(transform.get("dy", 0))
    angle = math.radians(float(transform.get("rotate", 0)))
    scale_x = float(transform.get("scaleX", 1))
    scale_y = float(transform.get("scaleY", 1))
    if scale_x == 0 or scale_y == 0:
        fail("rig scale cannot be zero")
    cosine, sine = math.cos(angle), math.sin(angle)
    around_origin = (
        cosine * scale_x,
        sine * scale_x,
        -sine * scale_y,
        cosine * scale_y,
        0.0,
        0.0,
    )
    return multiply_matrix(
        translation_matrix(dx + pivot_x, dy + pivot_y),
        multiply_matrix(around_origin, translation_matrix(-pivot_x, -pivot_y)),
    )


def invert_matrix(matrix):
    a, b, c, d, tx, ty = matrix
    determinant = a * d - b * c
    if abs(determinant) < 1e-9:
        fail("rig transform is not invertible")
    return (
        d / determinant,
        -b / determinant,
        -c / determinant,
        a / determinant,
        (c * ty - d * tx) / determinant,
        (b * tx - a * ty) / determinant,
    )


def apply_matrix(matrix, point):
    a, b, c, d, tx, ty = matrix
    x, y = map(float, point)
    return (a * x + c * y + tx, b * x + d * y + ty)


def matrix_rotation(matrix):
    return math.degrees(math.atan2(matrix[1], matrix[0]))


def transformed_matrix(layer, width, height, matrix):
    """Rasterize a layer through one composed affine matrix."""
    result = bytearray(len(layer))
    inverse = invert_matrix(matrix)
    for destination_y in range(height):
        for destination_x in range(width):
            source_center = apply_matrix(inverse, (destination_x + 0.5, destination_y + 0.5))
            source_x = int(round(source_center[0] - 0.5))
            source_y = int(round(source_center[1] - 0.5))
            if 0 <= source_x < width and 0 <= source_y < height:
                source_offset = (source_y * width + source_x) * 4
                if layer[source_offset + 3]:
                    target_offset = (destination_y * width + destination_x) * 4
                    result[target_offset:target_offset + 4] = layer[source_offset:source_offset + 4]
    return result


def barycentric_coordinates(point, first, second, third):
    """Return barycentric weights for point, or None for a degenerate triangle."""
    px, py = point
    ax, ay = first
    bx, by = second
    cx, cy = third
    denominator = (by - cy) * (ax - cx) + (cx - bx) * (ay - cy)
    if abs(denominator) < 1e-9:
        return None
    first_weight = ((by - cy) * (px - cx) + (cx - bx) * (py - cy)) / denominator
    second_weight = ((cy - ay) * (px - cx) + (ax - cx) * (py - cy)) / denominator
    return first_weight, second_weight, 1.0 - first_weight - second_weight


def mesh_destination_vertices(mesh, matrices):
    destination_vertices = []
    for vertex, influences in zip(mesh["vertices"], mesh["weights"]):
        destination_x = 0.0
        destination_y = 0.0
        for influence in influences:
            transformed_vertex = apply_matrix(matrices[influence["bone"]], vertex)
            destination_x += transformed_vertex[0] * influence["weight"]
            destination_y += transformed_vertex[1] * influence["weight"]
        destination_vertices.append((destination_x, destination_y))
    return destination_vertices


def transformed_mesh(layer, width, height, mesh, matrices):
    """Deform a pixel layer through a weighted triangle mesh.

    The UVs are the bind-pose vertex positions. Destination pixels are mapped
    back into each source triangle and sampled nearest-neighbour, preserving
    the locked pixel palette instead of blurring it.
    """
    source_vertices = mesh["vertices"]
    destination_vertices = mesh_destination_vertices(mesh, matrices)

    result = bytearray(len(layer))
    for triangle in mesh["triangles"]:
        source_triangle = [source_vertices[index] for index in triangle]
        destination_triangle = [destination_vertices[index] for index in triangle]
        minimum_x = max(0, int(math.floor(min(point[0] for point in destination_triangle))))
        maximum_x = min(width - 1, int(math.ceil(max(point[0] for point in destination_triangle))))
        minimum_y = max(0, int(math.floor(min(point[1] for point in destination_triangle))))
        maximum_y = min(height - 1, int(math.ceil(max(point[1] for point in destination_triangle))))
        for destination_y in range(minimum_y, maximum_y + 1):
            for destination_x in range(minimum_x, maximum_x + 1):
                weights = barycentric_coordinates(
                    (destination_x + 0.5, destination_y + 0.5), *destination_triangle,
                )
                if weights is None or min(weights) < -1e-6 or max(weights) > 1.0 + 1e-6:
                    continue
                source_x_float = sum(
                    weight * point[0] for weight, point in zip(weights, source_triangle)
                )
                source_y_float = sum(
                    weight * point[1] for weight, point in zip(weights, source_triangle)
                )
                source_x = int(round(source_x_float - 0.5))
                source_y = int(round(source_y_float - 0.5))
                if not (0 <= source_x < width and 0 <= source_y < height):
                    continue
                source_offset = (source_y * width + source_x) * 4
                if not layer[source_offset + 3]:
                    continue
                target_offset = (destination_y * width + destination_x) * 4
                result[target_offset:target_offset + 4] = layer[source_offset:source_offset + 4]
    return result


def mesh_destination_point(mesh, destination_vertices, point):
    for triangle in mesh["triangles"]:
        source_triangle = [mesh["vertices"][index] for index in triangle]
        weights = barycentric_coordinates(point, *source_triangle)
        if weights is None or min(weights) < -1e-6 or max(weights) > 1.0 + 1e-6:
            continue
        destination_triangle = [destination_vertices[index] for index in triangle]
        return (
            sum(weight * vertex[0] for weight, vertex in zip(weights, destination_triangle)),
            sum(weight * vertex[1] for weight, vertex in zip(weights, destination_triangle)),
        )
    return None


def transformed(layer, width, height, pivot, transform, root):
    """Legacy flat transform wrapper retained for rigVersion 1 compatibility."""
    root_matrix = translation_matrix(root.get("dx", 0), root.get("dy", 0))
    return transformed_matrix(layer, width, height, multiply_matrix(root_matrix, local_matrix(pivot, transform)))



def resolve_frame_matrices(parts, frame):
    """Resolve local part transforms into root-inclusive world matrices."""
    part_by_name = {part["name"]: part for part in parts}
    transforms = frame.get("transforms", {})
    root = frame.get("root", {})
    root_matrix = translation_matrix(root.get("dx", 0), root.get("dy", 0))
    local_world = {}
    resolving = set()

    def resolve_local_world(name):
        if name in local_world:
            return local_world[name]
        if name in resolving:
            fail(f"rig part parent cycle reaches {name!r}")
        resolving.add(name)
        part = part_by_name[name]
        parent = part.get("parent")
        parent_matrix = resolve_local_world(parent) if parent else IDENTITY_MATRIX
        transform = transforms.get(name, {})
        if "worldRotate" in transform:
            transform = dict(transform)
            transform["rotate"] = float(transform.pop("worldRotate")) - matrix_rotation(parent_matrix)
        matrix = local_matrix(part.get("pivot", [0, 0]), transform)
        if parent:
            matrix = multiply_matrix(parent_matrix, matrix)
        resolving.remove(name)
        local_world[name] = matrix
        return matrix

    matrices = {
        name: multiply_matrix(root_matrix, resolve_local_world(name))
        for name in part_by_name
    }
    matrices["base"] = multiply_matrix(
        root_matrix,
        local_matrix([0, 0], transforms.get("base", {})),
    )
    return matrices

