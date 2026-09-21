use std::collections::HashMap;

use quickcall::tools::antigravity::{build_antigravity_args, parse_antigravity_output};
use quickcall::tools::claude::{build_claude_args, parse_claude_output};
use quickcall::tools::codex::{build_codex_args, parse_codex_output};
use quickcall::tools::cursor::{build_cursor_args, parse_cursor_output};
use quickcall::tools::opencode::{build_opencode_args, parse_opencode_output};
use quickcall::tools::pi::{build_pi_args, parse_pi_output};
use quickcall::tools::types::AgentRequest;

fn base(over: impl FnOnce(&mut AgentRequest)) -> AgentRequest {
    let mut request = AgentRequest {
        prompt: "hello".into(),
        model: None,
        thinking: None,
        workdir: "/work".into(),
        session_id: "260901-1200--pi--abcdef".into(),
        native_id: None,
        no_skills: None,
        skill_path: None,
        command: "pi".into(),
        env: HashMap::new(),
    };
    over(&mut request);
    request
}

#[test]
fn builds_pi_argv_with_force_allow_and_session_id() {
    let request = base(|r| {
        r.model = Some("m".into());
        r.thinking = Some("high".into());
        r.no_skills = Some(true);
        r.skill_path = Some("/s".into());
    });
    assert_eq!(
        build_pi_args(&request),
        [
            "--mode",
            "json",
            "-a",
            "--session-id",
            "260901-1200--pi--abcdef",
            "--model",
            "m",
            "--thinking",
            "high",
            "--no-skills",
            "--skill",
            "/s",
        ]
    );
}

#[test]
fn parses_pi_jsonl_session_id_and_assistant_text() {
    let stdout = format!(
        "{}\n{}\n",
        serde_json::json!({"type":"session","id":"native-pi","version":3}),
        serde_json::json!({"type":"message","role":"assistant","content":[{"type":"text","text":"answer"}]})
    );
    let parsed = parse_pi_output(&stdout, "fallback");
    assert_eq!(parsed.text, "answer");
    assert_eq!(parsed.native_id, "native-pi");
}

#[test]
fn extracts_pi_assistant_text_from_live_message_end_content() {
    let joke = "Why do programmers prefer dark mode? Because light attracts bugs.";
    let stdout = format!(
        "{}\n{}\n",
        serde_json::json!({"type":"session","id":"native-pi","version":3}),
        serde_json::json!({"type":"message_end","message":{"role":"assistant","content":[{"type":"text","text":joke}]}})
    );
    let parsed = parse_pi_output(&stdout, "fallback");
    assert_eq!(parsed.text, joke);
    assert_eq!(parsed.native_id, "native-pi");
}

#[test]
fn extracts_only_assistant_message_end_text_when_the_live_stream_also_echoes_the_user_prompt() {
    let prompt =
        "# Tell a Short Joke\n- Prefer a classic setup and punchline, or a single one-liner.";
    let joke = "Why did the scarecrow win an award? Because he was outstanding in his field!";
    let stdout = [
        serde_json::json!({"type":"session","id":"native-pi","version":3}).to_string(),
        serde_json::json!({"type":"message_end","message":{"role":"user","content":[{"type":"text","text":prompt}]}}).to_string(),
        serde_json::json!({"type":"message_end","message":{"role":"assistant","content":[{"type":"thinking","thinking":"Pick a classic one-liner."},{"type":"text","text":joke}]}}).to_string(),
        serde_json::json!({"type":"message","role":"user","content":[{"type":"text","text":prompt}]}).to_string(),
    ]
    .join("\n");
    let parsed = parse_pi_output(&stdout, "fallback");
    assert_eq!(parsed.text, joke);
    assert_eq!(parsed.native_id, "native-pi");
    assert!(!parsed.text.contains("Tell a Short Joke"));
}

