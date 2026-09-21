use std::fs;
use std::path::Path;

use quickcall::app::run;
use quickcall::tools::ToolName;
use quickcall::tools::dispatch::run_agent;
use quickcall::tools::types::AgentRequest;

use crate::common::{
    Scratch, args, dummy_installed, fixture_env, install_fixture_names, unique_scratch, write_file,
};

struct AppCtx {
    scratch: Scratch,
    exe: std::path::PathBuf,
}

fn app_ctx(label: &str) -> AppCtx {
    let scratch = unique_scratch(label);
    install_fixture_names(&scratch.bin());
    fs::create_dir_all(scratch.cwd().join(".qc/prompts")).expect("prompts");
    let exe = dummy_installed(&scratch);
    AppCtx { scratch, exe }
}

fn invoke(ctx: &AppCtx, argv: &[&str], extra: &[(&str, &str)]) -> quickcall::AppOutput {
    let mut env = fixture_env(&ctx.scratch);
    for (key, value) in extra {
        env.insert((*key).into(), (*value).into());
    }
    run(&args(argv), &ctx.scratch.cwd(), &env, &ctx.exe).expect("app::run")
}

fn invoke_err(ctx: &AppCtx, argv: &[&str], extra: &[(&str, &str)]) -> quickcall::QcError {
    let mut env = fixture_env(&ctx.scratch);
    for (key, value) in extra {
        env.insert((*key).into(), (*value).into());
    }
    run(&args(argv), &ctx.scratch.cwd(), &env, &ctx.exe).expect_err("qc error")
}

fn session_files(home: &Path) -> Vec<String> {
    let dir = home.join(".qc/sessions");
    let Ok(entries) = fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".json"))
        .collect();
    names.sort();
    names
}

fn record_json(ctx: &AppCtx) -> serde_json::Value {
    serde_json::from_slice(&fs::read(ctx.scratch.root.join("record.json")).expect("record"))
        .expect("record json")
}

fn request(scratch: &crate::common::Scratch, command: &str, session_id: &str) -> AgentRequest {
    let mut env = fixture_env(scratch);
    env.insert(
        "QC_RECORD".into(),
        scratch
            .root
            .join("record.json")
            .to_string_lossy()
            .into_owned(),
    );
    AgentRequest {
        prompt: "hello-turn".into(),
        model: None,
        thinking: None,
        workdir: scratch.cwd().to_string_lossy().into_owned(),
        session_id: session_id.into(),
        native_id: None,
        no_skills: None,
        skill_path: None,
        command: command.into(),
        env,
    }
}

#[test]
fn run_agent_pi_fixture_returns_pretty_id_as_native_id() {
    let scratch = unique_scratch("run-pi");
    install_fixture_names(&scratch.bin());
    let session_id = "260901-1200--pi--abcdef";
    let mut req = request(&scratch, "pi", session_id);
    req.env.insert("QC_PI_TEXT".into(), "pi-assistant".into());
    let result = run_agent(ToolName::Pi, &req).expect("pi");
    assert_eq!(result.text, "pi-assistant");
    assert_eq!(result.native_id, session_id);
    assert_eq!(result.exit, 0);
    assert!(result.error.is_none());
    let recorded: serde_json::Value =
        serde_json::from_slice(&fs::read(scratch.root.join("record.json")).expect("record"))
            .expect("json");
    assert_eq!(
        recorded["argv"],
        serde_json::json!(["--mode", "json", "-a", "--session-id", session_id])
    );
    assert_eq!(recorded["stdin"], "hello-turn");
}

#[test]
fn run_agent_claude_uses_generated_uuid_when_json_omits_session() {
    let scratch = unique_scratch("run-claude");
    install_fixture_names(&scratch.bin());
    let mut req = request(&scratch, "claude", "260901-1200--claude--abcdef");
    req.env
        .insert("QC_AGENT_TEXT".into(), "claude-assistant".into());
    let result = run_agent(ToolName::Claude, &req).expect("claude");
    assert_eq!(result.text, "claude-assistant");
    assert!(!result.native_id.is_empty());
    assert_eq!(result.exit, 0);
    let uuid = result
        .argv
        .windows(2)
        .find(|w| w[0] == "--session-id")
        .map(|w| w[1].clone())
        .expect("session-id flag");
    assert_eq!(result.native_id, uuid);
}

