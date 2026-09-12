use crate::error::CommandResult;
use std::path::Path;

pub(super) const BUNDLED_PYTHON: &[(&str, &str)] = &[
    (
        "sprite_tool.py",
        include_str!("../../resources/sprite_tool.py"),
    ),
    (
        "sprite_rig.py",
        include_str!("../../resources/sprite_rig.py"),
    ),
    (
        "sprite_rig_json.py",
        include_str!("../../resources/sprite_rig_json.py"),
    ),
    (
        "sprite_rig_matrix.py",
        include_str!("../../resources/sprite_rig_matrix.py"),
    ),
    (
        "sprite_rig_raster.py",
        include_str!("../../resources/sprite_rig_raster.py"),
    ),
    (
        "sprite_rig_schema.py",
        include_str!("../../resources/sprite_rig_schema.py"),
    ),
    (
        "sprite_rig_ik.py",
        include_str!("../../resources/sprite_rig_ik.py"),
    ),
    (
        "sprite_rig_lock.py",
        include_str!("../../resources/sprite_rig_lock.py"),
    ),
    (
        "sprite_rig_commit.py",
        include_str!("../../resources/sprite_rig_commit.py"),
    ),
    (
        "sprite_rig_profile.py",
        include_str!("../../resources/sprite_rig_profile.py"),
    ),
    (
        "sprite_rig_gait.py",
        include_str!("../../resources/sprite_rig_gait.py"),
    ),
    (
        "sprite_rig_motion.py",
        include_str!("../../resources/sprite_rig_motion.py"),
    ),
    (
        "sprite_rig_validate.py",
        include_str!("../../resources/sprite_rig_validate.py"),
    ),
    (
        "sprite_rig_mcp.py",
        include_str!("../../resources/sprite_rig_mcp.py"),
    ),
    (
        "sprite_rig_mcp_paths.py",
        include_str!("../../resources/sprite_rig_mcp_paths.py"),
    ),
    (
        "sprite_rig_mcp_analysis.py",
        include_str!("../../resources/sprite_rig_mcp_analysis.py"),
    ),
    (
        "sprite_polish.py",
        include_str!("../../resources/sprite_polish.py"),
    ),
    (
        "terrain_cleanup.py",
        include_str!("../../resources/terrain_cleanup.py"),
    ),
];

pub(super) fn initialize_workspace(path: &Path) -> CommandResult<()> {
    for folder in [
        "assets/characters",
        "assets/creatures",
        "assets/terrain",
        "assets/props",
        "assets/effects",
        "animations",
        "exports",
        "worktrees",
    ] {
        std::fs::create_dir_all(path.join(folder))?;
    }
    let metadata = path.join(".sprite-studio");
    std::fs::create_dir_all(&metadata)?;
    std::fs::create_dir_all(metadata.join("packs"))?;
    let gitignore = metadata.join(".gitignore");
    if !gitignore.exists() {
        std::fs::write(gitignore, "thumbnails/\n")?;
    }
    for &(filename, bundled) in BUNDLED_PYTHON {
        let installed = metadata.join(filename);
        if !installed.exists() || std::fs::read_to_string(&installed)? != bundled {
            std::fs::write(installed, bundled)?;
        }
    }
    Ok(())
}
