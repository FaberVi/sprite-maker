use super::{
    blob_masks, blob_rules, expand_blob_atlas, godot_resource, grid_layout, is_blob_mask, slug,
    validate_terrain_rules, GridLayout, BLOB_COLUMNS, TERRAIN_RULE_ROLES,
};
use crate::models::TerrainExportInput;
use image::{DynamicImage, GenericImageView, Rgba, RgbaImage};

fn input() -> TerrainExportInput {
    TerrainExportInput {
        project_id: "project".into(),
        worktree_id: "terrain".into(),
        asset_id: "atlas".into(),
        name: "Forest Ground".into(),
        tile_width: 16,
        tile_height: 16,
        margin_x: 1,
        margin_y: 1,
        separation_x: 1,
        separation_y: 1,
        include_empty: false,
        terrain_name: None,
        terrain_mode: None,
        terrain_rules: Vec::new(),
    }
}

#[test]
fn calculates_bordered_atlas_grid_without_partial_cells() {
    let layout = grid_layout(69, 69, &input()).expect("grid should fit");
    assert_eq!(
        layout,
        GridLayout {
            columns: 4,
            rows: 4,
            trailing_x: 1,
            trailing_y: 1,
        }
    );
}

#[test]
fn rejects_tiles_larger_than_the_atlas() {
    assert!(grid_layout(8, 8, &input()).is_err());
}

#[test]
fn writes_native_godot_tileset_atlas_resource() {
    let layout = GridLayout {
        columns: 2,
        rows: 1,
        trailing_x: 0,
        trailing_y: 0,
    };
    let resource = godot_resource(
        "res://exports/godot/forest/forest.png",
        &input(),
        &layout,
        &[(0, 0), (1, 0)],
    );
    assert!(resource.contains("type=\"TileSet\""));
    assert!(resource.contains("texture_region_size = Vector2i(16, 16)"));
    assert!(resource.contains("0:0/0 = 0"));
    assert!(resource.contains("1:0/0 = 0"));
    assert_eq!(slug("  Forest Ground!  "), "forest-ground");
}

#[test]
fn writes_nine_slice_terrain_metadata() {
    let mut input = input();
    input.terrain_name = Some("Ground \"A\"".into());
    input.terrain_rules = TERRAIN_RULE_ROLES
        .iter()
        .enumerate()
        .map(|(index, role)| crate::models::TerrainRuleInput {
            role: (*role).into(),
            column: (index % 3) as u32,
            row: (index / 3) as u32,
        })
        .collect();
    let layout = GridLayout {
        columns: 3,
        rows: 3,
        trailing_x: 0,
        trailing_y: 0,
    };
    let occupied = (0..3)
        .flat_map(|row| (0..3).map(move |column| (column, row)))
        .collect::<Vec<_>>();
    validate_terrain_rules(&input, &layout, &occupied).expect("rules should be valid");
    let resource = godot_resource("res://terrain.png", &input, &layout, &occupied);
    assert!(resource.contains("terrain_set_0/mode = 0"));
    assert!(resource.contains("terrain_0/name = \"Ground \\\"A\\\"\""));
    assert!(resource.contains("1:1/0/terrains_peering_bit/top_left_corner = 0"));
    assert!(resource.contains("0:0/0/terrains_peering_bit/bottom_right_corner = 0"));
}

#[test]
fn enumerates_the_canonical_47_blob_masks() {
    let masks = blob_masks();
    assert_eq!(masks.len(), 47);
    assert_eq!(masks.first(), Some(&0));
    assert_eq!(masks.last(), Some(&255));
    assert!(!is_blob_mask(2));
    assert!(!is_blob_mask(128));
    assert!(is_blob_mask(255));
    let rules = blob_rules();
    assert_eq!(rules.len(), 47);
    assert_eq!(rules[46].role, "blob_255");
    assert_eq!((rules[46].column, rules[46].row), (6, 5));
}

#[test]
fn expands_nine_source_roles_into_an_eight_by_six_blob_atlas() {
    let mut input = input();
    input.margin_x = 0;
    input.margin_y = 0;
    input.separation_x = 0;
    input.separation_y = 0;
    input.tile_width = 4;
    input.tile_height = 4;
    input.terrain_mode = Some("blob_47".into());
    input.terrain_rules = TERRAIN_RULE_ROLES
        .iter()
        .enumerate()
        .map(|(index, role)| crate::models::TerrainRuleInput {
            role: (*role).into(),
            column: (index % 3) as u32,
            row: (index / 3) as u32,
        })
        .collect();
    let mut pixels = RgbaImage::new(12, 12);
    for (index, _) in TERRAIN_RULE_ROLES.iter().enumerate() {
        let color = Rgba([index as u8 + 1, 0, 0, 255]);
        let column = index as u32 % 3;
        let row = index as u32 / 3;
        for y in row * 4..row * 4 + 4 {
            for x in column * 4..column * 4 + 4 {
                pixels.put_pixel(x, y, color);
            }
        }
    }
    let expanded = expand_blob_atlas(&DynamicImage::ImageRgba8(pixels), &input)
        .expect("blob atlas should generate");
    assert_eq!(expanded.dimensions(), (32, 24));
    assert_eq!(expanded.get_pixel(0, 0), Rgba([1, 0, 0, 255]));
    let full_index = blob_masks().iter().position(|mask| *mask == 255).unwrap() as u32;
    let full_x = (full_index % BLOB_COLUMNS) * 4;
    let full_y = (full_index / BLOB_COLUMNS) * 4;
    assert_eq!(expanded.get_pixel(full_x, full_y), Rgba([5, 0, 0, 255]));
}
