use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;

use chrono::Utc;

use crate::args::{OpenArg, OutputMode, parse_args};
use crate::config::{
    BootstrapMode, Config, bootstrap_settings, config_paths, effective_shell, home_from_env,
    install_sample_prompts, load_config,
};
use crate::errors::QcError;
use crate::harness::install_agent_harness;
use crate::messages::{
    HELP, TextEnvelopeInput, debug_facts, empty_assistant_text, error, fail_output,
    inspect_command, interrupted, json_envelope, open_session_command, session_line, text_envelope,
    usage_fields,
};
use crate::package::resolve_package;
use crate::parsers::utc_millis;
use crate::progress::{CreateProgressOptions, create_progress, stderr_tty};
use crate::prompt::{PromptMeta, append_prompt, read_prompt};
use crate::session::{
    FoundSession, SessionMapping, find_latest_session, load_session, mint_session_id, save_session,
};
use crate::settings::{FlagOverrides, resolve_settings};
use crate::shell_output::expand_shell;
use crate::tools::dispatch::run_agent;
use crate::tools::spawn::spawn_interactive;
use crate::tools::tui::build_tui_args;
use crate::tools::types::{AgentRequest, is_tool_name};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit: i32,
}

fn empty_ok(exit: i32) -> AppOutput {
    AppOutput {
        stdout: String::new(),
        stderr: String::new(),
        exit,
    }
}

fn resolve_open_mapping(
    open: &OpenArg,
    tool_flag: Option<&str>,
    home: &Path,
    cwd: &str,
) -> Result<FoundSession, QcError> {
    if let Some(name) = tool_flag
        && !is_tool_name(name)
    {
        return Err(QcError::new(format!("unknown tool '{name}'")));
    }
    let tool = tool_flag.and_then(crate::tools::ToolName::parse);
    match open {
        OpenArg::Bare => find_latest_session(home, cwd, tool),
        OpenArg::Id(id) => {
            let mapping = load_session(home, id)?;
            if let Some(name) = tool_flag
                && name != mapping.tool.as_str()
            {
                return Err(QcError::new(format!(
                    "tool mismatch: session is '{}' but resolved tool is '{name}'",
                    mapping.tool
                )));
            }
            Ok(FoundSession {
                id: id.clone(),
                mapping,
            })
        }
    }
}

fn open_session(
    open: &OpenArg,
    tool_flag: Option<&str>,
    home: &Path,
    cwd: &str,
    config: &Config,
    environment: &HashMap<String, String>,
) -> Result<i32, QcError> {
    let FoundSession { id, mapping } = resolve_open_mapping(open, tool_flag, home, cwd)?;
    let command = config
        .tool
        .tools
        .get(mapping.tool.as_str())
        .and_then(|defaults| defaults.command.clone())
        .unwrap_or_else(|| mapping.tool.default_command().to_string());
    let args = build_tui_args(mapping.tool, &mapping.native_id)?;
    let home_owned = home.to_path_buf();
    let mapping_for_save = mapping.clone();
    spawn_interactive(
        &command,
        &args,
        &mapping.cwd,
        environment,
        mapping.tool.as_str(),
        Some(&|| {
            let mut updated = mapping_for_save.clone();
            updated.updated = utc_millis(Utc::now());
            save_session(&home_owned, &id, &updated)
        }),
    )
}