#[test]
fn run_agent_cursor_requires_observed_session_id() {
    let scratch = unique_scratch("run-cursor");
    install_fixture_names(&scratch.bin());
    let mut req = request(&scratch, "agent", "260901-1200--cursor--abcdef");
    req.env
        .insert("QC_AGENT_TEXT".into(), "cursor-assistant".into());
    let result = run_agent(ToolName::Cursor, &req).expect("cursor");
    assert_eq!(result.text, "cursor-assistant");
    assert_eq!(result.native_id, "11111111-2222-3333-4444-555555555555");
    assert!(result.error.is_none());

    req.env.insert("QC_AGENT_EMPTY_JSON".into(), "1".into());
    let empty = run_agent(ToolName::Cursor, &req).expect("empty");
    assert_eq!(empty.native_id, "");
    assert_eq!(
        empty.error.as_deref(),
        Some("cursor agent produced empty JSON output")
    );
}

#[test]
fn run_agent_opencode_does_not_infer_native_id_from_pretty_title() {
    let scratch = unique_scratch("run-opencode");
    install_fixture_names(&scratch.bin());
    let pretty = "260901-1200--opencode--abcdef";
    let mut req = request(&scratch, "opencode", pretty);
    req.env
        .insert("QC_AGENT_TEXT".into(), "opencode-assistant".into());
    let result = run_agent(ToolName::Opencode, &req).expect("opencode");
    assert_eq!(result.text, "opencode-assistant");
    assert_eq!(result.native_id, "ses_fixture_opencode");
    assert!(!result.argv.contains(&pretty.to_string()) || result.argv.contains(&"--title".into()));
    assert_ne!(result.native_id, pretty);
}

#[test]
fn run_agent_agy_requires_observed_conversation_id() {
    let scratch = unique_scratch("run-agy");
    install_fixture_names(&scratch.bin());
    let mut req = request(&scratch, "agy", "260901-1200--antigravity--abcdef");
    req.env
        .insert("QC_AGENT_TEXT".into(), "agy-assistant".into());
    let result = run_agent(ToolName::Antigravity, &req).expect("agy");
    assert_eq!(result.text, "agy-assistant");
    assert_eq!(result.native_id, "conv_fixture_agy");
    assert!(result.error.is_none());

    req.env.insert("QC_AGY_ERROR".into(), "agy failed".into());
    let failed = run_agent(ToolName::Antigravity, &req).expect("agy error");
    assert_eq!(failed.native_id, "");
    assert_eq!(failed.error.as_deref(), Some("agy failed"));
}

#[test]
fn run_agent_codex_uses_stdin_and_observed_thread_id() {
    let scratch = unique_scratch("run-codex");
    install_fixture_names(&scratch.bin());
    let pretty = "260901-1200--codex--abcdef";
    let mut req = request(&scratch, "codex", pretty);
    req.env
        .insert("QC_AGENT_TEXT".into(), "codex-assistant".into());
    let result = run_agent(ToolName::Codex, &req).expect("codex");
    assert_eq!(result.text, "codex-assistant");
    assert_eq!(result.native_id, "thread_fixture_codex");
    assert_ne!(result.native_id, pretty);
    assert!(result.error.is_none());
    assert!(result.usage.is_none());
    let recorded: serde_json::Value =
        serde_json::from_slice(&fs::read(scratch.root.join("record.json")).expect("record"))
            .expect("json");
    assert_eq!(recorded["stdin"], "hello-turn");
    let argv = recorded["argv"].as_array().expect("argv");
    assert_eq!(argv.last().and_then(|v| v.as_str()), Some("-"));
    assert!(argv.iter().any(|v| v == "exec"));
    assert!(argv.iter().any(|v| v == "--json"));
    assert!(
        argv.iter()
            .any(|v| v == "--dangerously-bypass-approvals-and-sandbox")
    );

    req.env.insert("QC_CODEX_USAGE".into(), "1".into());
    let with_usage = run_agent(ToolName::Codex, &req).expect("usage");
    let u = with_usage.usage.expect("usage present");
    assert_eq!(u.input_tokens, Some(1200.0));
    assert_eq!(u.output_tokens, Some(400.0));
    assert_eq!(u.cache_read_tokens, Some(50.0));
    assert_eq!(u.thinking_tokens, Some(10.0));
    assert_eq!(u.total_tokens, None);
    assert_eq!(u.cost, None);

    req.native_id = Some("thread_resume".into());
    req.env.remove("QC_CODEX_USAGE");
    let resumed = run_agent(ToolName::Codex, &req).expect("resume");
    assert_eq!(resumed.native_id, "thread_resume");
    let resume_record: serde_json::Value =
        serde_json::from_slice(&fs::read(scratch.root.join("record.json")).expect("record"))
            .expect("json");
    assert_eq!(resume_record["stdin"], "hello-turn");
    let resume_argv = resume_record["argv"].as_array().expect("argv");
    assert!(resume_argv.iter().any(|v| v == "resume"));
    assert!(resume_argv.iter().any(|v| v == "thread_resume"));
}

