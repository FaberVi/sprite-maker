use crate::error::{CommandError, CommandResult};
use crate::models::TerrainExportInput;
use image::{DynamicImage, GenericImageView, RgbaImage};
use std::collections::{HashMap, HashSet};

pub(super) const TERRAIN_RULE_ROLES: [&str; 9] = [
    "top_left",
    "top",
    "top_right",
    "left",
    "center",
    "right",
    "bottom_left",
    "bottom",
    "bottom_right",
];

pub(super) const BLOB_COLUMNS: u32 = 8;
const TERRAIN_BITS: [(u8, &str); 8] = [
    (1, "top_side"),
    (2, "top_right_corner"),
    (4, "right_side"),
    (8, "bottom_right_corner"),
    (16, "bottom_side"),
    (32, "bottom_left_corner"),
    (64, "left_side"),
    (128, "top_left_corner"),
];

#[derive(Debug, PartialEq)]
pub(super) struct GridLayout {
    pub(super) columns: u32,
    pub(super) rows: u32,
    pub(super) trailing_x: u32,
    pub(super) trailing_y: u32,
}

fn axis_layout(image_size: u32, margin: u32, tile: u32, separation: u32) -> Option<(u32, u32)> {
    if tile == 0 || margin >= image_size || image_size - margin < tile {
        return None;
    }
    let available = image_size - margin;
    let stride = tile.checked_add(separation)?;
    let count = 1 + (available - tile) / stride;
    let consumed = count
        .checked_mul(tile)?
        .checked_add(count.saturating_sub(1).checked_mul(separation)?)?;
    Some((count, available - consumed))
}

pub(super) fn grid_layout(
    image_width: u32,
    image_height: u32,
    input: &TerrainExportInput,
) -> CommandResult<GridLayout> {
    let (columns, trailing_x) = axis_layout(
        image_width,
        input.margin_x,
        input.tile_width,
        input.separation_x,
    )
    .ok_or_else(|| {
        CommandError::new(
            "invalid_terrain_grid",
            "Tile width and horizontal margin do not fit inside the atlas",
        )
    })?;
    let (rows, trailing_y) = axis_layout(
        image_height,
        input.margin_y,
        input.tile_height,
        input.separation_y,
    )
    .ok_or_else(|| {
        CommandError::new(
            "invalid_terrain_grid",
            "Tile height and vertical margin do not fit inside the atlas",
        )
    })?;
    Ok(GridLayout {
        columns,
        rows,
        trailing_x,
        trailing_y,
    })
}

pub(super) fn terrain_peering_bits(role: &str) -> Option<Vec<&'static str>> {
    if let Some(mask) = role
        .strip_prefix("blob_")
        .and_then(|value| value.parse::<u8>().ok())
    {
        if is_blob_mask(mask) {
            return Some(
                TERRAIN_BITS
                    .iter()
                    .filter_map(|(bit, name)| (mask & bit != 0).then_some(*name))
                    .collect(),
            );
        }
        return None;
    }
    match role {
        "top_left" => Some(vec!["right_side", "bottom_right_corner", "bottom_side"]),
        "top" => Some(vec![
            "right_side",
            "bottom_right_corner",
            "bottom_side",
            "bottom_left_corner",
            "left_side",
        ]),
        "top_right" => Some(vec!["bottom_side", "bottom_left_corner", "left_side"]),
        "left" => Some(vec![
            "top_side",
            "top_right_corner",
            "right_side",
            "bottom_right_corner",
            "bottom_side",
        ]),
        "center" => Some(vec![
            "right_side",
            "bottom_right_corner",
            "bottom_side",
            "bottom_left_corner",
            "left_side",
            "top_left_corner",
            "top_side",
            "top_right_corner",
        ]),
        "right" => Some(vec![
            "bottom_side",
            "bottom_left_corner",
            "left_side",
            "top_left_corner",
            "top_side",
        ]),
        "bottom_left" => Some(vec!["top_side", "top_right_corner", "right_side"]),
        "bottom" => Some(vec![
            "left_side",
            "top_left_corner",
            "top_side",
            "top_right_corner",
            "right_side",
        ]),
        "bottom_right" => Some(vec!["left_side", "top_left_corner", "top_side"]),
        _ => None,
    }
}

pub(super) fn is_blob_mask(mask: u8) -> bool {
    let top = mask & 1 != 0;
    let top_right = mask & 2 != 0;
    let right = mask & 4 != 0;
    let bottom_right = mask & 8 != 0;
    let bottom = mask & 16 != 0;
    let bottom_left = mask & 32 != 0;
    let left = mask & 64 != 0;
    let top_left = mask & 128 != 0;
    (!top_right || top && right)
        && (!bottom_right || right && bottom)
        && (!bottom_left || bottom && left)
        && (!top_left || left && top)
}

pub(super) fn blob_masks() -> Vec<u8> {
    (0..=u8::MAX).filter(|mask| is_blob_mask(*mask)).collect()
}

pub(super) fn blob_rules() -> Vec<crate::models::TerrainRuleInput> {
    blob_masks()
        .into_iter()
        .enumerate()
        .map(|(index, mask)| crate::models::TerrainRuleInput {
            role: format!("blob_{mask}"),
            column: index as u32 % BLOB_COLUMNS,
            row: index as u32 / BLOB_COLUMNS,
        })
        .collect()
}