#[test]
fn builds_cursor_argv_with_force_allow_and_workspace() {
    let request = base(|r| {
        r.command = "agent".into();
        r.model = Some("composer-2.5".into());
        r.native_id = Some("uuid".into());
    });
    assert_eq!(
        build_cursor_args(&request),
        [
            "-p",
            "--force",
            "--yolo",
            "--approve-mcps",
            "--trust",
            "--output-format",
            "json",
            "--workspace",
            "/work",
            "--model",
            "composer-2.5",
            "--resume",
            "uuid",
            "hello",
        ]
    );
    let parsed =
        parse_cursor_output(&serde_json::json!({"result":"ok","session_id":"sid"}).to_string());
    assert_eq!(parsed.text, "ok");
    assert_eq!(parsed.native_id, "sid");
    assert_eq!(
        parsed.result,
        serde_json::json!({"result":"ok","session_id":"sid"})
    );
}

#[test]
fn returns_a_cursor_empty_json_error_instead_of_throwing_so_callers_can_attach_stderr() {
    let empty = parse_cursor_output("");
    assert_eq!(empty.text, "");
    assert_eq!(empty.native_id, "");
    assert_eq!(
        empty.error.as_deref(),
        Some("cursor agent produced empty JSON output")
    );
    let invalid = parse_cursor_output("not-json");
    assert_eq!(invalid.text, "");
    assert_eq!(invalid.native_id, "");
    assert_eq!(
        invalid.error.as_deref(),
        Some("cursor agent produced invalid JSON output")
    );
}

#[test]
fn builds_claude_argv_with_skip_permissions_and_effort() {
    let created = build_claude_args(&base(|r| {
        r.command = "claude".into();
        r.model = Some("sonnet".into());
        r.thinking = Some("high".into());
    }));
    assert!(
        created
            .args
            .contains(&"--dangerously-skip-permissions".into())
    );
    assert_eq!(
        &created.args[..3],
        ["-p", "--dangerously-skip-permissions", "--output-format"]
    );
    assert!(created.args.contains(&"--effort".into()));
    assert!(created.args.contains(&"high".into()));
    assert!(created.args.contains(&"--session-id".into()));
    assert!(created.args.contains(&"--name".into()));
    assert_eq!(created.args.last().map(String::as_str), Some("hello"));
    let uuid = created.create_uuid.expect("uuid");
    assert!(regex_uuid(&uuid), "{uuid}");

    let resumed = build_claude_args(&base(|r| {
        r.command = "claude".into();
        r.native_id = Some("native".into());
    }));
    assert!(resumed.args.contains(&"--resume".into()));
    assert!(resumed.args.contains(&"native".into()));
    let parsed = parse_claude_output(
        &serde_json::json!({"result":"c","session_id":"s"}).to_string(),
        "fb",
        "",
    )
    .expect("parse");
    assert_eq!(parsed.text, "c");
    assert_eq!(parsed.native_id, "s");
    let usage = parse_claude_output(
        &serde_json::json!({
            "result":"c",
            "session_id":"s",
            "usage":{"input_tokens":100,"output_tokens":20},
            "total_cost_usd":0.012
        })
        .to_string(),
        "fb",
        "",
    )
    .expect("usage");
    let u = usage.usage.expect("usage present");
    assert_eq!(u.input_tokens, Some(100.0));
    assert_eq!(u.output_tokens, Some(20.0));
    assert_eq!(u.cost, Some(0.012));
    assert_eq!(u.cost_currency.as_deref(), Some("USD"));
}

fn regex_uuid(value: &str) -> bool {
    value.len() == 36
        && value.chars().enumerate().all(|(i, ch)| {
            if i == 8 || i == 13 || i == 18 || i == 23 {
                ch == '-'
            } else {
                ch.is_ascii_hexdigit()
            }
        })
}

