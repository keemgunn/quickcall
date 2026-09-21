use std::collections::BTreeMap;

use quickcall::args::OutputMode;
use quickcall::config::{Config, ToolConfig, ToolDefaults};
use quickcall::prompt::PromptMeta;
use quickcall::settings::{FlagOverrides, resolve_settings};
use quickcall::tools::ToolName;

fn base() -> Config {
    Config {
        shell: None,
        tool: ToolConfig {
            default: Some("pi".into()),
            tools: BTreeMap::from([
                (
                    "pi".into(),
                    ToolDefaults {
                        default_model: Some("pi/global".into()),
                        default_thinking: Some("medium".into()),
                        command: None,
                    },
                ),
                (
                    "cursor".into(),
                    ToolDefaults {
                        default_model: Some("composer-2.5".into()),
                        default_thinking: None,
                        command: None,
                    },
                ),
                (
                    "claude".into(),
                    ToolDefaults {
                        default_model: Some("sonnet".into()),
                        default_thinking: Some("high".into()),
                        command: None,
                    },
                ),
            ]),
        },
    }
}

#[test]
fn applies_flag_frontmatter_config_precedence() {
    let resolved = resolve_settings(
        &FlagOverrides {
            tool: Some("claude".into()),
            model: Some("opus".into()),
            ..FlagOverrides::default()
        },
        &PromptMeta {
            tool: Some("cursor".into()),
            model: Some("ignored".into()),
            thinking: Some("low".into()),
            ..PromptMeta::default()
        },
        &base(),
        None,
    )
    .expect("resolve");
    assert_eq!(resolved.tool, ToolName::Claude);
    assert_eq!(resolved.model.as_deref(), Some("opus"));
    assert_eq!(resolved.thinking.as_deref(), Some("low"));
    assert_eq!(resolved.command, "claude");
}

#[test]
fn warns_and_ignores_inapplicable_fields() {
    let cursor = resolve_settings(
        &FlagOverrides {
            thinking: Some("high".into()),
            no_skills: Some(true),
            skill_path: Some("/s".into()),
            ..FlagOverrides::default()
        },
        &PromptMeta {
            tool: Some("cursor".into()),
            ..PromptMeta::default()
        },
        &base(),
        None,
    )
    .expect("cursor");
    assert_eq!(cursor.thinking, None);
    assert_eq!(cursor.no_skills, None);
    assert_eq!(cursor.skill_path, None);
    assert!(cursor.warnings.iter().any(|w| w.contains("qc_thinking")));
    assert!(cursor.warnings.iter().any(|w| w.contains("qc_no_skills")));

    let claude = resolve_settings(
        &FlagOverrides {
            thinking: Some("off".into()),
            no_skills: Some(true),
            ..FlagOverrides::default()
        },
        &PromptMeta {
            tool: Some("claude".into()),
            ..PromptMeta::default()
        },
        &base(),
        None,
    )
    .expect("claude");
    assert_eq!(claude.thinking, None);
    assert!(
        claude
            .warnings
            .iter()
            .any(|w| w.contains("unsupported for claude"))
    );

    let agy = resolve_settings(
        &FlagOverrides {
            thinking: Some("xhigh".into()),
            ..FlagOverrides::default()
        },
        &PromptMeta {
            tool: Some("antigravity".into()),
            ..PromptMeta::default()
        },
        &base(),
        None,
    )
    .expect("agy");
    assert_eq!(agy.thinking, None);
    assert!(agy.warnings.iter().any(|w| w.contains("antigravity")));

    let codex_bad = resolve_settings(
        &FlagOverrides {
            thinking: Some("none".into()),
            no_skills: Some(true),
            skill_path: Some("/s".into()),
            ..FlagOverrides::default()
        },
        &PromptMeta {
            tool: Some("codex".into()),
            ..PromptMeta::default()
        },
        &base(),
        None,
    )
    .expect("codex");
    assert_eq!(codex_bad.thinking, None);
    assert_eq!(codex_bad.no_skills, None);
    assert_eq!(codex_bad.skill_path, None);
    assert!(
        codex_bad
            .warnings
            .iter()
            .any(|w| w.contains("unsupported for codex"))
    );
    assert!(
        codex_bad
            .warnings
            .iter()
            .any(|w| w.contains("qc_no_skills"))
    );

    let codex_off = resolve_settings(
        &FlagOverrides {
            thinking: Some("off".into()),
            ..FlagOverrides::default()
        },
        &PromptMeta {
            tool: Some("codex".into()),
            ..PromptMeta::default()
        },
        &base(),
        None,
    )
    .expect("codex off");
    assert_eq!(codex_off.thinking.as_deref(), Some("off"));
    assert_eq!(codex_off.command, "codex");
}

