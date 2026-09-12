"""Workspace path safety, generation manifests, and atomic render commits."""

import hashlib
import os
import tempfile
from datetime import datetime, timezone
from pathlib import Path

from sprite_rig_json import fail, strict_json_dumps, strict_json_loads
from sprite_rig_lock import (
    atomic_restore,
    atomic_write_text,
    copy_synced,
    ensure_regular_or_missing,
    fsync_directory,
    fsync_path,
    remove_exact_tree,
    sha256_file,
    stage_text,
)
from sprite_rig_raster import load_png

def safe_source(workspace, relative):
    workspace = workspace.resolve()
    path = (workspace / relative).resolve()
    allowed = [workspace / "assets", workspace / ".sprite-studio"]
    if not path.is_file() or not path.is_relative_to(workspace) or not any(
        path.is_relative_to(root) for root in allowed
    ):
        fail("source must be an existing PNG under assets/ or .sprite-studio/")
    return path


def safe_output_path(workspace, relative, label):
    """Resolve a workspace-owned write target without following it outside."""
    workspace = workspace.resolve()
    path = workspace / relative
    probe = path.parent
    while not probe.exists() and probe != workspace:
        probe = probe.parent
    if not probe.resolve().is_relative_to(workspace):
        fail(f"{label} directory must stay inside the workspace")
    if path.is_symlink():
        fail(f"{label} cannot be a symbolic link")
    return path


def acquire_render_lock(workspace):
    lock_path = safe_output_path(
        workspace, Path(".sprite-studio") / "rig-render.lock", "rig render lock",
    )
    ensure_regular_or_missing(lock_path, "rig render lock")
    handle = lock_path.open("a+b")
    try:
        if os.name == "nt":
            import msvcrt
            if lock_path.stat().st_size == 0:
                handle.write(b"\0")
                handle.flush()
            handle.seek(0)
            msvcrt.locking(handle.fileno(), msvcrt.LK_NBLCK, 1)
        else:
            import fcntl
            fcntl.flock(handle.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
    except (OSError, BlockingIOError):
        handle.close()
        fail("another rig render is already committing in this workspace")
    return handle

def read_previous_manifest(path):
    ensure_regular_or_missing(path, "generation manifest")
    if not path.exists():
        return {}
    try:
        value = strict_json_loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, ValueError):
        return {}
    return value if isinstance(value, dict) else {}


def manifest_output_paths(workspace, manifest, name, category, rig_relative):
    if not (
        manifest.get("name") == name
        and manifest.get("category") == category
        and manifest.get("rig") == rig_relative
    ):
        return None
    files = manifest.get("files")
    if not isinstance(files, list) or not (1 <= len(files) <= 32):
        fail("active rig manifest has an invalid frame list")
    paths = []
    seen = set()
    expected_parent = Path("assets") / category
    for index, value in enumerate(files):
        if not isinstance(value, str) or not value:
            fail(f"active rig manifest frame {index + 1} must be a workspace-relative path")
        relative = Path(value)
        expected_relative = expected_parent / f"{name}_{index + 1:02d}.png"
        if (
            relative.is_absolute()
            or relative != expected_relative
        ):
            fail("active rig manifest frames must be the exact ordered output sequence")
        path = safe_output_path(workspace, relative, f"active frame {index + 1}")
        if path in seen:
            fail("active rig manifest contains duplicate frame paths")
        seen.add(path)
        paths.append(path)
    return paths

