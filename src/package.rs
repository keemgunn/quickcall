use std::fs;
use std::path::{Path, PathBuf};

use crate::errors::QcError;

/// Installed package layout resolved from a native binary under `dist/<triple>/`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageLayout {
    pub root: PathBuf,
    pub version: String,
    pub starter_config: PathBuf,
    pub bundled_settings: PathBuf,
    pub bundled_skills: PathBuf,
}

#[derive(Debug, serde::Deserialize)]
struct PackageManifest {
    version: String,
}

/// Resolve `<package>/dist/<triple>/<binary>` → package root.
///
/// No cwd search, ancestor walk past this exact layout, installed-global fallback,
/// or compiled-in Mother path. Callers pass the real executable path.
pub fn resolve_package(executable: &Path) -> Result<PackageLayout, QcError> {
    let real = executable.canonicalize().map_err(|cause| {
        QcError::new(format!(
            "cannot resolve executable '{}': {cause}",
            executable.display()
        ))
    })?;

    let binary_dir = real.parent().ok_or_else(|| {
        QcError::new(format!(
            "native executable '{}' is not under dist/<triple>/",
            real.display()
        ))
    })?;
    let dist_dir = binary_dir.parent().ok_or_else(|| {
        QcError::new(format!(
            "native executable '{}' is not under dist/<triple>/",
            real.display()
        ))
    })?;
    if dist_dir.file_name() != Some(std::ffi::OsStr::new("dist")) {
        return Err(QcError::new(format!(
            "native executable '{}' is not under dist/<triple>/",
            real.display()
        )));
    }
    let root = dist_dir.parent().ok_or_else(|| {
        QcError::new(format!(
            "native executable '{}' is not under dist/<triple>/",
            real.display()
        ))
    })?;

    let manifest_path = root.join("package.json");
    let bundled_settings = root.join("share").join("settings");
    let bundled_skills = root.join("share").join("skills");
    let starter_config = bundled_settings.join("config.toml");
    if !manifest_path.is_file() {
        return Err(QcError::new(format!(
            "package manifest missing: {}",
            manifest_path.display()
        )));
    }
    if !starter_config.is_file() {
        return Err(QcError::new(format!(
            "packaged starter config missing: {}",
            starter_config.display()
        )));
    }

    let manifest_source = fs::read_to_string(&manifest_path).map_err(|cause| {
        QcError::new(format!(
            "cannot read package manifest '{}': {cause}",
            manifest_path.display()
        ))
    })?;
    let manifest: PackageManifest = serde_json::from_str(&manifest_source).map_err(|cause| {
        QcError::new(format!(
            "invalid package manifest '{}': {cause}",
            manifest_path.display()
        ))
    })?;

    Ok(PackageLayout {
        root: root.to_path_buf(),
        version: manifest.version,
        starter_config,
        bundled_settings,
        bundled_skills,
    })
}
