use super::paths::ConfigPaths;
use crate::errors::QcError;
use crate::fsutil::{copy_tree, ensure_dir, path_exists, qc_io, remove_tree};
use std::fs;
use std::io::{self, Write};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootstrapMode {
    Refresh,
    Repair,
}

/// Paths under ~/.qc/ that packaged bootstrap writes into .gitignore.
pub const REFERENCE_GITIGNORE_ENTRIES: [&str; 2] = [".default-settings/", "sessions/"];

pub fn bootstrap_global(paths: &ConfigPaths) -> Result<(), QcError> {
    bootstrap_settings(BootstrapMode::Repair, paths)
}

fn refresh_default_settings(paths: &ConfigPaths) -> Result<(), QcError> {
    ensure_dir(&paths.global_root)?;
    remove_tree(&paths.default_settings)?;
    copy_tree(&paths.bundled_settings, &paths.default_settings)
}

fn refresh_gitignore(paths: &ConfigPaths) -> Result<(), QcError> {
    ensure_dir(&paths.global_root)?;
    let body = format!("{}\n", REFERENCE_GITIGNORE_ENTRIES.join("\n"));
    fs::write(&paths.gitignore, body).map_err(|cause| qc_io("write", &paths.gitignore, cause))
}

fn ensure_gitignore(paths: &ConfigPaths) -> Result<(), QcError> {
    if path_exists(&paths.gitignore)? {
        return Ok(());
    }
    refresh_gitignore(paths)
}

/// Create config.toml only when absent. Never overwrite.
fn seed_config(paths: &ConfigPaths) -> Result<(), QcError> {
    ensure_dir(&paths.global_root)?;
    if let Some(parent) = paths.global.parent() {
        ensure_dir(parent)?;
    }
    let mut file = match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&paths.global)
    {
        Ok(file) => file,
        Err(err) if err.kind() == io::ErrorKind::AlreadyExists => return Ok(()),
        Err(err) => return Err(qc_io("create", &paths.global, err)),
    };
    let source = fs::read(&paths.starter).map_err(|cause| qc_io("read", &paths.starter, cause))?;
    file.write_all(&source)
        .map_err(|cause| qc_io("write", &paths.global, cause))?;
    Ok(())
}

/// Wipe and recopy packaged sample prompts into ~/.qc/prompts/samples/.
pub fn hard_refresh_sample_prompts(paths: &ConfigPaths) -> Result<(), QcError> {
    ensure_dir(&paths.global_root)?;
    if let Some(parent) = paths.sample_prompts.parent() {
        ensure_dir(parent)?;
    }
    remove_tree(&paths.sample_prompts)?;
    copy_tree(&paths.starter_sample_prompts, &paths.sample_prompts)
}

/// CLI flag: refresh samples only; does not create config.toml.
pub fn install_sample_prompts(paths: &ConfigPaths) -> Result<(), QcError> {
    hard_refresh_sample_prompts(paths)
}

pub fn bootstrap_settings(mode: BootstrapMode, paths: &ConfigPaths) -> Result<(), QcError> {
    if mode == BootstrapMode::Refresh {
        refresh_default_settings(paths)?;
        refresh_gitignore(paths)?;
    } else if !path_exists(&paths.default_settings)? {
        refresh_default_settings(paths)?;
        ensure_gitignore(paths)?;
    } else {
        ensure_gitignore(paths)?;
    }

    if !path_exists(&paths.global)? {
        seed_config(paths)?;
        hard_refresh_sample_prompts(paths)?;
    }
    Ok(())
}
