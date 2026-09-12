use crate::{
    assets::get_asset,
    error::{CommandError, CommandResult},
    models::{TerrainExportInput, TerrainExportResult},
    workspace::workspace_path,
    AppState,
};
use image::{GenericImageView, ImageFormat};
use rusqlite::OptionalExtension;
use std::path::PathBuf;
use tauri::State;

use super::blob::{
    blob_rules, expand_blob_atlas, grid_layout, terrain_mode, terrain_peering_bits,
    validate_terrain_rules, GridLayout, BLOB_COLUMNS,
};

pub(super) fn slug(value: &str) -> String {
    let mut output = String::new();
    let mut separator = false;
    for character in value.trim().chars() {
        if character.is_ascii_alphanumeric() {
            output.push(character.to_ascii_lowercase());
            separator = false;
        } else if !separator && !output.is_empty() {
            output.push('-');
            separator = true;
        }
    }
    while output.ends_with('-') {
        output.pop();
    }
    if output.is_empty() {
        "terrain-tileset".into()
    } else {
        output
    }
}

fn cell_has_pixels(
    image: &image::DynamicImage,
    column: u32,
    row: u32,
    input: &TerrainExportInput,
) -> bool {
    let start_x = input.margin_x + column * (input.tile_width + input.separation_x);
    let start_y = input.margin_y + row * (input.tile_height + input.separation_y);
    (start_y..start_y + input.tile_height)
        .any(|y| (start_x..start_x + input.tile_width).any(|x| image.get_pixel(x, y).0[3] != 0))
}

