use quickcall::messages::{
    HELP, TextEnvelopeInput, compact_tokens, debug_line, elapsed_label, empty_assistant_text,
    error, fail_output, inspect_command, interrupted, missing_binary, open_session_command,
    quoted_text, renamed_config_key, renamed_frontmatter_key, session_line, success_line,
    text_envelope, usage_fields, usage_line, wait_line, warn,
};
use quickcall::tools::types::AgentUsage;

/// Independent expected help body, copied from TypeScript `src/messages.ts`.
const TS_HELP: &str = "\
Usage: qc [<prompt-reference>] [options]
       qc -o|--open [id] [--tool <name>]
       qc --install-sample-prompts
       qc --install-agent-harness

Run a Markdown prompt through a headless agent tool (default: pi).

Options:
  --tool <name>        Agent tool: pi, cursor, claude, opencode, antigravity, codex
  --model <id>         Model / slug for the selected tool
  --thinking <level>   Shared thinking knob (tool-specific mapping)
  --workdir <path>     Working directory for shell expansion and agent spawn
  --output <mode>      text (default) or json
  --debug              Include [WARNING] and [DEBUG] lines on the stdout envelope
  -q, --quiet           Print only the final envelope (no spinner)
  -c, --continue <id>  Resume a qc session by pretty id
  -o, --open [id]      Open a stored session in the provider's native TUI
  --skill <path>       Pi-only skill path
  --no-skills          Pi-only: disable skills
  -s, --shell <path>   Shell used only for !`command` substitutions
  -a, --append <text>  Append user text, or the whole turn when no prompt is given
      --install-sample-prompts
                       Replace ~/.qc/prompts/samples/ from packaged defaults
      --install-agent-harness
                       Replace-install qc skills to ~/.agents/skills and ~/.claude/skills
  -h, --help           Show this help
  -v, --version        Show the qc version

Prompt reference is optional when -a/--append is set. -o/--open does not take a prompt.";

#[test]
fn help_matches_typescript_messages_help() {
    assert_eq!(HELP, TS_HELP);
}

#[test]
fn centralizes_manifest_derived_user_facing_output() {
    assert!(HELP.contains("Usage: qc"));
    assert!(HELP.contains("--tool"));
    assert!(HELP.contains("--continue"));
    assert!(HELP.contains("-o, --open"));
    assert!(HELP.contains("-s, --shell"));
    assert!(HELP.contains("-a, --append"));
    assert!(HELP.contains("--debug"));
    assert!(HELP.contains("-q, --quiet"));
    assert!(HELP.contains("or the whole turn when no prompt is given"));
    assert!(HELP.contains("Prompt reference is optional when -a/--append is set"));
    assert!(HELP.contains("--install-sample-prompts"));
    assert!(HELP.contains("--install-agent-harness"));
    assert!(HELP.contains("pi, cursor, claude, opencode, antigravity, codex"));
    assert_eq!(error("bad"), "[ERROR] bad");
    assert_eq!(warn("w"), "[WARNING] w");
    assert_eq!(debug_line("spawn cwd=/tmp"), "[DEBUG] spawn cwd=/tmp");
    assert_eq!(session_line("id"), "[QC-SESSION] id");
    assert!(missing_binary("pi", "pi").contains("pi"));
    assert_eq!(
        empty_assistant_text("opencode"),
        "opencode produced empty assistant text"
    );
    assert_eq!(interrupted(2), "interrupted (signal 2)");
}

#[test]
fn error_prefix_matches_typescript() {
    assert_eq!(error("bad"), "[ERROR] bad");
}