#[test]
fn extracts_pi_usage_from_the_last_assistant_message_end() {
    let stdout = [
        serde_json::json!({"type":"session","id":"native-pi","version":3}).to_string(),
        serde_json::json!({"type":"message_update","usage":{"input":9,"output":1,"cost":{"total":0.001}}}).to_string(),
        serde_json::json!({"type":"message_end","message":{"role":"assistant","content":[{"type":"text","text":"answer"}],"usage":{"input":1200,"output":400,"cacheRead":50,"reasoning":10,"totalTokens":1660,"cost":{"total":0.012}}}}).to_string(),
    ]
    .join("\n");
    let parsed = parse_pi_output(&stdout, "fallback");
    assert_eq!(parsed.text, "answer");
    assert_eq!(parsed.native_id, "native-pi");
    let u = parsed.usage.expect("usage");
    assert_eq!(u.input_tokens, Some(1200.0));
    assert_eq!(u.output_tokens, Some(400.0));
    assert_eq!(u.cache_read_tokens, Some(50.0));
    assert_eq!(u.thinking_tokens, Some(10.0));
    assert_eq!(u.total_tokens, Some(1660.0));
    assert_eq!(u.cost, Some(0.012));
}

#[test]
fn keeps_json_result_number_serialization_and_a_reported_zero_cost() {
    let stdout = concat!(
        r#"{"type":"session","id":"native-pi","version":3,"n":0,"f":1.5,"i":1,"big":1e+21}"#,
        "\n",
        r#"{"type":"message_end","message":{"role":"assistant","content":[{"type":"text","text":"answer"}],"usage":{"input":10,"output":4,"cost":{"total":0}}}}"#,
        "\n",
    );
    let parsed = parse_pi_output(stdout, "fallback");
    assert_eq!(
        parsed.usage.as_ref().and_then(|u| u.input_tokens),
        Some(10.0)
    );
    assert_eq!(
        parsed.usage.as_ref().and_then(|u| u.output_tokens),
        Some(4.0)
    );
    assert_eq!(parsed.usage.as_ref().and_then(|u| u.cost), Some(0.0));
    let encoded = serde_json::to_string(&parsed.result).expect("json");
    assert_eq!(
        encoded,
        r#"[{"type":"session","id":"native-pi","version":3,"n":0,"f":1.5,"i":1,"big":1e+21},{"type":"message_end","message":{"role":"assistant","content":[{"type":"text","text":"answer"}],"usage":{"input":10,"output":4,"cost":{"total":0}}}}]"#
    );
}

#[test]
fn returns_pi_stopreason_error_with_session_native_id_and_no_assistant_text() {
    let detail = "OpenAI API error (429): rate_limit_exceeded";
    let stdout = [
        serde_json::json!({"type":"session","id":"native-pi","version":3}).to_string(),
        serde_json::json!({"type":"message_end","message":{"role":"assistant","content":[],"stopReason":"error","errorMessage":detail}}).to_string(),
    ]
    .join("\n");
    let parsed = parse_pi_output(&stdout, "fallback");
    assert_eq!(parsed.text, "");
    assert_eq!(parsed.native_id, "native-pi");
    assert_eq!(parsed.error.as_deref(), Some(detail));
}

#[test]
fn treats_claude_is_error_json_as_a_hard_fail_even_when_result_has_assistant_text() {
    let detail = "There's an issue with the selected model (some-wrong-fake-model). It may not exist or you may not have access to it.";
    let stdout = serde_json::json!({
        "type":"result",
        "subtype":"success",
        "is_error":true,
        "result":detail,
        "session_id":"sess-claude"
    })
    .to_string();
    let parsed = parse_claude_output(&stdout, "fb", "").expect("parse");
    assert_eq!(parsed.text, detail);
    assert_eq!(parsed.native_id, "sess-claude");
    assert_eq!(parsed.error.as_deref(), Some(detail));
}