def commit_render(
    workspace,
    spec_path,
    source_path,
    original_spec_bytes,
    spec,
    canvases,
    manifest,
    output_paths,
):
    name = manifest["name"]
    category = manifest["category"]
    rig_relative = manifest["rig"]
    destination = safe_output_path(workspace, Path("assets") / category, "output asset")
    if destination.exists() and not destination.is_dir():
        fail("output asset directory must be a directory")
    destination.mkdir(parents=True, exist_ok=True)
    destination = destination.resolve()
    if not destination.is_relative_to(workspace):
        fail("output asset directory must stay inside the workspace")

    metadata = safe_output_path(workspace, Path(".sprite-studio"), "Sprite Studio metadata")
    if metadata.exists() and not metadata.is_dir():
        fail("Sprite Studio metadata must be a directory")
    metadata.mkdir(parents=True, exist_ok=True)
    metadata = metadata.resolve()
    if not metadata.is_relative_to(workspace):
        fail("Sprite Studio metadata directory must stay inside the workspace")

    manifest_record = safe_output_path(
        workspace, Path(".sprite-studio") / "last-generation.json", "generation manifest",
    )
    ensure_regular_or_missing(manifest_record, "generation manifest")
    ensure_regular_or_missing(spec_path, "rig record")
    for index, path in enumerate(output_paths):
        ensure_regular_or_missing(path, f"output frame {index + 1}")
        if path.resolve() == source_path.resolve() or (
            path.exists() and source_path.exists() and os.path.samefile(path, source_path)
        ):
            fail(f"output frame {index + 1} cannot replace the locked source master")

    previous = read_previous_manifest(manifest_record)
    previous_paths = manifest_output_paths(
        workspace, previous, name, category, rig_relative,
    )
    collisions = [path for path in output_paths if path.exists()]
    if collisions and previous_paths is None:
        fail("output frames exist but are not owned by this exact rig")
    old_active_paths = []
    if previous_paths is not None and collisions:
        for index, path in enumerate(previous_paths):
            ensure_regular_or_missing(path, f"active frame {index + 1}")
            if not path.exists():
                fail("the active rig output set is incomplete; restore it before rendering")
            if path.resolve() == source_path.resolve() or os.path.samefile(path, source_path):
                fail("the active rig output set aliases the locked source master")
        if any(path not in previous_paths for path in collisions):
            fail("an output frame collides with a file outside this rig's active result")
        old_active_paths = previous_paths

    transaction_root = safe_output_path(
        workspace, Path(".sprite-studio") / "transactions", "render transaction directory",
    )
    if transaction_root.exists() and not transaction_root.is_dir():
        fail("render transaction path must be a directory")
    transaction_root.mkdir(parents=True, exist_ok=True)
    transaction_dir = Path(tempfile.mkdtemp(prefix=f"{name}-", dir=str(transaction_root)))
    frame_stage = Path(tempfile.mkdtemp(prefix=f".{name}-stage-", dir=str(destination)))
    preserve_transaction = False
    try:
        staged_frames = []
        expected_hashes = manifest["quality"].get("renderedFrameHashes", [])
        if len(expected_hashes) != len(canvases):
            fail("validated render did not produce one raster hash per output frame")
        for index, (canvas, expected_hash) in enumerate(zip(canvases, expected_hashes)):
            staged = frame_stage / output_paths[index].name
            canvas.save_png(staged)
            fsync_path(staged)
            staged_width, staged_height, staged_pixels, _ = load_png(staged)
            staged_hash = hashlib.sha256(bytes(staged_pixels)).hexdigest()
            if (
                staged_width != canvas.width
                or staged_height != canvas.height
                or staged_hash != expected_hash
            ):
                fail(f"staged output frame {index + 1} failed deterministic hash verification")
            staged_frames.append(staged)

        normalized_spec = strict_json_dumps(spec, indent=2) + "\n"
        manifest_text = strict_json_dumps(manifest, indent=2) + "\n"
        staged_spec = stage_text(
            transaction_dir,
            "normalized-rig-",
            normalized_spec,
            spec_path.stat().st_mode,
        )
        manifest_mode = manifest_record.stat().st_mode if manifest_record.exists() else None
        staged_manifest = stage_text(
            transaction_dir, "manifest-", manifest_text, manifest_mode,
        )

        backup_root = transaction_dir / "backups"
        frame_backups = backup_root / "frames"
        state_backups = backup_root / "state"
        frame_backups.mkdir(parents=True)
        state_backups.mkdir(parents=True)
        fsync_directory(transaction_dir)
        fsync_directory(backup_root)
        backups = {}
        original_existence = {}
        mutation_targets = list(dict.fromkeys(old_active_paths + output_paths + [spec_path, manifest_record]))
        for index, target in enumerate(mutation_targets):
            exists = target.exists()
            original_existence[target] = exists
            if not exists:
                continue
            backup_parent = frame_backups if target in old_active_paths else state_backups
            backup = (
                backup_parent / target.name
                if target in old_active_paths
                else backup_parent / f"{index:03d}-{target.name}"
            )
            copy_synced(target, backup)
            backups[target] = backup
        fsync_directory(frame_backups)
        fsync_directory(state_backups)
        fsync_directory(backup_root)

        journal_targets = []
        for target in mutation_targets:
            backup = backups.get(target)
            journal_targets.append({
                "path": str(target.relative_to(workspace)),
                "existed": original_existence[target],
                "backup": str(backup.relative_to(transaction_dir)) if backup else None,
                "sha256": sha256_file(backup) if backup else None,
            })
        journal = {
            "state": "prepared",
            "name": name,
            "rig": rig_relative,
            "outputs": [str(path.relative_to(workspace)) for path in output_paths],
            "frameStage": str(frame_stage.relative_to(workspace)),
            "targets": journal_targets,
            "createdAt": datetime.now(timezone.utc).isoformat(),
        }
        journal_path = transaction_dir / "journal.json"
        atomic_write_text(journal_path, strict_json_dumps(journal, indent=2) + "\n")

        if spec_path.read_bytes() != original_spec_bytes:
            fail("rig JSON changed while the render was being prepared; retry from the new revision")
        if sha256_file(source_path) != manifest["masterHash"]:
            fail("source master changed while the render was being prepared; retry from the new revision")
        for target, backup in backups.items():
            if not target.exists() or sha256_file(target) != sha256_file(backup):
                fail(f"{target.name} changed while the render was being prepared")
        for target, existed in original_existence.items():
            if not existed and (target.exists() or target.is_symlink()):
                fail(f"{target.name} appeared while the render was being prepared")

        try:
            journal["state"] = "committing"
            atomic_write_text(journal_path, strict_json_dumps(journal, indent=2) + "\n")
            for staged, target in zip(staged_frames, output_paths):
                os.replace(staged, target)
            for stale in old_active_paths:
                if stale not in output_paths:
                    stale.unlink()
            fsync_directory(destination)
            os.replace(staged_spec, spec_path)
            fsync_directory(spec_path.parent)
            os.replace(staged_manifest, manifest_record)
            fsync_directory(metadata)
        except BaseException as error:
            preserve_transaction = True
            rollback_errors = []
            for target in reversed(mutation_targets):
                try:
                    backup = backups.get(target)
                    if backup is not None:
                        atomic_restore(backup, target)
                    elif not original_existence[target] and (target.exists() or target.is_symlink()):
                        if target.is_file() or target.is_symlink():
                            target.unlink()
                        else:
                            raise OSError(f"cannot remove unexpected rollback target {target}")
                except BaseException as rollback_error:
                    rollback_errors.append(f"{target.name}: {rollback_error}")
            try:
                fsync_directory(destination)
                fsync_directory(metadata)
                fsync_directory(spec_path.parent)
            except OSError as rollback_error:
                rollback_errors.append(f"directory sync: {rollback_error}")
            if rollback_errors:
                fail(
                    "render commit failed and rollback needs manual recovery from "
                    f"{transaction_dir.relative_to(workspace)}: {'; '.join(rollback_errors)}"
                )
            preserve_transaction = False
            fail(f"render commit failed before activation and was rolled back: {error}")

        journal["state"] = "committed"
        preserve_transaction = True
        try:
            atomic_write_text(journal_path, strict_json_dumps(journal, indent=2) + "\n")
        except BaseException:
            pass
        else:
            preserve_transaction = False
        if old_active_paths:
            preserve_transaction = True
            try:
                archive_parent = safe_output_path(
                    workspace,
                    Path(".sprite-studio") / "versions" / "rigs" / name,
                    "rig archive",
                )
                if archive_parent.exists() and not archive_parent.is_dir():
                    raise OSError("rig archive parent must be a directory")
                archive_parent.mkdir(parents=True, exist_ok=True)
                archive_name = (
                    datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%S%fZ-")
                    + transaction_dir.name.rsplit("-", 1)[-1]
                )
                archive = safe_output_path(
                    workspace,
                    archive_parent.relative_to(workspace) / archive_name,
                    "rig archive",
                )
                os.replace(frame_backups, archive)
                fsync_directory(archive_parent)
            except BaseException:
                pass
            else:
                preserve_transaction = False
    finally:
        try:
            if not remove_exact_tree(frame_stage):
                preserve_transaction = True
            if not preserve_transaction:
                remove_exact_tree(transaction_dir)
        except BaseException:
            pass

