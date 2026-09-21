use std::path::{Path, PathBuf};

use crate::errors::QcError;
use crate::fsutil::{copy_tree, ensure_dir, list_dir_names, path_exists, remove_tree};

pub const PRODUCT_SKILL_PREFIX: &str = "qc";

const SKILL_DEST_RELATIVE: [&str; 2] = [".agents/skills", ".claude/skills"];

pub fn skill_destinations(home: impl AsRef<Path>) -> Vec<PathBuf> {
    let home = home.as_ref();
    SKILL_DEST_RELATIVE
        .iter()
        .map(|relative| home.join(relative))
        .collect()
}

pub fn is_product_skill_name(name: &str) -> bool {
    name == PRODUCT_SKILL_PREFIX || name.starts_with(&format!("{PRODUCT_SKILL_PREFIX}-"))
}

fn dest_has_product_skills(dest: &Path) -> Result<bool, QcError> {
    for name in list_dir_names(dest)? {
        if is_product_skill_name(&name) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Clean-slate: delete dest dirs named `qc` or `qc-*`, then copy every packaged skill directory.
fn replace_install_dest(bundled_root: &Path, dest: &Path) -> Result<Vec<String>, QcError> {
    ensure_dir(dest)?;
    for name in list_dir_names(dest)? {
        if !is_product_skill_name(&name) {
            continue;
        }
        remove_tree(&dest.join(&name))?;
    }
    let mut copied = Vec::new();
    for name in list_dir_names(bundled_root)? {
        let destination = dest.join(&name);
        copy_tree(&bundled_root.join(&name), &destination)?;
        copied.push(destination.to_string_lossy().into_owned());
    }
    Ok(copied)
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HarnessInstallResult {
    pub copied: Vec<String>,
    pub skipped: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RefreshKnownResult {
    pub copied: Vec<String>,
    pub skipped: Vec<String>,
    pub known: bool,
}

pub fn install_agent_harness(
    home: impl AsRef<Path>,
    bundled_root: impl AsRef<Path>,
) -> Result<HarnessInstallResult, QcError> {
    let bundled_root = bundled_root.as_ref();
    if !path_exists(bundled_root)? {
        return Err(QcError::new(format!(
            "packaged skills missing: {}",
            bundled_root.display()
        )));
    }
    let mut copied = Vec::new();
    for dest in skill_destinations(home) {
        copied.extend(replace_install_dest(bundled_root, &dest)?);
    }
    Ok(HarnessInstallResult {
        copied,
        skipped: Vec::new(),
    })
}

/// Per dest: if it already contains `qc` or `qc-*`, wipe those dirs and copy the
/// full packaged set. Skip dests with no product skills.
pub fn refresh_known(
    home: impl AsRef<Path>,
    bundled_root: impl AsRef<Path>,
) -> Result<RefreshKnownResult, QcError> {
    let bundled_root = bundled_root.as_ref();
    if !path_exists(bundled_root)? {
        return Err(QcError::new(format!(
            "packaged skills missing: {}",
            bundled_root.display()
        )));
    }
    let mut copied = Vec::new();
    let mut skipped = Vec::new();
    for dest in skill_destinations(home) {
        if dest_has_product_skills(&dest)? {
            copied.extend(replace_install_dest(bundled_root, &dest)?);
        } else {
            skipped.push(dest.to_string_lossy().into_owned());
        }
    }
    if copied.is_empty() {
        return Ok(RefreshKnownResult {
            copied: Vec::new(),
            skipped: Vec::new(),
            known: false,
        });
    }
    Ok(RefreshKnownResult {
        copied,
        skipped,
        known: true,
    })
}