#[test]
fn treats_claude_unrecognized_model_stderr_as_a_hard_fail_when_json_still_looks_successful() {
    let detail = "There's an issue with the selected model (some-wrong-fake-model). It may not exist or you may not have access to it.";
    let stdout = serde_json::json!({
        "type":"result",
        "subtype":"success",
        "is_error":false,
        "result":detail,
        "session_id":"sess-claude"
    })
    .to_string();
    let stderr = "[claude-code:unrecognized_model] model not in catalog\n";
    let parsed = parse_claude_output(&stdout, "fb", stderr).expect("parse");
    assert_eq!(parsed.native_id, "sess-claude");
    assert_eq!(parsed.error.as_deref(), Some(detail));
}

#[test]
fn builds_opencode_argv_with_auto_dir_title_resume() {
    let created = base(|r| {
        r.command = "opencode".into();
        r.model = Some("opencode-go/x".into());
        r.thinking = Some("high".into());
    });
    assert_eq!(
        build_opencode_args(&created),
        [
            "run",
            "--auto",
            "--format",
            "json",
            "--dir",
            "/work",
            "--title",
            "260901-1200--pi--abcdef",
            "-m",
            "opencode-go/x",
            "--variant",
            "high",
            "hello",
        ]
    );
    let resumed = base(|r| {
        r.native_id = Some("ses_1".into());
    });
    assert!(build_opencode_args(&resumed).contains(&"-s".into()));
    let parsed = parse_opencode_output(
        &format!(
            "{}\n{}\n",
            serde_json::json!({"type":"session","sessionID":"ses_x"}),
            serde_json::json!({"type":"text","part":{"text":"t"}})
        ),
        "fallback",
        "",
    );
    assert_eq!(parsed.text, "t");
    assert_eq!(parsed.native_id, "ses_x");
    let pretty = parse_opencode_output(
        &format!(
            "{}\n",
            serde_json::json!({"type":"text","part":{"text":"t"}})
        ),
        "260901-1200--opencode--abcdef",
        "",
    );
    assert_eq!(pretty.text, "t");
    assert_eq!(pretty.native_id, "");
}

#[test]
fn treats_opencode_jsonl_error_events_as_a_hard_fail() {
    let detail = "Model not found: some-fake-wrong-model";
    let stdout = [
        serde_json::json!({"type":"session","sessionID":"ses_x"}).to_string(),
        serde_json::json!({"type":"error","sessionID":"ses_x","error":{"name":"UnknownError","data":{"message":detail}}}).to_string(),
    ]
    .join("\n");
    let parsed = parse_opencode_output(&stdout, "pretty-fallback", "");
    assert_eq!(parsed.text, "");
    assert_eq!(parsed.native_id, "ses_x");
    assert_eq!(parsed.error.as_deref(), Some(detail));
}

#[test]
fn treats_opencode_empty_assistant_text_as_a_hard_fail() {
    let stdout = format!(
        "{}\n",
        serde_json::json!({"type":"session","sessionID":"ses_x"})
    );
    let parsed = parse_opencode_output(&stdout, "pretty-fallback", "");
    assert_eq!(parsed.text, "");
    assert_eq!(parsed.native_id, "ses_x");
    assert_eq!(
        parsed.error.as_deref(),
        Some("opencode produced empty assistant text")
    );
}