/// §3 application order. Tests must catch moving side effects earlier.
pub fn run(
    argv: &[String],
    cwd: &Path,
    env: &HashMap<String, String>,
    executable: &Path,
) -> Result<AppOutput, QcError> {
    let args = parse_args(argv)?;
    let home = home_from_env(env)
        .ok_or_else(|| QcError::new("HOME is required to locate qc configuration"))?;
    let package = resolve_package(executable)?;
    let paths = config_paths(&home, cwd, &package.starter_config);

    if args.install_sample_prompts {
        install_sample_prompts(&paths)?;
        return Ok(empty_ok(0));
    }

    if args.install_agent_harness {
        install_agent_harness(&home, &package.bundled_skills)?;
        return Ok(empty_ok(0));
    }

    bootstrap_settings(BootstrapMode::Repair, &paths)?;

    if args.help {
        return Ok(AppOutput {
            stdout: format!("{HELP}\n"),
            stderr: String::new(),
            exit: 0,
        });
    }
    if args.version {
        return Ok(AppOutput {
            stdout: format!("{}\n", package.version),
            stderr: String::new(),
            exit: 0,
        });
    }

    let config = load_config(&paths)?;
    let cwd_str = cwd.to_string_lossy().into_owned();

    if let Some(open) = &args.open {
        let exit = open_session(open, args.tool.as_deref(), &home, &cwd_str, &config, env)?;
        return Ok(empty_ok(exit));
    }

    let mut meta = PromptMeta::default();
    let mut body = String::new();
    if let Some(reference) = &args.prompt {
        let prompt = read_prompt(reference, cwd, &home)?;
        meta = prompt.meta;
        body = prompt.body;
    }

    let mut locked_tool = None;
    let mut native_id = None;
    let mut session_id = None;
    let mut session_cwd = None;
    let mut session_created = None;
    let mut prior_warnings: Vec<String> = Vec::new();

    if let Some(id) = &args.continue_id {
        let mapping = load_session(&home, id)?;
        locked_tool = Some(mapping.tool);
        native_id = Some(mapping.native_id);
        session_id = Some(id.clone());
        session_cwd = Some(mapping.cwd);
        session_created = Some(mapping.created);
        prior_warnings = mapping.warnings;
    }

    let settings = resolve_settings(
        &FlagOverrides {
            tool: args.tool.clone(),
            model: args.model.clone(),
            thinking: args.thinking.clone(),
            workdir: args.workdir.clone(),
            no_skills: args.no_skills.then_some(true),
            skill_path: args.skill.clone(),
            output: args.output.clone(),
        },
        &meta,
        &config,
        locked_tool,
    )?;

    let workdir = settings
        .workdir
        .clone()
        .or(session_cwd)
        .unwrap_or_else(|| cwd_str.clone());

    let raw_append = args.prompt.is_none();
    let complete = append_prompt(&body, args.append.as_deref(), raw_append);
    let expanded = expand_shell(
        &complete,
        &effective_shell(args.shell.as_deref(), &config, env),
        &config,
        Path::new(&workdir),
        env,
    )?;

    let pretty_id = session_id.unwrap_or_else(|| mint_session_id(settings.tool, None));
    let created = utc_millis(Utc::now());

    let live = settings.output == OutputMode::Text && !args.quiet;
    let progress = create_progress(CreateProgressOptions {
        live,
        tool: settings.tool.as_str().to_string(),
        model: settings.model.clone(),
        stream: None,
        is_tty: stderr_tty(),
        now_ms: None,
    });
    if live {
        for warning in &settings.warnings {
            progress.warn(warning);
        }
    }

    let started = Instant::now();
    progress.start();
    let agent = match run_agent(
        settings.tool,
        &AgentRequest {
            prompt: expanded.clone(),
            model: settings.model.clone(),
            thinking: settings.thinking.clone(),
            workdir: workdir.clone(),
            session_id: pretty_id.clone(),
            native_id,
            no_skills: settings.no_skills,
            skill_path: settings.skill_path.clone(),
            command: settings.command.clone(),
            env: env.clone(),
        },
    ) {
        Ok(result) => {
            progress.stop();
            result
        }
        Err(cause) => {
            progress.stop();
            let mut stderr = progress.captured();
            stderr.push_str(&format!("{}\n", error(&cause.message)));
            return Ok(AppOutput {
                stdout: String::new(),
                stderr,
                exit: cause.exit_code,
            });
        }
    };
    let duration_s = (started.elapsed().as_millis() as f64 / 1000.0).round() as i64;
    let failed =
        agent.error.as_deref().is_some_and(|text| !text.is_empty()) || agent.text.trim().is_empty();
    let warnings: Vec<String> = prior_warnings
        .iter()
        .cloned()
        .chain(settings.warnings.iter().cloned())
        .collect();

    if !agent.native_id.is_empty() {
        save_session(
            &home,
            &pretty_id,
            &SessionMapping {
                tool: settings.tool,
                native_id: agent.native_id.clone(),
                cwd: workdir.clone(),
                created: session_created.unwrap_or(created),
                updated: utc_millis(Utc::now()),
                warnings: warnings.clone(),
            },
        )?;
    }

    let mut stderr = progress.captured();
    if failed {
        let primary = agent
            .error
            .as_deref()
            .filter(|text| !text.is_empty())
            .map(str::to_string)
            .or_else(|| {
                if agent.exit >= 128 {
                    Some(interrupted(agent.exit - 128))
                } else {
                    None
                }
            })
            .filter(|text| !text.is_empty())
            .unwrap_or_else(|| empty_assistant_text(settings.tool.as_str()));
        stderr.push_str(&fail_output(&primary, &agent.stderr));
        if !agent.native_id.is_empty() {
            stderr.push_str(&format!("{}\n", session_line(&pretty_id)));
            if live {
                stderr.push_str(&format!("{}\n", open_session_command(&pretty_id)));
            }
        } else if live {
            stderr.push_str(&format!("{}\n", inspect_command(&settings.command)));
        }
        let exit = if agent.exit >= 128 {
            agent.exit
        } else if agent.exit == 0 {
            1
        } else {
            agent.exit
        };
        return Ok(AppOutput {
            stdout: String::new(),
            stderr,
            exit,
        });
    }

    let mut debug_lines = if args.debug {
        Some(debug_facts(
            settings.tool.as_str(),
            settings.model.as_deref(),
            &settings.command,
            &agent.argv,
            &expanded,
            &workdir,
            agent.exit,
            agent.stdout_bytes,
            &agent.stderr,
            &agent.native_id,
            duration_s,
        ))
    } else {
        None
    };
    if let Some(lines) = debug_lines.as_mut()
        && usage_fields(agent.usage.as_ref()).is_none()
    {
        lines.push("usage_absent".into());
    }

    let stdout = if settings.output == OutputMode::Json {
        json_envelope(
            settings.tool.as_str(),
            &pretty_id,
            &agent.native_id,
            &agent.result,
            &warnings,
            agent.exit,
            settings.model.as_deref(),
            duration_s,
            agent.usage.as_ref(),
            debug_lines.as_deref(),
        )
    } else {
        text_envelope(TextEnvelopeInput {
            text: &agent.text,
            tool: settings.tool.as_str(),
            duration_s,
            session_id: &pretty_id,
            model: settings.model.as_deref(),
            warnings: &settings.warnings,
            debug_lines: debug_lines.as_deref(),
            usage: agent.usage.as_ref(),
        })
    };

    Ok(AppOutput {
        stdout,
        stderr,
        exit: agent.exit,
    })
}
