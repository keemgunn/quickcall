use std::collections::HashMap;

use super::parse::{Config, TOOL_NAMES, ToolConfig, ToolDefaults, read_config};
use super::paths::ConfigPaths;
use crate::errors::QcError;

fn merge_tool_defaults(
    global: Option<&ToolDefaults>,
    project: Option<&ToolDefaults>,
) -> Option<ToolDefaults> {
    if global.is_none() && project.is_none() {
        return None;
    }
    Some(ToolDefaults {
        default_model: project
            .and_then(|value| value.default_model.clone())
            .or_else(|| global.and_then(|value| value.default_model.clone())),
        default_thinking: project
            .and_then(|value| value.default_thinking.clone())
            .or_else(|| global.and_then(|value| value.default_thinking.clone())),
        command: project
            .and_then(|value| value.command.clone())
            .or_else(|| global.and_then(|value| value.command.clone())),
    })
}

fn merge_tool_config(global: &ToolConfig, project: &ToolConfig) -> ToolConfig {
    let mut tools = std::collections::BTreeMap::new();
    for name in TOOL_NAMES {
        if let Some(merged) = merge_tool_defaults(global.tools.get(name), project.tools.get(name)) {
            tools.insert(name.to_string(), merged);
        }
    }
    ToolConfig {
        default: project.default.clone().or_else(|| global.default.clone()),
        tools,
    }
}

pub fn load_config(paths: &ConfigPaths) -> Result<Config, QcError> {
    let global = read_config(&paths.global)?;
    let project = read_config(&paths.project)?;
    Ok(Config {
        shell: project.shell.or(global.shell),
        tool: merge_tool_config(&global.tool, &project.tool),
    })
}

/// `--shell` → project `shell` → global `shell` → `$SHELL` → `/bin/sh`.
/// `??` semantics: empty `$SHELL` is used when the key is present.
pub fn effective_shell(
    cli_shell: Option<&str>,
    config: &Config,
    environment: &HashMap<String, String>,
) -> String {
    if let Some(shell) = cli_shell {
        return shell.to_string();
    }
    if let Some(shell) = &config.shell {
        return shell.clone();
    }
    if let Some(shell) = environment.get("SHELL") {
        return shell.clone();
    }
    "/bin/sh".to_string()
}
