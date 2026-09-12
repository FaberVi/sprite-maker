mod blob;
mod godot;

#[cfg(test)]
use blob::{
    blob_masks, blob_rules, expand_blob_atlas, grid_layout, is_blob_mask, validate_terrain_rules,
    GridLayout, BLOB_COLUMNS, TERRAIN_RULE_ROLES,
};
pub(crate) use godot::export_godot_tileset_inner;
pub use godot::{
    __cmd__export_godot_tileset, __tauri_command_name_export_godot_tileset, export_godot_tileset,
};
#[cfg(test)]
use godot::{godot_resource, slug};

#[cfg(test)]
mod tests;
