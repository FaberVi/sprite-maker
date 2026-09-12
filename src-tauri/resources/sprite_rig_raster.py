"""PNG decode, mask cuts, compositing, and deterministic frame rasterization."""

import hashlib
import struct
import zlib

from sprite_rig_json import fail, point_distance
from sprite_rig_matrix import (
    apply_matrix,
    mesh_destination_point,
    mesh_destination_vertices,
    resolve_frame_matrices,
    transformed_matrix,
    transformed_mesh,
)
from sprite_tool import Canvas

def load_png(path):
    try:
        file_size = path.stat().st_size
    except OSError as error:
        fail(f"cannot inspect source PNG: {error}")
    if file_size > 32 * 1024 * 1024:
        fail("source PNG file is too large")
    payload = path.read_bytes()
    payload_hash = hashlib.sha256(payload).hexdigest()
    if payload[:8] != b"\x89PNG\r\n\x1a\n":
        fail(f"source is not a PNG: {path}")
    offset = 8
    width = height = bit_depth = color_type = interlace = None
    compressed = bytearray()
    palette = b""
    transparency = b""
    while offset + 12 <= len(payload):
        length = struct.unpack(">I", payload[offset:offset + 4])[0]
        kind = payload[offset + 4:offset + 8]
        chunk = payload[offset + 8:offset + 8 + length]
        offset += 12 + length
        if kind == b"IHDR":
            width, height, bit_depth, color_type, _, _, interlace = struct.unpack(">IIBBBBB", chunk)
            if not (8 <= width <= 512 and 8 <= height <= 512):
                fail("source canvas must be between 8 and 512 pixels per side")
        elif kind == b"IDAT":
            compressed.extend(chunk)
        elif kind == b"PLTE":
            palette = chunk
        elif kind == b"tRNS":
            transparency = chunk
        elif kind == b"IEND":
            break
    if not width or not height or bit_depth != 8 or color_type not in {2, 3, 6} or interlace != 0:
        fail("source PNG must be non-interlaced 8-bit RGB, RGBA, or indexed color")
    if color_type == 3 and (not palette or len(palette) % 3):
        fail("indexed source PNG is missing a valid palette")
    channels = 4 if color_type == 6 else (3 if color_type == 2 else 1)
    stride = width * channels
    expected = height * (stride + 1)
    decompressor = zlib.decompressobj()
    try:
        raw = decompressor.decompress(bytes(compressed), expected + 1)
        if len(raw) > expected or decompressor.unconsumed_tail:
            fail("source PNG expands beyond its declared canvas")
        raw += decompressor.flush(expected + 1 - len(raw))
    except zlib.error as error:
        fail(f"source PNG has invalid compressed data: {error}")
    if len(raw) > expected or not decompressor.eof:
        fail("source PNG expands beyond its declared canvas")
    if len(raw) != expected:
        fail("source PNG has an unexpected scanline layout")
    rows = []
    cursor = 0
    previous = bytearray(stride)
    for _ in range(height):
        filter_type = raw[cursor]
        cursor += 1
        scanline = bytearray(raw[cursor:cursor + stride])
        cursor += stride
        reconstructed = bytearray(stride)
        for index, value in enumerate(scanline):
            left = reconstructed[index - channels] if index >= channels else 0
            up = previous[index]
            upper_left = previous[index - channels] if index >= channels else 0
            if filter_type == 0:
                predictor = 0
            elif filter_type == 1:
                predictor = left
            elif filter_type == 2:
                predictor = up
            elif filter_type == 3:
                predictor = (left + up) // 2
            elif filter_type == 4:
                estimate = left + up - upper_left
                distances = (abs(estimate - left), abs(estimate - up), abs(estimate - upper_left))
                predictor = (left, up, upper_left)[distances.index(min(distances))]
            else:
                fail(f"unsupported PNG filter {filter_type}")
            reconstructed[index] = (value + predictor) & 255
        rows.append(reconstructed)
        previous = reconstructed
    rgba = bytearray(width * height * 4)
    for y, row in enumerate(rows):
        for x in range(width):
            source_offset = x * channels
            target_offset = (y * width + x) * 4
            if color_type == 3:
                palette_index = row[source_offset]
                palette_offset = palette_index * 3
                if palette_offset + 3 > len(palette):
                    fail("indexed source PNG refers outside its palette")
                rgba[target_offset:target_offset + 3] = palette[palette_offset:palette_offset + 3]
                rgba[target_offset + 3] = transparency[palette_index] if palette_index < len(transparency) else 255
            else:
                rgba[target_offset:target_offset + 3] = row[source_offset:source_offset + 3]
                rgba[target_offset + 3] = row[source_offset + 3] if channels == 4 else 255
    return width, height, rgba, payload_hash