#[test]
fn locks_tool_from_session_mapping_and_errors_on_mismatch() {
    let flag_mismatch = resolve_settings(
        &FlagOverrides {
            tool: Some("claude".into()),
            ..FlagOverrides::default()
        },
        &PromptMeta::default(),
        &base(),
        Some(ToolName::Pi),
    )
    .expect_err("flag mismatch");
    assert!(flag_mismatch.message.contains("tool mismatch"));

    let meta_mismatch = resolve_settings(
        &FlagOverrides::default(),
        &PromptMeta {
            tool: Some("claude".into()),
            ..PromptMeta::default()
        },
        &base(),
        Some(ToolName::Pi),
    )
    .expect_err("meta mismatch");
    assert!(meta_mismatch.message.contains("tool mismatch"));

    let continued = resolve_settings(
        &FlagOverrides::default(),
        &PromptMeta::default(),
        &base(),
        Some(ToolName::Cursor),
    )
    .expect("config default does not mismatch");
    assert_eq!(continued.tool, ToolName::Cursor);
    assert_eq!(continued.command, "agent");
}

#[test]
fn defaults_tool_to_pi_and_uses_per_tool_command_overrides() {
    let mut config = base();
    config.tool.tools.insert(
        "pi".into(),
        ToolDefaults {
            command: Some("custom-pi".into()),
            ..ToolDefaults::default()
        },
    );
    assert_eq!(
        resolve_settings(
            &FlagOverrides::default(),
            &PromptMeta::default(),
            &config,
            None
        )
        .expect("custom")
        .command,
        "custom-pi"
    );
    assert_eq!(
        resolve_settings(
            &FlagOverrides {
                tool: Some("cursor".into()),
                ..FlagOverrides::default()
            },
            &PromptMeta::default(),
            &base(),
            None
        )
        .expect("agent")
        .command,
        "agent"
    );
    assert_eq!(
        resolve_settings(
            &FlagOverrides {
                tool: Some("antigravity".into()),
                ..FlagOverrides::default()
            },
            &PromptMeta::default(),
            &base(),
            None
        )
        .expect("agy")
        .command,
        "agy"
    );
    assert_eq!(
        resolve_settings(
            &FlagOverrides {
                tool: Some("codex".into()),
                ..FlagOverrides::default()
            },
            &PromptMeta::default(),
            &base(),
            None
        )
        .expect("codex")
        .command,
        "codex"
    );
}

#[test]
fn explicit_false_no_skills_is_omitted_and_empty_model_stays_defined() {
    let omitted = resolve_settings(
        &FlagOverrides {
            no_skills: Some(false),
            ..FlagOverrides::default()
        },
        &PromptMeta::default(),
        &base(),
        None,
    )
    .expect("false");
    assert_eq!(omitted.no_skills, None);
    assert_eq!(omitted.tool, ToolName::Pi);
    assert_eq!(omitted.output, OutputMode::Text);

    let empty_model = resolve_settings(
        &FlagOverrides {
            model: Some(String::new()),
            ..FlagOverrides::default()
        },
        &PromptMeta::default(),
        &base(),
        None,
    )
    .expect("empty");
    assert_eq!(empty_model.model.as_deref(), Some(""));

    let unknown = resolve_settings(
        &FlagOverrides {
            tool: Some("nope".into()),
            ..FlagOverrides::default()
        },
        &PromptMeta::default(),
        &base(),
        None,
    )
    .expect_err("unknown");
    assert_eq!(unknown.message, "unknown tool 'nope'");
}