#[test]
fn run_agent_missing_binary_returns_qc_message() {
    let scratch = unique_scratch("run-missing");
    let req = AgentRequest {
        prompt: "x".into(),
        model: None,
        thinking: None,
        workdir: scratch.cwd().to_string_lossy().into_owned(),
        session_id: "260901-1200--pi--abcdef".into(),
        native_id: None,
        no_skills: None,
        skill_path: None,
        command: "pi".into(),
        env: {
            let mut env = std::collections::HashMap::new();
            env.insert("HOME".into(), scratch.home().to_string_lossy().into_owned());
            env.insert("PATH".into(), scratch.bin().display().to_string());
            env
        },
    };
    let err = run_agent(ToolName::Pi, &req).expect_err("missing");
    assert!(
        err.message.contains("pi executable 'pi' was not found"),
        "{}",
        err.message
    );
}

#[test]
fn wires_real_config_prompt_shell_expansion_and_fixture_pi() {
    let ctx = app_ctx("pipe-wire");
    let fixtures = crate::common::crate_root().join("tests/fixtures");
    fs::copy(
        fixtures.join("config/global.toml"),
        ctx.scratch.home().join(".qc/config.toml"),
    )
    .ok();
    fs::create_dir_all(ctx.scratch.home().join(".qc")).expect("home qc");
    fs::copy(
        fixtures.join("config/global.toml"),
        ctx.scratch.home().join(".qc/config.toml"),
    )
    .expect("global");
    fs::copy(
        fixtures.join("config/project.toml"),
        ctx.scratch.cwd().join(".qc/config.toml"),
    )
    .expect("project");
    let front = fs::read_to_string(fixtures.join("prompts/frontmatter.md")).expect("fm");
    let shell = fs::read_to_string(fixtures.join("prompts/shell-output.md")).expect("shell");
    write_file(
        &ctx.scratch.cwd().join(".qc/prompts/daily.md"),
        &front.replace("Body text", &shell.replace("First", "Body")),
    );
    let record = ctx.scratch.root.join("record.json");
    let out = invoke(
        &ctx,
        &["daily", "--append", "More !`printf append`"],
        &[
            ("QC_RECORD", record.to_str().unwrap()),
            ("QC_AGENT_TEXT", "done"),
        ],
    );
    assert_eq!(out.exit, 0);
    assert!(out.stdout.contains("done"));
    assert!(out.stdout.contains("[QC-SESSION]"));
    let recorded = record_json(&ctx);
    assert_eq!(
        recorded["stdin"],
        "Body one then two\n\n\n\n---\n\nAdditional Message from the user:\n\nMore append"
    );
    let argv = recorded["argv"].as_array().expect("argv");
    assert!(argv.iter().any(|v| v == "prompt/model"));
    assert!(argv.iter().any(|v| v == "high"));
}

#[test]
fn prints_quoted_success_envelope_and_saves_mapping() {
    let ctx = app_ctx("pipe-text");
    write_file(&ctx.scratch.home().join(".qc/config.toml"), "");
    write_file(&ctx.scratch.cwd().join(".qc/prompts/plain.md"), "Plain");
    let out = invoke(&ctx, &["plain"], &[("QC_AGENT_TEXT", "joke-text")]);
    assert_eq!(out.exit, 0);
    assert_eq!(out.stderr, "");
    let re = regex_lite_session(&out.stdout);
    assert!(
        out.stdout
            .starts_with("\"\"\"\njoke-text\n\"\"\"\n[SUCCESS] pi ⋅ "),
        "{}",
        out.stdout
    );
    assert!(out.stdout.contains("[QC-SESSION]"));
    let mapping: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(
            ctx.scratch
                .home()
                .join(".qc/sessions")
                .join(format!("{re}.json")),
        )
        .expect("mapping"),
    )
    .expect("json");
    assert_eq!(mapping["tool"], "pi");
    assert!(!mapping["native_id"].as_str().unwrap().is_empty());
    assert!(mapping.get("usage").is_none());
}

