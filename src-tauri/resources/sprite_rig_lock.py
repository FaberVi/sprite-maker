"""Fsync, atomic replace, and backup helpers for render commits."""

import errno
import hashlib
import os
import shutil
import tempfile
from pathlib import Path

from sprite_rig_json import fail

def ensure_regular_or_missing(path, label):
    if path.is_symlink():
        fail(f"{label} cannot be a symbolic link")
    if path.exists() and not path.is_file():
        fail(f"{label} must be a regular file")


def sha256_file(path):
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def fsync_path(path):
    # Windows FlushFileBuffers requires a writable handle; O_RDONLY yields EBADF.
    flags = os.O_RDWR if os.name == "nt" else os.O_RDONLY
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        if os.name == "nt" and error.errno in (errno.EACCES, errno.EPERM):
            return
        raise
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def copy_synced(source, destination):
    shutil.copy2(source, destination)
    fsync_path(destination)
    if sha256_file(source) != sha256_file(destination):
        raise OSError(f"backup verification failed for {source.name}")


def atomic_restore(backup, target):
    descriptor, temporary_name = tempfile.mkstemp(
        prefix=f".{target.name}.rollback-", suffix=".tmp", dir=str(target.parent),
    )
    os.close(descriptor)
    temporary = Path(temporary_name)
    try:
        copy_synced(backup, temporary)
        os.replace(temporary, target)
    finally:
        if temporary.exists() or temporary.is_symlink():
            temporary.unlink()



def fsync_directory(path):
    if os.name == "nt":
        return
    descriptor = os.open(path, os.O_RDONLY)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def stage_text(directory, prefix, content, mode=None):
    descriptor, temporary_name = tempfile.mkstemp(prefix=prefix, suffix=".tmp", dir=str(directory))
    temporary = Path(temporary_name)
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8", newline="\n") as handle:
            handle.write(content)
            handle.flush()
            os.fsync(handle.fileno())
        if mode is not None:
            os.chmod(temporary, mode & 0o777)
        return temporary
    except BaseException:
        if temporary.exists() or temporary.is_symlink():
            temporary.unlink()
        raise


def atomic_write_text(path, content, mode=None):
    temporary = stage_text(path.parent, f".{path.name}-", content, mode)
    try:
        os.replace(temporary, path)
        fsync_directory(path.parent)
    finally:
        if temporary.exists() or temporary.is_symlink():
            temporary.unlink()


def remove_exact_tree(path):
    try:
        if path is not None and path.exists() and path.is_dir() and not path.is_symlink():
            shutil.rmtree(path)
            return True
    except OSError:
        return False
    return True


