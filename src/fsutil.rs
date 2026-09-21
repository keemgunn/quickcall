use std::fs;
use std::io;
use std::path::Path;

use crate::errors::QcError;

pub(crate) fn qc_io(op: &str, path: &Path, cause: io::Error) -> QcError {
    QcError::new(format!("{op} '{}': {cause}", path.display()))
}

/// Follows the path, matching Node `access(F_OK)` / `stat`.
pub(crate) fn path_exists(path: &Path) -> Result<bool, QcError> {
    match fs::metadata(path) {
        Ok(_) => Ok(true),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(err) => Err(qc_io("stat", path, err)),
    }
}

pub(crate) fn ensure_dir(path: &Path) -> Result<(), QcError> {
    fs::create_dir_all(path).map_err(|cause| qc_io("mkdir", path, cause))
}

pub(crate) fn remove_tree(path: &Path) -> Result<(), QcError> {
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(qc_io("rm", path, err)),
    }
}

/// Copy `src` onto `dest` (dest becomes a directory with src's children).
pub(crate) fn copy_tree(src: &Path, dest: &Path) -> Result<(), QcError> {
    ensure_dir(dest)?;
    let entries = fs::read_dir(src).map_err(|cause| qc_io("readdir", src, cause))?;
    for entry in entries {
        let entry = entry.map_err(|cause| qc_io("readdir", src, cause))?;
        let from = entry.path();
        let to = dest.join(entry.file_name());
        let file_type = entry
            .file_type()
            .map_err(|cause| qc_io("stat", &from, cause))?;
        if file_type.is_dir() {
            copy_tree(&from, &to)?;
        } else {
            fs::copy(&from, &to).map_err(|cause| qc_io("copy", &from, cause))?;
        }
    }
    Ok(())
}

/// Directory names only. Uses `DirEntry::file_type` (lstat), so a symlink is not a directory.
pub(crate) fn list_dir_names(path: &Path) -> Result<Vec<String>, QcError> {
    if !path_exists(path)? {
        return Ok(Vec::new());
    }
    let entries = fs::read_dir(path).map_err(|cause| qc_io("readdir", path, cause))?;
    let mut names = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|cause| qc_io("readdir", path, cause))?;
        let file_type = entry
            .file_type()
            .map_err(|cause| qc_io("stat", &entry.path(), cause))?;
        if file_type.is_dir() {
            names.push(entry.file_name().to_string_lossy().into_owned());
        }
    }
    Ok(names)
}