#[test]
fn renamed_config_key_matches_typescript() {
    assert_eq!(
        renamed_config_key("default-cli", r#"[tool] default = "…""#),
        r#"config key 'default-cli' was removed; use [tool] default = "…""#
    );
}

#[test]
fn renamed_frontmatter_key_matches_typescript() {
    assert_eq!(
        renamed_frontmatter_key("qc_cli", "qc_tool"),
        "frontmatter key 'qc_cli' was removed; use qc_tool"
    );
}

#[test]
fn pads_elapsed_seconds_to_at_least_three_digits() {
    assert_eq!(elapsed_label(0), "000s");
    assert_eq!(elapsed_label(12), "012s");
    assert_eq!(elapsed_label(999), "999s");
    assert_eq!(elapsed_label(1000), "1000s");
}

#[test]
fn quotes_assistant_text_and_formats_success_with_optional_model() {
    assert_eq!(
        quoted_text("Why do programmers prefer dark mode?"),
        "\"\"\"\nWhy do programmers prefer dark mode?\n\"\"\""
    );
    assert_eq!(
        success_line("antigravity", 50, Some("gemini-3.8-flash-high")),
        "[SUCCESS] antigravity ⋅ gemini-3.8-flash-high ⋅ 50s"
    );
    assert_eq!(success_line("pi", 12, None), "[SUCCESS] pi ⋅ 12s");
}

#[test]
fn formats_the_live_wait_line_with_tool_optional_model_and_padded_elapsed() {
    assert_eq!(
        wait_line("cursor", 4, Some("composer-2.5")),
        " ⋅ cursor ⋅ composer-2.5 ⋅ 004s"
    );
    assert_eq!(wait_line("pi", 12, None), " ⋅ pi ⋅ 012s");
    assert_eq!(wait_line("pi", 0, None), " ⋅ pi ⋅ 000s");
}

#[test]
fn formats_a_copy_paste_open_command_for_a_saved_pretty_id() {
    assert_eq!(
        open_session_command("260905-1545--pi--a1b2c3"),
        "qc -o 260905-1545--pi--a1b2c3"
    );
}

#[test]
fn appends_child_stderr_to_the_fail_body_when_it_adds_information() {
    assert_eq!(
        fail_output("cursor agent produced empty JSON output", ""),
        "[ERROR] cursor agent produced empty JSON output\n"
    );
    assert_eq!(
        fail_output(
            "cursor agent produced empty JSON output",
            "Not logged in.\n"
        ),
        "[ERROR] cursor agent produced empty JSON output\nNot logged in.\n"
    );
    assert_eq!(
        fail_output("agy refused the model", "agy refused the model\n"),
        "[ERROR] agy refused the model\n"
    );
}

#[test]
fn formats_a_copy_paste_native_inspect_command() {
    assert_eq!(inspect_command("agent"), "agent");
    assert_eq!(inspect_command("/opt/bin/agent"), "/opt/bin/agent");
    assert_eq!(inspect_command("/opt/my agent"), "'/opt/my agent'");
}

#[test]
fn builds_a_text_envelope_with_warnings_and_debug_only_when_requested() {
    let base = text_envelope(TextEnvelopeInput {
        text: "joke",
        tool: "pi",
        duration_s: 12,
        session_id: "260906-1245--pi--abcdef",
        model: None,
        warnings: &[],
        debug_lines: None,
        usage: None,
    });
    assert_eq!(
        base,
        [
            "\"\"\"",
            "joke",
            "\"\"\"",
            "[SUCCESS] pi ⋅ 12s",
            "[QC-SESSION] 260906-1245--pi--abcdef",
            "",
        ]
        .join("\n")
    );
    assert!(!base.contains("[WARNING]"));
    assert!(!base.contains("[DEBUG]"));

    let debug = text_envelope(TextEnvelopeInput {
        text: "joke",
        tool: "pi",
        duration_s: 12,
        session_id: "260906-1245--pi--abcdef",
        model: Some("provider/model"),
        warnings: &["qc_no_skills is ignored for tool 'opencode'".into()],
        debug_lines: Some(&[
            "tool=pi".into(),
            "child_stderr:\nNo project session found".into(),
        ]),
        usage: None,
    });
    assert!(debug.contains("[SUCCESS] pi ⋅ provider/model ⋅ 12s"));
    assert!(debug.contains("[WARNING] qc_no_skills is ignored for tool 'opencode'"));
    assert!(debug.contains("[DEBUG] tool=pi"));
    assert!(debug.contains("[DEBUG] child_stderr:\nNo project session found"));
}

#[test]
fn formats_usage_between_success_and_session_and_omits_when_empty() {
    assert_eq!(compact_tokens(400.0), "400");
    assert_eq!(compact_tokens(1200.0), "1.2k");
    assert_eq!(compact_tokens(1000.0), "1k");
    assert_eq!(usage_line(None), None);
    assert_eq!(usage_line(Some(&AgentUsage::default())), None);
    assert_eq!(
        usage_line(Some(&AgentUsage {
            input_tokens: Some(1200.0),
            output_tokens: Some(400.0),
            cost: Some(0.012),
            ..AgentUsage::default()
        })),
        Some("[USAGE] 1.2k in ⋅ 400 out ⋅ 0.012".into())
    );
    assert_eq!(
        usage_line(Some(&AgentUsage {
            input_tokens: Some(1200.0),
            output_tokens: Some(400.0),
            cost: Some(0.012),
            cost_currency: Some("USD".into()),
            ..AgentUsage::default()
        })),
        Some("[USAGE] 1.2k in ⋅ 400 out ⋅ $0.012".into())
    );
    assert_eq!(
        usage_line(Some(&AgentUsage {
            cost: Some(0.012),
            cost_currency: Some("EUR".into()),
            ..AgentUsage::default()
        })),
        Some("[USAGE] 0.012 EUR".into())
    );
    assert_eq!(
        usage_line(Some(&AgentUsage {
            cost: Some(0.0),
            cost_currency: Some("USD".into()),
            ..AgentUsage::default()
        })),
        None
    );
    assert_eq!(
        usage_line(Some(&AgentUsage {
            input_tokens: Some(10.0),
            output_tokens: Some(4.0),
            cost: Some(0.0),
            cost_currency: Some("USD".into()),
            ..AgentUsage::default()
        })),
        Some("[USAGE] 10 in ⋅ 4 out".into())
    );
    assert_eq!(
        usage_line(Some(&AgentUsage {
            input_tokens: Some(1200.0),
            output_tokens: Some(400.0),
            cache_read_tokens: Some(50.0),
            thinking_tokens: Some(10.0),
            total_tokens: Some(1660.0),
            ..AgentUsage::default()
        })),
        Some("[USAGE] 1.2k in ⋅ 400 out ⋅ 50 cache ⋅ 10 think ⋅ 1.7k total".into())
    );

    let fields = usage_fields(Some(&AgentUsage {
        input_tokens: Some(10.0),
        output_tokens: Some(4.0),
        cost: Some(0.012),
        ..AgentUsage::default()
    }))
    .expect("fields");
    assert_eq!(fields["input_tokens"], 10);
    assert_eq!(fields["output_tokens"], 4);
    assert_eq!(fields["cost"].as_f64(), Some(0.012));

    let zero = usage_fields(Some(&AgentUsage {
        cost: Some(0.0),
        ..AgentUsage::default()
    }))
    .expect("zero cost");
    assert_eq!(zero["cost"], 0);
    assert_eq!(serde_json::to_string(&zero).expect("json"), r#"{"cost":0}"#);
    assert!(usage_fields(Some(&AgentUsage::default())).is_none());
    assert_eq!(usage_fields(None), None);

    let with_usage = text_envelope(TextEnvelopeInput {
        text: "joke",
        tool: "pi",
        duration_s: 12,
        session_id: "260906-1245--pi--abcdef",
        model: Some("provider/model"),
        warnings: &[],
        debug_lines: None,
        usage: Some(&AgentUsage {
            input_tokens: Some(1200.0),
            output_tokens: Some(400.0),
            cost: Some(0.012),
            ..AgentUsage::default()
        }),
    });
    assert_eq!(
        with_usage,
        [
            "\"\"\"",
            "joke",
            "\"\"\"",
            "[SUCCESS] pi ⋅ provider/model ⋅ 12s",
            "[USAGE] 1.2k in ⋅ 400 out ⋅ 0.012",
            "[QC-SESSION] 260906-1245--pi--abcdef",
            "",
        ]
        .join("\n")
    );
}