fn regex_lite_session(text: &str) -> String {
    let marker = "[QC-SESSION] ";
    let start = text.find(marker).expect("session") + marker.len();
    let end = text[start..]
        .find(|ch: char| ch.is_whitespace())
        .map(|i| start + i)
        .unwrap_or(text.len());
    text[start..end].to_string()
}

#[test]
fn quiet_json_and_debug_envelopes_match_typescript_rules() {
    let ctx = app_ctx("pipe-modes");
    write_file(&ctx.scratch.home().join(".qc/config.toml"), "");
    write_file(&ctx.scratch.cwd().join(".qc/prompts/plain.md"), "Plain");
    let quiet = invoke(&ctx, &["-q", "plain"], &[("QC_AGENT_TEXT", "joke-text")]);
    assert_eq!(quiet.exit, 0);
    assert_eq!(quiet.stderr, "");
    assert!(
        quiet
            .stdout
            .starts_with("\"\"\"\njoke-text\n\"\"\"\n[SUCCESS] pi ⋅ ")
    );

    let json = invoke(
        &ctx,
        &["plain", "--output", "json"],
        &[("QC_AGENT_TEXT", "text")],
    );
    assert_eq!(json.exit, 0);
    assert_eq!(json.stderr, "");
    let envelope: serde_json::Value = serde_json::from_str(&json.stdout).expect("json");
    assert_eq!(envelope["tool"], "pi");
    assert_eq!(envelope["warnings"], serde_json::json!([]));
    assert_eq!(envelope["exit"], 0);
    assert!(envelope["model"].is_null());
    assert!(envelope.get("usage").is_none());
    assert!(envelope.get("debug").is_none());

    let debug = invoke(
        &ctx,
        &["plain", "--output", "json", "--debug"],
        &[("QC_AGENT_TEXT", "text")],
    );
    let debug_env: serde_json::Value = serde_json::from_str(&debug.stdout).expect("debug json");
    let lines = debug_env["debug"].as_array().expect("debug array");
    assert!(
        lines
            .iter()
            .any(|line| line.as_str().unwrap().starts_with("tool="))
    );
    let joined = lines
        .iter()
        .filter_map(|v| v.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(!joined.contains("Plain"));
}

#[test]
fn usage_text_omits_zero_cost_json_keeps_zero() {
    let ctx = app_ctx("pipe-usage");
    write_file(&ctx.scratch.home().join(".qc/config.toml"), "");
    write_file(&ctx.scratch.cwd().join(".qc/prompts/plain.md"), "Plain");
    let text = invoke(
        &ctx,
        &["plain"],
        &[("QC_AGENT_TEXT", "joke-text"), ("QC_PI_USAGE", "1")],
    );
    assert!(
        text.stdout
            .contains("[USAGE] 1.2k in ⋅ 400 out ⋅ 50 cache ⋅ 10 think ⋅ 1.7k total ⋅ 0.012")
    );
    let session_id = regex_lite_session(&text.stdout);
    let mapping: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(
            ctx.scratch
                .home()
                .join(".qc/sessions")
                .join(format!("{session_id}.json")),
        )
        .expect("mapping"),
    )
    .expect("json");
    assert!(mapping.get("usage").is_none());

    let json = invoke(
        &ctx,
        &["plain", "--output", "json"],
        &[
            ("QC_AGENT_TEXT", "joke"),
            ("QC_PI_USAGE", "1"),
            ("QC_PI_USAGE_COST", "0"),
        ],
    );
    assert!(json.stdout.contains("\"cost\": 0"));
    let envelope: serde_json::Value = serde_json::from_str(&json.stdout).expect("json");
    assert_eq!(envelope["usage"]["cost"], 0);
    let text_zero = invoke(
        &ctx,
        &["plain"],
        &[
            ("QC_AGENT_TEXT", "ok"),
            ("QC_PI_USAGE", "1"),
            ("QC_PI_USAGE_COST", "0"),
        ],
    );
    assert!(
        text_zero
            .stdout
            .contains("[USAGE] 1.2k in ⋅ 400 out ⋅ 50 cache ⋅ 10 think ⋅ 1.7k total")
    );
    assert!(!text_zero.stdout.contains("$0"));
}

