"""Workspace and rig-path safety for the Sprite Studio MCP server."""

import os
import stat
from pathlib import Path


def bind_workspace(workspace):
    """Resolve the workspace, rig root, and bundled engine; reject unsafe paths."""
    raw_workspace = Path(workspace).expanduser()
    if not raw_workspace.is_dir():
        raise ValueError(f"workspace does not exist: {raw_workspace}")
    resolved = raw_workspace.resolve()
    if raw_workspace.is_symlink():
        raise ValueError("workspace cannot be a symbolic link")
    rig_root = (resolved / ".sprite-studio" / "rigs").resolve()
    engine = (resolved / ".sprite-studio" / "sprite_rig.py").resolve()
    if not rig_root.is_dir() or not rig_root.is_relative_to(resolved):
        raise ValueError("workspace is missing .sprite-studio/rigs")
    if not engine.is_file() or not engine.is_relative_to(resolved):
        raise ValueError("workspace is missing .sprite-studio/sprite_rig.py")
    return resolved, rig_root, engine


def resolve_rig_path(workspace, rig_root, value):
    """Resolve a workspace-relative rig JSON that stays under .sprite-studio/rigs."""
    if not isinstance(value, str) or not value.strip():
        raise ValueError("rig must be a non-empty workspace-relative JSON path")
    supplied = Path(value)
    if supplied.is_absolute():
        raise ValueError("rig must be workspace-relative, not absolute")
    candidate = (workspace / supplied).resolve()
    if not candidate.is_relative_to(rig_root):
        raise ValueError("rig must stay under .sprite-studio/rigs")
    if candidate.suffix.lower() != ".json" or not candidate.is_file():
        raise ValueError("rig must be an existing JSON file under .sprite-studio/rigs")
    try:
        opened = candidate.open("rb")
        opened_stat = os.fstat(opened.fileno())
        path_stat = candidate.stat()
    except OSError as error:
        raise ValueError(f"cannot open rig: {error}") from None
    finally:
        if "opened" in locals():
            opened.close()
    if (opened_stat.st_dev, opened_stat.st_ino) != (path_stat.st_dev, path_stat.st_ino):
        raise ValueError("rig changed while its path was being verified")
    if not stat.S_ISREG(opened_stat.st_mode):
        raise ValueError("rig must be a regular JSON file")
    return candidate