fn godot_string(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

pub(super) fn godot_resource(
    resource_texture_path: &str,
    input: &TerrainExportInput,
    layout: &GridLayout,
    occupied: &[(u32, u32)],
) -> String {
    let mut resource = format!(
        "[gd_resource type=\"TileSet\" load_steps=3 format=3]\n\n\
[ext_resource type=\"Texture2D\" path=\"{resource_texture_path}\" id=\"1_texture\"]\n\n\
[sub_resource type=\"TileSetAtlasSource\" id=\"TileSetAtlasSource_terrain\"]\n\
texture = ExtResource(\"1_texture\")\n\
texture_region_size = Vector2i({}, {})\n\
margins = Vector2i({}, {})\n\
separation = Vector2i({}, {})\n",
        input.tile_width,
        input.tile_height,
        input.margin_x,
        input.margin_y,
        input.separation_x,
        input.separation_y,
    );
    for (column, row) in occupied {
        resource.push_str(&format!("{column}:{row}/0 = 0\n"));
    }
    for rule in &input.terrain_rules {
        resource.push_str(&format!(
            "{column}:{row}/0/terrain_set = 0\n{column}:{row}/0/terrain = 0\n",
            column = rule.column,
            row = rule.row,
        ));
        if let Some(bits) = terrain_peering_bits(&rule.role) {
            for bit in bits {
                resource.push_str(&format!(
                    "{column}:{row}/0/terrains_peering_bit/{bit} = 0\n",
                    column = rule.column,
                    row = rule.row,
                ));
            }
        }
    }
    let terrain_metadata = if input.terrain_rules.is_empty() {
        String::new()
    } else {
        let terrain_name = input
            .terrain_name
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("Terrain");
        format!(
            "terrain_set_0/mode = 0\nterrain_set_0/terrain_0/name = \"{}\"\nterrain_set_0/terrain_0/color = Color(0.34901962, 0.65882355, 0.41568628, 1)\n",
            godot_string(terrain_name)
        )
    };
    resource.push_str(&format!(
        "\n[resource]\n\
tile_size = Vector2i({}, {})\n\
{}\
sources/0 = SubResource(\"TileSetAtlasSource_terrain\")\n",
        input.tile_width, input.tile_height, terrain_metadata
    ));
    debug_assert!(occupied.len() <= (layout.columns * layout.rows) as usize);
    resource
}

fn validate_worktree(state: &AppState, input: &TerrainExportInput) -> CommandResult<()> {
    let connection = state
        .db
        .lock()
        .map_err(|_| CommandError::new("database_locked", "Database lock was poisoned"))?;
    let kind: Option<String> = connection
        .query_row(
            "SELECT kind FROM worktrees WHERE id=?1 AND project_id=?2",
            [&input.worktree_id, &input.project_id],
            |row| row.get(0),
        )
        .optional()?;
    if kind.as_deref() != Some("tileset") {
        return Err(CommandError::new(
            "invalid_terrain_worktree",
            "Godot terrain exports require a Terrain worktree",
        ));
    }
    Ok(())
}

#[tauri::command]
pub fn export_godot_tileset(
    input: TerrainExportInput,
    state: State<'_, AppState>,
) -> CommandResult<TerrainExportResult> {
    export_godot_tileset_inner(input, &state)
}

pub(crate) fn export_godot_tileset_inner(
    input: TerrainExportInput,
    state: &AppState,
) -> CommandResult<TerrainExportResult> {
    validate_worktree(state, &input)?;
    let asset = get_asset(state, &input.asset_id)?;
    if asset.workspace_id != input.project_id {
        return Err(CommandError::new(
            "invalid_terrain_asset",
            "The selected atlas does not belong to this project",
        ));
    }
    let source = PathBuf::from(&asset.path);
    if !source.is_file() {
        return Err(CommandError::new(
            "asset_missing",
            "The selected terrain atlas was moved or deleted",
        ));
    }
    let image = image::open(&source)?;
    let layout = grid_layout(image.width(), image.height(), &input)?;
    let mut visible = Vec::new();
    for row in 0..layout.rows {
        for column in 0..layout.columns {
            if cell_has_pixels(&image, column, row, &input) {
                visible.push((column, row));
            }
        }
    }
    let occupied = if input.include_empty {
        (0..layout.rows)
            .flat_map(|row| (0..layout.columns).map(move |column| (column, row)))
            .collect::<Vec<_>>()
    } else {
        visible.clone()
    };
    if occupied.is_empty() {
        return Err(CommandError::new(
            "empty_terrain_atlas",
            "No visible tiles were found. Enable empty cells or adjust the grid",
        ));
    }
    validate_terrain_rules(&input, &layout, &visible)?;
    let mode = terrain_mode(&input)?;

    let (export_image, export_input, export_layout, export_occupied) = if mode == "blob_47" {
        let generated = expand_blob_atlas(&image, &input)?;
        let mut generated_input = input.clone();
        generated_input.margin_x = 0;
        generated_input.margin_y = 0;
        generated_input.separation_x = 0;
        generated_input.separation_y = 0;
        generated_input.include_empty = false;
        generated_input.terrain_rules = blob_rules();
        let generated_layout = GridLayout {
            columns: BLOB_COLUMNS,
            rows: 6,
            trailing_x: 0,
            trailing_y: 0,
        };
        let generated_occupied = generated_input
            .terrain_rules
            .iter()
            .map(|rule| (rule.column, rule.row))
            .collect::<Vec<_>>();
        (
            generated,
            generated_input,
            generated_layout,
            generated_occupied,
        )
    } else {
        (image, input.clone(), layout, occupied)
    };

    let root = workspace_path(state, &input.project_id)?;
    let export_slug = slug(&input.name);
    let directory = root.join("exports").join("godot").join(&export_slug);
    std::fs::create_dir_all(&directory)?;
    let texture_path = directory.join(format!("{export_slug}.png"));
    let resource_path = directory.join(format!("{export_slug}.tres"));
    export_image.save_with_format(&texture_path, ImageFormat::Png)?;
    let godot_texture_path = format!("res://exports/godot/{export_slug}/{export_slug}.png");
    let contents = godot_resource(
        &godot_texture_path,
        &export_input,
        &export_layout,
        &export_occupied,
    );
    std::fs::write(&resource_path, contents)?;

    Ok(TerrainExportResult {
        directory_path: directory.to_string_lossy().into_owned(),
        texture_path: texture_path.to_string_lossy().into_owned(),
        resource_path: resource_path.to_string_lossy().into_owned(),
        columns: export_layout.columns,
        rows: export_layout.rows,
        tile_count: export_layout.columns * export_layout.rows,
        occupied_tile_count: export_occupied.len() as u32,
        trailing_x: export_layout.trailing_x,
        trailing_y: export_layout.trailing_y,
        terrain_rule_count: export_input.terrain_rules.len() as u32,
        terrain_mode: mode.into(),
    })
}