#[test]
fn valid_text_plus_nonzero_or_signal_is_success_not_exit_predicate() {
    let ctx = app_ctx("pipe-predicate");
    write_file(&ctx.scratch.home().join(".qc/config.toml"), "");
    write_file(&ctx.scratch.cwd().join(".qc/prompts/plain.md"), "Plain");
    let nonzero = invoke(
        &ctx,
        &["plain"],
        &[("QC_AGENT_TEXT", "kept-text"), ("QC_PI_EXIT", "7")],
    );
    assert_eq!(nonzero.exit, 7);
    assert!(nonzero.stdout.contains("kept-text"));
    assert!(nonzero.stdout.contains("[SUCCESS]"));
    assert_eq!(nonzero.stderr, "");

    let signalled = invoke(
        &ctx,
        &["plain"],
        &[
            ("QC_AGENT_TEXT", "kept-text"),
            ("QC_PI_SIGNAL_AFTER_TEXT", "SIGINT"),
        ],
    );
    assert_eq!(signalled.exit, 130);
    assert!(signalled.stdout.contains("kept-text"));
    assert!(signalled.stdout.contains("[SUCCESS]"));
    assert_eq!(signalled.stderr, "");
}

#[test]
fn empty_text_saves_mapping_then_prints_fail_envelope() {
    let ctx = app_ctx("pipe-fail-save");
    write_file(&ctx.scratch.home().join(".qc/config.toml"), "");
    write_file(&ctx.scratch.cwd().join(".qc/prompts/plain.md"), "Plain");
    let failed = invoke(&ctx, &["plain"], &[("QC_AGENT_TEXT", "")]);
    assert_eq!(failed.exit, 1);
    assert_eq!(failed.stdout, "");
    assert!(
        failed
            .stderr
            .starts_with("[ERROR] pi produced empty assistant text\n[QC-SESSION] ")
    );
    assert!(failed.stderr.contains("qc -o "));
    let session_id = regex_lite_session(&failed.stderr);
    let mapping: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(
            ctx.scratch
                .home()
                .join(".qc/sessions")
                .join(format!("{session_id}.json")),
        )
        .expect("mapping"),
    )
    .expect("json");
    assert_eq!(mapping["tool"], "pi");
    assert_eq!(mapping["native_id"], session_id);

    let quiet = invoke(&ctx, &["-q", "plain"], &[("QC_AGENT_TEXT", "")]);
    assert!(!quiet.stderr.contains("qc -o"));

    let resumed = invoke(
        &ctx,
        &["-c", &session_id, "--append", "retry"],
        &[("QC_AGENT_TEXT", "resumed")],
    );
    assert_eq!(resumed.exit, 0);
    assert!(resumed.stdout.contains("resumed"));
    assert!(
        resumed
            .stdout
            .contains(&format!("[QC-SESSION] {session_id}"))
    );
}

#[test]
fn missing_binary_and_preflight_create_no_session_file() {
    let ctx = app_ctx("pipe-preflight");
    write_file(&ctx.scratch.home().join(".qc/config.toml"), "");
    write_file(&ctx.scratch.cwd().join(".qc/prompts/plain.md"), "Plain");
    let missing = invoke(
        &ctx,
        &["plain"],
        &[("PATH", ctx.scratch.cwd().join("empty").to_str().unwrap())],
    );
    assert_eq!(missing.exit, 1);
    assert!(missing.stderr.contains("pi executable 'pi' was not found"));
    assert!(session_files(&ctx.scratch.home()).is_empty());

    let bad_prompt = invoke_err(&ctx, &["absent"], &[]);
    assert!(
        bad_prompt
            .message
            .contains("prompt alias 'absent' was not found")
    );
    assert!(session_files(&ctx.scratch.home()).is_empty());

    write_file(
        &ctx.scratch.home().join(".qc/config.toml"),
        "default-model = [",
    );
    let malformed = invoke_err(&ctx, &["plain"], &[]);
    assert!(
        malformed.message.contains("[ERROR]")
            || malformed.message.contains("TOML")
            || malformed.message.contains("invalid")
    );
    assert!(session_files(&ctx.scratch.home()).is_empty());
}