#[test]
fn builds_antigravity_argv_and_parses_response_conversation_id() {
    let request = base(|r| {
        r.command = "agy".into();
        r.model = Some("m".into());
        r.thinking = Some("medium".into());
        r.native_id = Some("c1".into());
    });
    let args = build_antigravity_args(&request);
    assert_eq!(
        args,
        [
            "--dangerously-skip-permissions",
            "--output-format",
            "json",
            "--model",
            "m",
            "--effort",
            "medium",
            "--conversation",
            "c1",
            "--add-dir",
            "/work",
            "-p",
            "hello",
        ]
    );
    let add_dir = args.iter().position(|t| t == "--add-dir").expect("add-dir");
    assert_eq!(args[add_dir + 1], "/work");
    let p = args.iter().position(|t| t == "-p").expect("-p");
    assert!(p > add_dir);
    assert_eq!(args[p + 1], "hello");
    assert!(!args[p + 1].starts_with('-'));
    assert_eq!(args.last().map(String::as_str), Some("hello"));
    let parsed = parse_antigravity_output(
        &serde_json::json!({"response":"r","conversation_id":"cid"}).to_string(),
        "",
    );
    assert_eq!(parsed.text, "r");
    assert_eq!(parsed.native_id, "cid");
    let usage = parse_antigravity_output(
        &serde_json::json!({
            "response":"r",
            "conversation_id":"cid",
            "usage":{
                "input_tokens":1200,
                "output_tokens":400,
                "thinking_tokens":20,
                "cache_read_tokens":50,
                "total_tokens":1660
            }
        })
        .to_string(),
        "",
    );
    let u = usage.usage.expect("usage");
    assert_eq!(u.input_tokens, Some(1200.0));
    assert_eq!(u.output_tokens, Some(400.0));
    assert_eq!(u.thinking_tokens, Some(20.0));
    assert_eq!(u.cache_read_tokens, Some(50.0));
    assert_eq!(u.total_tokens, Some(1660.0));
}

#[test]
fn surfaces_agy_status_error_from_the_payload_error_field() {
    let detail = r#"invalid model selection (--model "gemini-3.8-flash"): model gemini-3.8-flash is not recognized as a known model or custom model in settings"#;
    let stdout = serde_json::json!({
        "status":"ERROR",
        "conversation_id":"",
        "error":detail,
        "response":""
    })
    .to_string();
    let parsed = parse_antigravity_output(&stdout, "");
    assert_eq!(parsed.text, "");
    assert_eq!(parsed.native_id, "");
    assert_eq!(parsed.error.as_deref(), Some(detail));
}

#[test]
fn falls_back_to_captured_stderr_when_agy_json_has_no_error_field() {
    let stdout =
        serde_json::json!({"status":"ERROR","conversation_id":"","response":""}).to_string();
    let parsed = parse_antigravity_output(&stdout, "agy refused the model\n");
    assert_eq!(parsed.native_id, "");
    assert_eq!(parsed.error.as_deref(), Some("agy refused the model"));
}

#[test]
fn builds_codex_create_argv_with_stdin_dash_thinking_model_cd_and_force_allow() {
    let request = base(|r| {
        r.command = "codex".into();
        r.model = Some("gpt-5.6-luna".into());
        r.thinking = Some("medium".into());
        r.session_id = "260901-1200--codex--abcdef".into();
    });
    assert_eq!(
        build_codex_args(&request),
        [
            "-c",
            r#"model_reasoning_effort="medium""#,
            "exec",
            "--json",
            "--model",
            "gpt-5.6-luna",
            "--cd",
            "/work",
            "--dangerously-bypass-approvals-and-sandbox",
            "--dangerously-bypass-hook-trust",
            "-",
        ]
    );
}

