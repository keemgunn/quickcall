use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::errors::QcError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigPaths {
    pub global: PathBuf,
    pub project: PathBuf,
    pub starter: PathBuf,
    pub starter_prompts: PathBuf,
    pub global_root: PathBuf,
    pub default_settings: PathBuf,
    pub gitignore: PathBuf,
    pub sample_prompts: PathBuf,
    pub starter_sample_prompts: PathBuf,
    pub bundled_settings: PathBuf,
}

/// Resolve every global and starter path qc bootstrap reads or writes.
pub fn config_paths(
    home: impl AsRef<Path>,
    cwd: impl AsRef<Path>,
    starter: impl AsRef<Path>,
) -> ConfigPaths {
    let home = home.as_ref();
    let cwd = cwd.as_ref();
    let starter = starter.as_ref().to_path_buf();
    let global_root = home.join(".qc");
    let bundled_settings = starter
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    ConfigPaths {
        global: global_root.join("config.toml"),
        project: cwd.join(".qc").join("config.toml"),
        starter_prompts: bundled_settings.join("prompts"),
        starter_sample_prompts: bundled_settings.join("prompts").join("samples"),
        default_settings: global_root.join(".default-settings"),
        gitignore: global_root.join(".gitignore"),
        sample_prompts: global_root.join("prompts").join("samples"),
        bundled_settings,
        starter,
        global_root,
    }
}

/// Public `qc` HOME: missing or empty is absent. Empty string is falsy like TypeScript `if (!home)`.
pub fn home_from_env(env: &HashMap<String, String>) -> Option<PathBuf> {
    env.get("HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

/// Bootstrap helper: `HOME ?? fallback`, matching Node `process.env.HOME ?? homedir()`.
/// Empty HOME is kept (`??` does not skip `""`); tests inject `fallback` instead of touching the real homedir.
pub fn resolve_bootstrap_home(
    env: &HashMap<String, String>,
    fallback: impl FnOnce() -> Option<PathBuf>,
) -> Result<PathBuf, QcError> {
    if let Some(home) = env.get("HOME") {
        return Ok(PathBuf::from(home));
    }
    fallback().ok_or_else(|| QcError::new("HOME is required to locate qc configuration"))
}

/// Passwd home when `HOME` is unset. Production `qc-bootstrap` only; tests pass a controlled fallback.
pub fn fallback_home() -> Option<PathBuf> {
    #[cfg(unix)]
    {
        // SAFETY: getpwuid returns libc storage; we copy the C string before any later libc call.
        unsafe {
            let pwd = libc::getpwuid(libc::getuid());
            if pwd.is_null() {
                return None;
            }
            let dir = (*pwd).pw_dir;
            if dir.is_null() {
                return None;
            }
            let cstr = std::ffi::CStr::from_ptr(dir);
            let value = cstr.to_string_lossy();
            if value.is_empty() {
                None
            } else {
                Some(PathBuf::from(value.as_ref()))
            }
        }
    }
    #[cfg(not(unix))]
    {
        None
    }
}