#[test]
fn parse_fail_does_not_bootstrap() {
    let ctx = app_ctx("pipe-parse");
    let err = invoke_err(&ctx, &["--wat"], &[]);
    assert_eq!(err.message, "unknown option '--wat'");
    // HOME/.qc from dummy layout resolve happens after parse — parse fails first.
    assert!(
        !ctx.scratch.home().join(".qc/config.toml").exists() || {
            // dummy_installed does not create ~/.qc; only repair would.
            !ctx.scratch.home().join(".qc/.gitignore").exists()
        }
    );
}

#[test]
fn continue_and_warning_accumulation_asymmetry() {
    let ctx = app_ctx("pipe-warn");
    write_file(&ctx.scratch.home().join(".qc/config.toml"), "");
    let id = "260901-1200--cursor--a1b2c3";
    let cwd = ctx.scratch.cwd().to_string_lossy().into_owned();
    write_file(
        &ctx.scratch
            .home()
            .join(".qc/sessions")
            .join(format!("{id}.json")),
        &format!(
            "{}\n",
            serde_json::to_string_pretty(&serde_json::json!({
                "tool": "cursor",
                "native_id": "native-cursor",
                "cwd": cwd,
                "created": "2026-09-01T00:00:00.000Z",
                "updated": "2026-09-01T00:00:00.000Z",
                "warnings": ["prior-warning"]
            }))
            .unwrap()
        ),
    );
    let current = "qc_thinking is ignored for tool 'cursor' (encode effort in the model slug)";
    let live = invoke(
        &ctx,
        &["-c", id, "-a", "more", "--thinking", "high"],
        &[("QC_AGENT_TEXT", "ok")],
    );
    assert_eq!(live.exit, 0);
    assert!(live.stderr.contains(&format!("[WARNING] {current}")));
    assert!(!live.stderr.contains("prior-warning"));
    assert!(!live.stdout.contains("[WARNING]"));
    let mapping: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(
            ctx.scratch
                .home()
                .join(".qc/sessions")
                .join(format!("{id}.json")),
        )
        .expect("mapping"),
    )
    .expect("json");
    assert_eq!(
        mapping["warnings"],
        serde_json::json!(["prior-warning", current])
    );

    write_file(
        &ctx.scratch
            .home()
            .join(".qc/sessions")
            .join(format!("{id}.json")),
        &format!(
            "{}\n",
            serde_json::to_string_pretty(&serde_json::json!({
                "tool": "cursor",
                "native_id": "native-cursor",
                "cwd": cwd,
                "created": "2026-09-01T00:00:00.000Z",
                "updated": "2026-09-01T00:00:00.000Z",
                "warnings": ["prior-warning"]
            }))
            .unwrap()
        ),
    );
    let json = invoke(
        &ctx,
        &[
            "-c",
            id,
            "-a",
            "more",
            "--thinking",
            "high",
            "--output",
            "json",
        ],
        &[("QC_AGENT_TEXT", "ok")],
    );
    let envelope: serde_json::Value = serde_json::from_str(&json.stdout).expect("json");
    assert_eq!(
        envelope["warnings"],
        serde_json::json!(["prior-warning", current])
    );
}

