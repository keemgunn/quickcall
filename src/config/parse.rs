use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::errors::QcError;
use crate::messages::renamed_config_key;

pub const TOOL_NAMES: [&str; 6] = ["pi", "cursor", "claude", "opencode", "antigravity", "codex"];

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ToolDefaults {
    pub default_model: Option<String>,
    pub default_thinking: Option<String>,
    pub command: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ToolConfig {
    pub default: Option<String>,
    pub tools: BTreeMap<String, ToolDefaults>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Config {
    pub shell: Option<String>,
    pub tool: ToolConfig,
}

fn is_tool_name(value: &str) -> bool {
    TOOL_NAMES.contains(&value)
}

fn non_empty_string<'a>(value: &'a toml::Value, label: &str) -> Result<&'a str, QcError> {
    match value.as_str() {
        Some(text) if !text.is_empty() => Ok(text),
        _ => Err(QcError::new(format!("{label} must be a non-empty string"))),
    }
}

fn parse_tool_table(raw: &toml::Table, path: &str) -> Result<ToolConfig, QcError> {
    let mut result = ToolConfig::default();
    let allowed_top = [
        "default",
        "pi",
        "cursor",
        "claude",
        "opencode",
        "antigravity",
        "codex",
    ];
    for key in raw.keys() {
        if !allowed_top.contains(&key.as_str()) {
            return Err(QcError::new(format!(
                "unknown tool config key '{key}' in {path}"
            )));
        }
    }
    if let Some(default) = raw.get("default") {
        let text = non_empty_string(default, &format!("tool.default in {path}"))?;
        if !is_tool_name(text) {
            return Err(QcError::new(format!(
                "tool.default in {path} must be one of: pi, cursor, claude, opencode, antigravity, codex"
            )));
        }
        result.default = Some(text.to_string());
    }
    for name in TOOL_NAMES {
        let Some(table) = raw.get(name) else {
            continue;
        };
        let Some(entry) = table.as_table() else {
            return Err(QcError::new(format!(
                "tool.{name} in {path} must be a table"
            )));
        };
        let allowed = ["default_model", "default_thinking", "command"];
        for key in entry.keys() {
            if !allowed.contains(&key.as_str()) {
                return Err(QcError::new(format!(
                    "unknown tool.{name} key '{key}' in {path}"
                )));
            }
        }
        let text = |key: &str| -> Result<Option<String>, QcError> {
            match entry.get(key) {
                None => Ok(None),
                Some(value) => Ok(Some(
                    non_empty_string(value, &format!("tool.{name}.{key} in {path}"))?.to_string(),
                )),
            }
        };
        result.tools.insert(
            name.to_string(),
            ToolDefaults {
                default_model: text("default_model")?,
                default_thinking: text("default_thinking")?,
                command: text("command")?,
            },
        );
    }
    Ok(result)
}

/// Missing file → empty layer. A present malformed file is an error.
pub fn read_config(path: impl AsRef<Path>) -> Result<Config, QcError> {
    let path = path.as_ref();
    let path_display = path.display().to_string();
    match fs::metadata(path) {
        Ok(_) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Config::default());
        }
        Err(err) => {
            return Err(QcError::new(format!("cannot stat '{path_display}': {err}")));
        }
    }
    let source = fs::read_to_string(path)
        .map_err(|cause| QcError::new(format!("cannot read '{path_display}': {cause}")))?;
    let raw: toml::Table = source
        .parse()
        .map_err(|cause| QcError::new(format!("invalid TOML in {path_display}: {cause}")))?;

    if raw.contains_key("default-cli") {
        return Err(QcError::new(renamed_config_key(
            "default-cli",
            r#"[tool] default = "…""#,
        )));
    }
    if raw.contains_key("default-model") {
        return Err(QcError::new(renamed_config_key(
            "default-model",
            "[tool.<name>] default_model",
        )));
    }
    if raw.contains_key("default-thinking") {
        return Err(QcError::new(renamed_config_key(
            "default-thinking",
            "[tool.<name>] default_thinking",
        )));
    }

    // Leftover [command-permissions] stays in the allow-list and is ignored.
    let allowed = ["shell", "tool", "command-permissions"];
    for key in raw.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(QcError::new(format!(
                "unknown config key '{key}' in {path_display}"
            )));
        }
    }

    let shell = match raw.get("shell") {
        None => None,
        Some(value) => {
            Some(non_empty_string(value, &format!("shell in {path_display}"))?.to_string())
        }
    };

    let tool = match raw.get("tool") {
        None => ToolConfig::default(),
        Some(value) => {
            let Some(table) = value.as_table() else {
                return Err(QcError::new(format!(
                    "tool in {path_display} must be a table"
                )));
            };
            parse_tool_table(table, &path_display)?
        }
    };

    Ok(Config { shell, tool })
}