pub(super) fn terrain_mode(input: &TerrainExportInput) -> CommandResult<&'static str> {
    if input.terrain_rules.is_empty() {
        return Ok("plain");
    }
    match input.terrain_mode.as_deref().unwrap_or("nine_slice") {
        "nine_slice" => Ok("nine_slice"),
        "blob_47" => Ok("blob_47"),
        _ => Err(CommandError::new(
            "invalid_terrain_mode",
            "Terrain rule mode must be nine_slice or blob_47",
        )),
    }
}

pub(super) fn validate_terrain_rules(
    input: &TerrainExportInput,
    layout: &GridLayout,
    occupied: &[(u32, u32)],
) -> CommandResult<()> {
    if input.terrain_rules.is_empty() {
        return Ok(());
    }
    if input.terrain_rules.len() != TERRAIN_RULE_ROLES.len() {
        return Err(CommandError::new(
            "invalid_terrain_rules",
            "3x3 auto-connect requires all nine terrain roles",
        ));
    }
    let mut roles = HashSet::new();
    let mut coordinates = HashSet::new();
    let occupied: HashSet<(u32, u32)> = occupied.iter().copied().collect();
    for rule in &input.terrain_rules {
        if terrain_peering_bits(&rule.role).is_none() || !roles.insert(rule.role.as_str()) {
            return Err(CommandError::new(
                "invalid_terrain_rules",
                "Terrain roles must be the nine unique 3x3 positions",
            ));
        }
        if rule.column >= layout.columns || rule.row >= layout.rows {
            return Err(CommandError::new(
                "invalid_terrain_rules",
                format!(
                    "The {} rule points outside the detected atlas grid",
                    rule.role.replace('_', " ")
                ),
            ));
        }
        if !coordinates.insert((rule.column, rule.row)) {
            return Err(CommandError::new(
                "invalid_terrain_rules",
                "Each terrain role must use a different atlas cell",
            ));
        }
        if !occupied.contains(&(rule.column, rule.row)) {
            return Err(CommandError::new(
                "invalid_terrain_rules",
                format!(
                    "The {} rule points to a fully transparent cell",
                    rule.role.replace('_', " ")
                ),
            ));
        }
    }
    if TERRAIN_RULE_ROLES.iter().any(|role| !roles.contains(role)) {
        return Err(CommandError::new(
            "invalid_terrain_rules",
            "3x3 auto-connect requires all nine terrain roles",
        ));
    }
    Ok(())
}

fn source_rule_coordinates(
    input: &TerrainExportInput,
) -> CommandResult<HashMap<String, (u32, u32)>> {
    let coordinates = input
        .terrain_rules
        .iter()
        .map(|rule| (rule.role.clone(), (rule.column, rule.row)))
        .collect::<HashMap<_, _>>();
    if TERRAIN_RULE_ROLES
        .iter()
        .any(|role| !coordinates.contains_key(*role))
    {
        return Err(CommandError::new(
            "invalid_terrain_rules",
            "Complete 47-tile terrain generation requires all nine source roles",
        ));
    }
    Ok(coordinates)
}

fn quadrant_source_role(mask: u8, quadrant: u8) -> &'static str {
    let (side_a, side_b, diagonal, outer, edge_a, edge_b) = match quadrant {
        0 => (1, 64, 128, "top_left", "left", "top"),
        1 => (1, 4, 2, "top_right", "right", "top"),
        2 => (16, 4, 8, "bottom_right", "right", "bottom"),
        _ => (16, 64, 32, "bottom_left", "left", "bottom"),
    };
    match (mask & side_a != 0, mask & side_b != 0) {
        (false, false) => outer,
        (true, false) => edge_a,
        (false, true) => edge_b,
        (true, true) if mask & diagonal != 0 => "center",
        (true, true) => outer,
    }
}

pub(super) fn expand_blob_atlas(
    image: &DynamicImage,
    input: &TerrainExportInput,
) -> CommandResult<DynamicImage> {
    if input.tile_width < 2
        || input.tile_height < 2
        || !input.tile_width.is_multiple_of(2)
        || !input.tile_height.is_multiple_of(2)
    {
        return Err(CommandError::new(
            "invalid_blob_tile_size",
            "Complete 47-tile terrain generation requires even tile dimensions of at least 2 pixels",
        ));
    }
    let coordinates = source_rule_coordinates(input)?;
    let masks = blob_masks();
    let rows = (masks.len() as u32).div_ceil(BLOB_COLUMNS);
    let mut output = RgbaImage::new(BLOB_COLUMNS * input.tile_width, rows * input.tile_height);
    let half_width = input.tile_width / 2;
    let half_height = input.tile_height / 2;

    for (index, mask) in masks.into_iter().enumerate() {
        let output_column = index as u32 % BLOB_COLUMNS;
        let output_row = index as u32 / BLOB_COLUMNS;
        for y in 0..input.tile_height {
            for x in 0..input.tile_width {
                let quadrant = match (y < half_height, x < half_width) {
                    (true, true) => 0,
                    (true, false) => 1,
                    (false, false) => 2,
                    (false, true) => 3,
                };
                let role = quadrant_source_role(mask, quadrant);
                let (source_column, source_row) = coordinates[role];
                let source_x =
                    input.margin_x + source_column * (input.tile_width + input.separation_x) + x;
                let source_y =
                    input.margin_y + source_row * (input.tile_height + input.separation_y) + y;
                output.put_pixel(
                    output_column * input.tile_width + x,
                    output_row * input.tile_height + y,
                    image.get_pixel(source_x, source_y),
                );
            }
        }
    }
    Ok(DynamicImage::ImageRgba8(output))
}