def inside_polygon(x, y, points):
    inside = False
    previous = points[-1]
    for current in points:
        if (current[1] > y) != (previous[1] > y):
            crossing = (previous[0] - current[0]) * (y - current[1]) / (previous[1] - current[1]) + current[0]
            if x < crossing:
                inside = not inside
        previous = current
    return inside


def mask_contains(mask, x, y):
    center_x, center_y = x + 0.5, y + 0.5
    if "rect" in mask:
        left, top, width, height = map(float, mask["rect"])
        return left <= center_x < left + width and top <= center_y < top + height
    if "polygon" in mask:
        points = [(float(point[0]), float(point[1])) for point in mask["polygon"]]
        return len(points) >= 3 and inside_polygon(center_x, center_y, points)
    fail("every rig part requires a rect or polygon mask")


def layer_from_mask(source, width, height, mask):
    layer = bytearray(len(source))
    for y in range(height):
        for x in range(width):
            if mask_contains(mask, x, y):
                offset = (y * width + x) * 4
                layer[offset:offset + 4] = source[offset:offset + 4]
    return layer


def clear_mask(layer, width, height, mask):
    for y in range(height):
        for x in range(width):
            if mask_contains(mask, x, y):
                offset = (y * width + x) * 4
                layer[offset:offset + 4] = b"\x00\x00\x00\x00"


def composite(destination, source):
    for offset in range(0, len(source), 4):
        alpha = source[offset + 3]
        if alpha == 0:
            continue
        if alpha == 255:
            destination[offset:offset + 4] = source[offset:offset + 4]
            continue
        inverse = 255 - alpha
        destination_alpha = destination[offset + 3]
        output_alpha = alpha + destination_alpha * inverse // 255
        if output_alpha == 0:
            continue
        for channel in range(3):
            numerator = source[offset + channel] * alpha + destination[offset + channel] * destination_alpha * inverse // 255
            destination[offset + channel] = numerator // output_alpha
        destination[offset + 3] = output_alpha



def visible_pixel_count(pixels):
    return sum(1 for offset in range(3, len(pixels), 4) if pixels[offset])


def alpha_component_sizes(pixels, width, height):
    remaining = {
        (x, y)
        for y in range(height)
        for x in range(width)
        if pixels[(y * width + x) * 4 + 3]
    }
    sizes = []
    while remaining:
        stack = [remaining.pop()]
        size = 0
        while stack:
            x, y = stack.pop()
            size += 1
            for offset_y in (-1, 0, 1):
                for offset_x in (-1, 0, 1):
                    neighbor = (x + offset_x, y + offset_y)
                    if neighbor in remaining:
                        remaining.remove(neighbor)
                        stack.append(neighbor)
        sizes.append(size)
    return sorted(sizes, reverse=True)


def prepare_layers(source, width, height, parts, rig_version):
    """Split the locked master into a base layer and named movable layers."""
    base = bytearray(source)
    raw_layers = [layer_from_mask(source, width, height, part["mask"]) for part in parts]
    if rig_version >= 2:
        for y in range(height):
            for x in range(width):
                offset = (y * width + x) * 4
                if not source[offset + 3]:
                    continue
                selectors = [
                    index for index, layer in enumerate(raw_layers)
                    if layer[offset + 3]
                ]
                if not selectors:
                    continue
                joint_cap = any(parts[index].get("overlapMode") == "joint-cap" for index in selectors)
                if not joint_cap:
                    owner = max(
                        selectors,
                        key=lambda index: (int(parts[index].get("z", index)), index),
                    )
                    for index in selectors:
                        if index != owner:
                            raw_layers[index][offset:offset + 4] = b"\x00\x00\x00\x00"
    layers = []
    for index, (part, layer) in enumerate(zip(parts, raw_layers)):
        if not visible_pixel_count(layer):
            fail(f"part {part['name']!r} selected no exclusively owned visible pixels")
        if not part.get("keepInBase", False):
            clear_mask(base, width, height, part["mask"])
        layers.append({
            "name": part["name"],
            "pixels": layer,
            "pivot": part.get("pivot", [width / 2, height / 2]),
            "z": int(part.get("z", index)),
            "mesh": part.get("mesh"),
        })
    return base, layers


