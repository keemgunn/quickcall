use crate::args::OutputMode;
use crate::config::{Config, ToolDefaults};
use crate::errors::QcError;
use crate::prompt::PromptMeta;
use crate::tools::types::{AGY_EFFORT, CLAUDE_EFFORT, CODEX_EFFORT, ToolName};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FlagOverrides {
    pub tool: Option<String>,
    pub model: Option<String>,
    pub thinking: Option<String>,
    pub workdir: Option<String>,
    pub no_skills: Option<bool>,
    pub skill_path: Option<String>,
    pub output: Option<OutputMode>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedSettings {
    pub tool: ToolName,
    pub model: Option<String>,
    pub thinking: Option<String>,
    pub workdir: Option<String>,
    pub no_skills: Option<bool>,
    pub skill_path: Option<String>,
    pub command: String,
    pub output: OutputMode,
    pub warnings: Vec<String>,
}

fn tool_defaults(config: &Config, tool: ToolName) -> ToolDefaults {
    config
        .tool
        .tools
        .get(tool.as_str())
        .cloned()
        .unwrap_or_default()
}

fn pick_string(values: &[Option<&str>]) -> Option<String> {
    values
        .iter()
        .flatten()
        .next()
        .map(|text| (*text).to_string())
}

/// Flags > frontmatter > project/global merged `[tool.*]` > omit.
/// Inapplicable fields warn + ignore; unsupported effort values warn + ignore.
/// Workdir is flag/frontmatter only — mapping cwd is restored later by the CLI.
pub fn resolve_settings(
    flags: &FlagOverrides,
    meta: &PromptMeta,
    config: &Config,
    locked_tool: Option<ToolName>,
) -> Result<ResolvedSettings, QcError> {
    let mut warnings = Vec::new();
    let configured_default = config.tool.default.as_deref().unwrap_or("pi");
    // Explicit tool only from flags/frontmatter — config default must not fight a continued session.
    let explicit = flags.tool.as_deref().or(meta.tool.as_deref());
    if let Some(name) = explicit
        && ToolName::parse(name).is_none()
    {
        return Err(QcError::new(format!("unknown tool '{name}'")));
    }
    if let (Some(locked), Some(name)) = (locked_tool, explicit)
        && name != locked.as_str()
    {
        return Err(QcError::new(format!(
            "tool mismatch: session is '{locked}' but resolved tool is '{name}'"
        )));
    }
    let chosen = explicit.unwrap_or_else(|| {
        locked_tool
            .map(ToolName::as_str)
            .unwrap_or(configured_default)
    });
    let tool =
        ToolName::parse(chosen).ok_or_else(|| QcError::new(format!("unknown tool '{chosen}'")))?;
    let defaults = tool_defaults(config, tool);
    let command = defaults
        .command
        .clone()
        .unwrap_or_else(|| tool.default_command().to_string());

    let model = pick_string(&[
        flags.model.as_deref(),
        meta.model.as_deref(),
        defaults.default_model.as_deref(),
    ]);
    let mut thinking = pick_string(&[
        flags.thinking.as_deref(),
        meta.thinking.as_deref(),
        defaults.default_thinking.as_deref(),
    ]);
    let workdir = pick_string(&[flags.workdir.as_deref(), meta.workdir.as_deref()]);
    let mut no_skills = flags.no_skills.or(meta.no_skills);
    let mut skill_path = pick_string(&[flags.skill_path.as_deref(), meta.skill_path.as_deref()]);
    let output = flags.output.clone().unwrap_or(OutputMode::Text);

    // Cursor ignores thinking entirely (effort lives in the exact model slug).
    if tool == ToolName::Cursor && thinking.is_some() {
        warnings.push(
            "qc_thinking is ignored for tool 'cursor' (encode effort in the model slug)".into(),
        );
        thinking = None;
    }

    if tool == ToolName::Claude
        && let Some(level) = thinking.as_deref()
        && !CLAUDE_EFFORT.contains(&level)
    {
        warnings.push(format!(
            "qc_thinking '{level}' is unsupported for claude --effort; ignoring"
        ));
        thinking = None;
    }
    if tool == ToolName::Antigravity
        && let Some(level) = thinking.as_deref()
        && !AGY_EFFORT.contains(&level)
    {
        warnings.push(format!(
            "qc_thinking '{level}' is unsupported for antigravity --effort; ignoring"
        ));
        thinking = None;
    }
    if tool == ToolName::Codex
        && let Some(level) = thinking.as_deref()
        && !CODEX_EFFORT.contains(&level)
    {
        warnings.push(format!(
            "qc_thinking '{level}' is unsupported for codex; ignoring"
        ));
        thinking = None;
    }

    if tool != ToolName::Pi {
        if no_skills == Some(true) {
            warnings.push(format!("qc_no_skills is ignored for tool '{tool}'"));
            no_skills = None;
        }
        if skill_path.is_some() {
            warnings.push(format!("qc_skill_path is ignored for tool '{tool}'"));
            skill_path = None;
        }
    }

    // JS `noSkills || undefined`: false becomes omitted.
    let no_skills = no_skills.filter(|flag| *flag);

    Ok(ResolvedSettings {
        tool,
        model,
        thinking,
        workdir,
        no_skills,
        skill_path,
        command,
        output,
        warnings,
    })
}