#[test]
fn open_updates_only_updated_and_prints_no_envelope() {
    let ctx = app_ctx("pipe-open");
    write_file(&ctx.scratch.home().join(".qc/config.toml"), "");
    let id = "260901-1000--pi--bbbbbb";
    let cwd = ctx.scratch.cwd().to_string_lossy().into_owned();
    write_file(
        &ctx.scratch
            .home()
            .join(".qc/sessions")
            .join(format!("{id}.json")),
        &format!(
            "{}\n",
            serde_json::to_string_pretty(&serde_json::json!({
                "tool": "pi",
                "native_id": "new-native",
                "cwd": cwd,
                "created": "2026-09-01T00:00:00.000Z",
                "updated": "2026-09-01T02:00:00.000Z",
                "warnings": []
            }))
            .unwrap()
        ),
    );
    let record = ctx.scratch.root.join("record.json");
    let opened = invoke(
        &ctx,
        &["-o", id],
        &[("QC_RECORD", record.to_str().unwrap())],
    );
    assert_eq!(opened.exit, 0);
    assert_eq!(opened.stdout, "");
    assert!(!opened.stderr.contains("[SUCCESS]"));
    assert!(!opened.stderr.contains("[ERROR]"));
    let recorded = record_json(&ctx);
    assert_eq!(
        recorded["argv"],
        serde_json::json!(["--session-id", "new-native"])
    );
    let mapping: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(
            ctx.scratch
                .home()
                .join(".qc/sessions")
                .join(format!("{id}.json")),
        )
        .expect("mapping"),
    )
    .expect("json");
    assert_eq!(mapping["created"], "2026-09-01T00:00:00.000Z");
    assert_ne!(mapping["updated"], "2026-09-01T02:00:00.000Z");
    assert_eq!(mapping["native_id"], "new-native");
}

#[test]
fn append_only_is_raw_and_cursor_empty_json_does_not_save() {
    let ctx = app_ctx("pipe-append");
    write_file(&ctx.scratch.home().join(".qc/config.toml"), "");
    let record = ctx.scratch.root.join("record.json");
    let out = invoke(
        &ctx,
        &["--append", "inspect this repo"],
        &[
            ("QC_RECORD", record.to_str().unwrap()),
            ("QC_AGENT_TEXT", "ok"),
        ],
    );
    assert_eq!(out.exit, 0);
    assert_eq!(record_json(&ctx)["stdin"], "inspect this repo");

    write_file(&ctx.scratch.cwd().join(".qc/prompts/plain.md"), "Plain");
    let empty = invoke(
        &ctx,
        &["plain", "--tool", "cursor"],
        &[
            ("QC_AGENT_EMPTY_JSON", "1"),
            (
                "QC_AGENT_STDERR",
                "Not logged in. Run `agent` to authenticate.\n",
            ),
        ],
    );
    assert_eq!(empty.exit, 1);
    assert_eq!(empty.stdout, "");
    assert!(
        empty
            .stderr
            .contains("cursor agent produced empty JSON output")
    );
    assert!(
        empty
            .stderr
            .contains("Not logged in. Run `agent` to authenticate.")
    );
    assert!(empty.stderr.contains("\nagent\n"));
    let cursor_sessions: Vec<_> = session_files(&ctx.scratch.home())
        .into_iter()
        .filter(|name| name.contains("--cursor--"))
        .collect();
    assert!(cursor_sessions.is_empty(), "{cursor_sessions:?}");
}

#[test]
fn live_warnings_on_stderr_debug_also_on_stdout() {
    let ctx = app_ctx("pipe-live-warn");
    write_file(&ctx.scratch.home().join(".qc/config.toml"), "");
    write_file(&ctx.scratch.cwd().join(".qc/prompts/plain.md"), "Plain");
    let live = invoke(
        &ctx,
        &["plain", "--tool", "cursor", "--thinking", "high"],
        &[("QC_AGENT_TEXT", "ok")],
    );
    assert!(
        live.stderr
            .contains("[WARNING] qc_thinking is ignored for tool 'cursor'")
    );
    assert!(!live.stdout.contains("[WARNING]"));

    let debug = invoke(
        &ctx,
        &["plain", "--tool", "cursor", "--thinking", "high", "--debug"],
        &[("QC_AGENT_TEXT", "ok")],
    );
    assert!(debug.stderr.contains("[WARNING]"));
    assert!(debug.stdout.contains("[WARNING]"));
    assert!(debug.stdout.contains("[DEBUG]"));
}

#[test]
fn shell_preflight_failure_creates_no_mapping() {
    let ctx = app_ctx("pipe-shell");
    write_file(&ctx.scratch.home().join(".qc/config.toml"), "");
    write_file(&ctx.scratch.cwd().join(".qc/prompts/plain.md"), "Plain");
    let missing = ctx.scratch.root.join("missing-shell");
    let err = invoke_err(&ctx, &["plain", "--shell", missing.to_str().unwrap()], &[]);
    assert!(
        err.message
            .contains("shell executable could not be started")
    );
    assert!(session_files(&ctx.scratch.home()).is_empty());
}