#[test]
fn maps_codex_qc_off_thinking_to_none_and_omits_thinking_or_model_when_absent() {
    let off = build_codex_args(&base(|r| {
        r.thinking = Some("off".into());
    }));
    assert_eq!(
        &off[..3],
        ["-c", r#"model_reasoning_effort="none""#, "exec"]
    );
    assert!(!off.contains(&"--model".into()));
    assert_eq!(off.last().map(String::as_str), Some("-"));

    let omitted = build_codex_args(&base(|_| {}));
    assert!(!omitted.contains(&"-c".into()));
    assert!(!omitted.contains(&"--model".into()));
    assert!(omitted.contains(&"exec".into()));
    assert!(omitted.contains(&"--json".into()));
    assert_eq!(
        omitted,
        [
            "exec",
            "--json",
            "--cd",
            "/work",
            "--dangerously-bypass-approvals-and-sandbox",
            "--dangerously-bypass-hook-trust",
            "-",
        ]
    );
}

#[test]
fn builds_codex_resume_argv_with_exec_options_before_resume() {
    let args = build_codex_args(&base(|r| {
        r.model = Some("gpt-5.6-luna".into());
        r.thinking = Some("high".into());
        r.native_id = Some("thread_1".into());
    }));
    assert_eq!(
        args,
        [
            "-c",
            r#"model_reasoning_effort="high""#,
            "exec",
            "--json",
            "--model",
            "gpt-5.6-luna",
            "--cd",
            "/work",
            "--dangerously-bypass-approvals-and-sandbox",
            "--dangerously-bypass-hook-trust",
            "resume",
            "thread_1",
            "-",
        ]
    );
    let resume = args.iter().position(|t| t == "resume").expect("resume");
    let exec = args.iter().position(|t| t == "exec").expect("exec");
    let cd = args.iter().position(|t| t == "--cd").expect("cd");
    assert!(exec < resume);
    assert!(cd < resume);
    assert!(args.contains(&"--dangerously-bypass-approvals-and-sandbox".into()));
    assert!(args.contains(&"--dangerously-bypass-hook-trust".into()));
    assert!(!args.contains(&"hello".into()));
}

#[test]
fn parses_codex_jsonl_thread_id_last_agent_message_and_usage_maps() {
    let stdout = [
        serde_json::json!({"type":"thread.started","thread_id":"thread_abc"}).to_string(),
        serde_json::json!({"type":"item.completed","item":{"type":"agent_message","text":"first"}}).to_string(),
        serde_json::json!({"type":"item.completed","item":{"type":"agent_message","text":"last-answer"}}).to_string(),
        serde_json::json!({"type":"turn.completed","usage":{
            "input_tokens":10,
            "output_tokens":4,
            "cached_input_tokens":2,
            "reasoning_output_tokens":3,
            "cache_write_input_tokens":9
        }}).to_string(),
    ]
    .join("\n");
    let parsed = parse_codex_output(&stdout, "", "");
    assert_eq!(parsed.text, "last-answer");
    assert_eq!(parsed.native_id, "thread_abc");
    assert!(parsed.error.is_none());
    let u = parsed.usage.expect("usage");
    assert_eq!(u.input_tokens, Some(10.0));
    assert_eq!(u.output_tokens, Some(4.0));
    assert_eq!(u.cache_read_tokens, Some(2.0));
    assert_eq!(u.thinking_tokens, Some(3.0));
    assert_eq!(u.total_tokens, None);
    assert_eq!(u.cost, None);
}

#[test]
fn omits_codex_usage_when_absent_and_keeps_optional_zeros_when_reported() {
    let no_usage = parse_codex_output(
        &format!(
            "{}\n{}\n",
            serde_json::json!({"type":"thread.started","thread_id":"t1"}),
            serde_json::json!({"type":"item.completed","item":{"type":"agent_message","text":"ok"}})
        ),
        "",
        "",
    );
    assert!(no_usage.usage.is_none());

    let partial = parse_codex_output(
        &format!(
            "{}\n{}\n{}\n",
            serde_json::json!({"type":"thread.started","thread_id":"t1"}),
            serde_json::json!({"type":"item.completed","item":{"type":"agent_message","text":"ok"}}),
            serde_json::json!({"type":"turn.completed","usage":{"input_tokens":5,"output_tokens":1}})
        ),
        "",
        "",
    );
    let u = partial.usage.expect("partial");
    assert_eq!(u.input_tokens, Some(5.0));
    assert_eq!(u.output_tokens, Some(1.0));
    assert_eq!(u.cache_read_tokens, None);
    assert_eq!(u.thinking_tokens, None);
    assert_eq!(u.total_tokens, None);

    let zeros = parse_codex_output(
        &format!(
            "{}\n{}\n{}\n",
            serde_json::json!({"type":"thread.started","thread_id":"t1"}),
            serde_json::json!({"type":"item.completed","item":{"type":"agent_message","text":"ok"}}),
            serde_json::json!({"type":"turn.completed","usage":{
                "input_tokens":0,
                "output_tokens":0,
                "cached_input_tokens":0,
                "reasoning_output_tokens":0
            }})
        ),
        "",
        "",
    );
    let z = zeros.usage.expect("zeros");
    assert_eq!(z.input_tokens, Some(0.0));
    assert_eq!(z.output_tokens, Some(0.0));
    assert_eq!(z.cache_read_tokens, Some(0.0));
    assert_eq!(z.thinking_tokens, Some(0.0));
}

#[test]
fn reports_one_codex_fatal_when_turn_failed_and_top_level_error_both_appear() {
    let stdout = [
        serde_json::json!({"type":"thread.started","thread_id":"t-fail"}).to_string(),
        serde_json::json!({"type":"turn.failed","error":{"message":"turn boom"}}).to_string(),
        serde_json::json!({"type":"error","message":"top boom"}).to_string(),
    ]
    .join("\n");
    let parsed = parse_codex_output(&stdout, "", "");
    assert_eq!(parsed.native_id, "t-fail");
    assert_eq!(parsed.error.as_deref(), Some("turn boom"));
    assert!(!parsed.error.as_deref().unwrap_or("").contains("top boom"));
}

#[test]
fn treats_codex_item_error_as_nonfatal_when_assistant_text_exists() {
    let stdout = [
        serde_json::json!({"type":"thread.started","thread_id":"t1"}).to_string(),
        serde_json::json!({"type":"item.completed","item":{"type":"error","text":"tool failed"}}).to_string(),
        serde_json::json!({"type":"item.completed","item":{"type":"agent_message","text":"recovered"}}).to_string(),
    ]
    .join("\n");
    let parsed = parse_codex_output(&stdout, "", "");
    assert_eq!(parsed.text, "recovered");
    assert_eq!(parsed.native_id, "t1");
    assert!(parsed.error.is_none());
}

#[test]
fn fails_closed_on_empty_codex_text_missing_jsonl_and_stderr_only_startup() {
    let empty = parse_codex_output(
        &format!(
            "{}\n",
            serde_json::json!({"type":"thread.started","thread_id":"t-empty"})
        ),
        "",
        "",
    );
    assert_eq!(empty.text, "");
    assert_eq!(empty.native_id, "t-empty");
    assert_eq!(
        empty.error.as_deref(),
        Some("codex produced empty assistant text")
    );

    let invalid = parse_codex_output("not-json\n{bad\n", "", "");
    assert_eq!(invalid.native_id, "");
    assert_eq!(
        invalid.error.as_deref(),
        Some("codex produced empty assistant text")
    );

    let stderr_only = parse_codex_output("", "", "codex: not logged in\n");
    assert_eq!(stderr_only.native_id, "");
    assert_eq!(stderr_only.error.as_deref(), Some("codex: not logged in"));
    assert!(stderr_only.usage.is_none());
}

#[test]
fn keeps_codex_continue_native_id_when_stream_omits_thread_and_never_uses_pretty_id() {
    let kept = parse_codex_output(
        &format!(
            "{}\n",
            serde_json::json!({"type":"item.completed","item":{"type":"agent_message","text":"cont"}})
        ),
        "stored-thread",
        "",
    );
    assert_eq!(kept.text, "cont");
    assert_eq!(kept.native_id, "stored-thread");

    let observed = parse_codex_output(
        &format!(
            "{}\n{}\n",
            serde_json::json!({"type":"thread.started","thread_id":"thread_new"}),
            serde_json::json!({"type":"item.completed","item":{"type":"agent_message","text":"cont"}})
        ),
        "stored-thread",
        "",
    );
    assert_eq!(observed.native_id, "thread_new");

    let pretty = parse_codex_output(
        &format!(
            "{}\n",
            serde_json::json!({"type":"item.completed","item":{"type":"agent_message","text":"x"}})
        ),
        "260901-1200--codex--abcdef",
        "",
    );
    assert_eq!(pretty.text, "x");
    assert_eq!(pretty.native_id, "");
}