def frame_clipped_pixels(pixels, width, height, matrix):
    """Count opaque source cells whose transformed area leaves the canvas.

    Sampling only the pixel center misses the common case where a rotated or
    scaled edge cell is visibly shaved by the canvas boundary.  Treat each
    source pixel as a unit square and require all four transformed corners to
    remain inside the locked canvas.
    """
    clipped = 0
    for y in range(height):
        for x in range(width):
            offset = (y * width + x) * 4
            if not pixels[offset + 3]:
                continue
            corners = (
                apply_matrix(matrix, (x, y)),
                apply_matrix(matrix, (x + 1, y)),
                apply_matrix(matrix, (x, y + 1)),
                apply_matrix(matrix, (x + 1, y + 1)),
            )
            if any(
                not (0 <= destination_x <= width and 0 <= destination_y <= height)
                for destination_x, destination_y in corners
            ):
                clipped += 1
    return clipped


def mesh_clipped_pixels(pixels, width, height, mesh, matrices):
    clipped = 0
    destination_vertices = mesh_destination_vertices(mesh, matrices)
    for y in range(height):
        for x in range(width):
            offset = (y * width + x) * 4
            if not pixels[offset + 3]:
                continue
            destination = mesh_destination_point(
                mesh, destination_vertices, (x + 0.5, y + 0.5),
            )
            if destination is None or not (0 <= destination[0] < width and 0 <= destination[1] < height):
                clipped += 1
    return clipped


def render_frame(width, height, base, layers, parts, frame, palette, base_z=0, rig_version=2):
    matrices = resolve_frame_matrices(parts, frame)
    canvas = Canvas(width, height)
    if rig_version == 1:
        composite(canvas.data, transformed_matrix(base, width, height, matrices["base"]))
        for command in frame.get("underlay", []):
            canvas.command(command, palette)
        for layer in sorted(layers, key=lambda item: item["z"]):
            composite(canvas.data, transformed_matrix(
                layer["pixels"], width, height, matrices[layer["name"]],
            ))
        for command in frame.get("overlay", []):
            canvas.command(command, palette)
        return canvas, matrices
    for command in frame.get("underlay", []):
        canvas.command(command, palette)
    z_overrides = frame.get("zOverrides", {})
    draw_items = [{
        "name": "base",
        "pixels": base,
        "z": int(z_overrides.get("base", base_z)),
        "order": -1,
    }]
    for order, layer in enumerate(layers):
        draw_items.append({
            "name": layer["name"],
            "pixels": layer["pixels"],
            "z": int(z_overrides.get(layer["name"], layer["z"])),
            "order": order,
        })
    for item in sorted(draw_items, key=lambda value: (value["z"], value["order"])):
        layer = next((value for value in layers if value["name"] == item["name"]), None)
        if layer is not None and layer.get("mesh"):
            rendered = transformed_mesh(
                item["pixels"], width, height, layer["mesh"], matrices,
            )
        else:
            rendered = transformed_matrix(
                item["pixels"], width, height, matrices[item["name"]],
            )
        composite(canvas.data, rendered)
    for command in frame.get("overlay", []):
        canvas.command(command, palette)
    return canvas, matrices

def nearest_visible_distance(source, width, height, mask, point):
    nearest = float("inf")
    for y in range(height):
        for x in range(width):
            offset = (y * width + x) * 4
            if source[offset + 3] and mask_contains(mask, x, y):
                nearest = min(nearest, point_distance((x + 0.5, y + 0.5), point))
    return nearest