#[test]
fn wires_codex_create_fail_continue_open_usage_and_missing_binary() {
    let ctx = app_ctx("pipe-codex");
    write_file(&ctx.scratch.home().join(".qc/config.toml"), "");
    write_file(&ctx.scratch.cwd().join(".qc/prompts/plain.md"), "Plain");
    let record = ctx.scratch.root.join("record.json");

    let created = invoke(
        &ctx,
        &["plain", "--tool", "codex"],
        &[
            ("QC_RECORD", record.to_str().unwrap()),
            ("QC_AGENT_TEXT", "codex-ok"),
        ],
    );
    assert_eq!(created.exit, 0);
    assert!(created.stdout.contains("codex-ok"));
    assert!(created.stdout.contains("[SUCCESS] codex ⋅"));
    assert!(!created.stdout.contains("[USAGE]"));
    let created_id = regex_lite_session(&created.stdout);
    assert!(created_id.contains("--codex--"), "{created_id}");
    let created_record = record_json(&ctx);
    assert_eq!(created_record["stdin"], "Plain");
    assert_eq!(
        created_record["argv"].as_array().unwrap().last(),
        Some(&serde_json::json!("-"))
    );
    let mapping: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(
            ctx.scratch
                .home()
                .join(".qc/sessions")
                .join(format!("{created_id}.json")),
        )
        .expect("mapping"),
    )
    .expect("json");
    assert_eq!(mapping["tool"], "codex");
    assert_eq!(mapping["native_id"], "thread_fixture_codex");

    let usage = invoke(
        &ctx,
        &["plain", "--tool", "codex"],
        &[("QC_AGENT_TEXT", "with-usage"), ("QC_CODEX_USAGE", "1")],
    );
    assert!(
        usage
            .stdout
            .contains("[USAGE] 1.2k in ⋅ 400 out ⋅ 50 cache ⋅ 10 think")
    );
    assert!(!usage.stdout.contains("total"));

    let failed = invoke(
        &ctx,
        &["plain", "--tool", "codex"],
        &[("QC_CODEX_ERROR", "codex boom")],
    );
    assert_eq!(failed.exit, 1);
    assert_eq!(failed.stdout, "");
    assert!(failed.stderr.contains("[ERROR] codex boom"));
    let failed_id = regex_lite_session(&failed.stderr);
    let failed_mapping: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(
            ctx.scratch
                .home()
                .join(".qc/sessions")
                .join(format!("{failed_id}.json")),
        )
        .expect("failed mapping"),
    )
    .expect("json");
    assert_eq!(failed_mapping["native_id"], "thread_fixture_codex");

    let resumed = invoke(
        &ctx,
        &["-c", &failed_id, "--append", "retry"],
        &[
            ("QC_RECORD", record.to_str().unwrap()),
            ("QC_AGENT_TEXT", "codex-resumed"),
        ],
    );
    assert_eq!(resumed.exit, 0);
    assert!(resumed.stdout.contains("codex-resumed"));
    let resume_record = record_json(&ctx);
    assert_eq!(resume_record["stdin"], "retry");
    let resume_argv = resume_record["argv"].as_array().expect("argv");
    assert!(resume_argv.iter().any(|v| v == "resume"));
    assert!(resume_argv.iter().any(|v| v == "thread_fixture_codex"));

    let opened = invoke(
        &ctx,
        &["-o", &failed_id],
        &[("QC_RECORD", record.to_str().unwrap())],
    );
    assert_eq!(opened.exit, 0);
    assert_eq!(opened.stdout, "");
    assert_eq!(
        record_json(&ctx)["argv"],
        serde_json::json!(["resume", "thread_fixture_codex"])
    );

    let before_missing = session_files(&ctx.scratch.home());
    let missing = invoke(
        &ctx,
        &["plain", "--tool", "codex"],
        &[("PATH", ctx.scratch.cwd().join("empty").to_str().unwrap())],
    );
    assert_eq!(missing.exit, 1);
    assert!(
        missing
            .stderr
            .contains("codex executable 'codex' was not found")
    );
    assert_eq!(session_files(&ctx.scratch.home()), before_missing);
}
