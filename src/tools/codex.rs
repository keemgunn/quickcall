use serde_json::Value;

use crate::errors::QcError;
use crate::messages::empty_assistant_text;
use crate::tools::spawn::spawn_agent;
use crate::tools::types::{AgentRequest, AgentResult, AgentUsage, CODEX_EFFORT, utf16_units};

/// qc `off` becomes Codex `none`; other CODEX_EFFORT values pass through.
fn map_codex_effort(level: &str) -> Option<&'static str> {
    match level {
        "off" => Some("none"),
        "minimal" => Some("minimal"),
        "low" => Some("low"),
        "medium" => Some("medium"),
        "high" => Some("high"),
        "xhigh" => Some("xhigh"),
        "max" => Some("max"),
        _ => None,
    }
}

pub fn build_codex_args(request: &AgentRequest) -> Vec<String> {
    let mut args = Vec::new();
    // `-c` is global; `--cd` / force-allow live on `exec` and do not cross `resume`.
    if let Some(thinking) = &request.thinking
        && CODEX_EFFORT.contains(&thinking.as_str())
        && let Some(mapped) = map_codex_effort(thinking)
    {
        args.push("-c".into());
        args.push(format!("model_reasoning_effort=\"{mapped}\""));
    }
    args.push("exec".into());
    args.push("--json".into());
    if let Some(model) = &request.model {
        args.push("--model".into());
        args.push(model.clone());
    }
    args.push("--cd".into());
    args.push(request.workdir.clone());
    args.push("--dangerously-bypass-approvals-and-sandbox".into());
    args.push("--dangerously-bypass-hook-trust".into());
    if let Some(native) = &request.native_id {
        args.push("resume".into());
        args.push(native.clone());
    }
    args.push("-".into());
    args
}

fn pretty_id(id: &str) -> bool {
    let bytes = id.as_bytes();
    if bytes.len() < 20 {
        return false;
    }
    let parts: Vec<&str> = id.splitn(3, "--").collect();
    if parts.len() != 3 {
        return false;
    }
    let stamp = parts[0].as_bytes();
    stamp.len() == 11
        && stamp[6] == b'-'
        && stamp.iter().enumerate().all(|(i, b)| {
            if i == 6 {
                *b == b'-'
            } else {
                b.is_ascii_digit()
            }
        })
        && parts[1].bytes().all(|b| b.is_ascii_lowercase())
        && !parts[1].is_empty()
        && parts[2].len() == 6
        && parts[2]
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
}

fn seed_codex_native_id(fallback: &str) -> String {
    if fallback.is_empty() || pretty_id(fallback) {
        String::new()
    } else {
        fallback.to_string()
    }
}

fn json_number(value: &Value) -> Option<f64> {
    value.as_f64()
}

fn usage_from_codex(raw: Option<&Value>) -> Option<AgentUsage> {
    let u = raw.and_then(Value::as_object)?;
    let mut out = AgentUsage::default();
    if let Some(n) = u.get("input_tokens").and_then(json_number) {
        out.input_tokens = Some(n);
    }
    if let Some(n) = u.get("output_tokens").and_then(json_number) {
        out.output_tokens = Some(n);
    }
    if let Some(n) = u.get("cached_input_tokens").and_then(json_number) {
        out.cache_read_tokens = Some(n);
    }
    if let Some(n) = u.get("reasoning_output_tokens").and_then(json_number) {
        out.thinking_tokens = Some(n);
    }
    if out.is_empty() { None } else { Some(out) }
}

fn error_message(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn nested_error_message(value: &Value) -> Option<String> {
    value
        .get("message")
        .and_then(error_message)
        .or_else(|| error_message(value))
}

pub struct ParsedCodex {
    pub text: String,
    pub native_id: String,
    pub result: Value,
    pub error: Option<String>,
    pub usage: Option<AgentUsage>,
}

/// JSONL: persist `thread.started.thread_id` only; last completed `agent_message` wins.
pub fn parse_codex_output(stdout: &str, fallback_native_id: &str, stderr: &str) -> ParsedCodex {
    let mut events = Vec::new();
    let mut text = String::new();
    let mut native_id = seed_codex_native_id(fallback_native_id);
    let mut turn_failed = None;
    let mut top_error = None;
    let mut usage = None;

    for line in stdout.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        events.push(event.clone());
        let kind = event.get("type").and_then(Value::as_str);

        if kind == Some("thread.started")
            && let Some(id) = event
                .get("thread_id")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
        {
            native_id = id.to_string();
        }

        if kind == Some("item.completed")
            && let Some(item) = event.get("item")
            && item.get("type").and_then(Value::as_str) == Some("agent_message")
            && let Some(body) = item.get("text").and_then(Value::as_str)
        {
            text = body.to_string();
        }

        if kind == Some("turn.completed")
            && let Some(lifted) = usage_from_codex(event.get("usage"))
        {
            usage = Some(lifted);
        }

        if kind == Some("turn.failed") && turn_failed.is_none() {
            turn_failed = event.get("error").and_then(nested_error_message);
        }

        if kind == Some("error") && top_error.is_none() {
            top_error = event
                .get("message")
                .and_then(error_message)
                .or_else(|| event.get("error").and_then(nested_error_message));
        } else if kind != Some("turn.failed")
            && kind != Some("item.completed")
            && top_error.is_none()
        {
            top_error = event.get("error").and_then(nested_error_message);
        }
    }

    let result = if events.is_empty() {
        Value::String(stdout.to_string())
    } else {
        Value::Array(events)
    };
    let fatal = turn_failed.or(top_error);
    if let Some(detail) = fatal {
        return ParsedCodex {
            text,
            native_id,
            result,
            error: Some(detail),
            usage: None,
        };
    }
    if text.trim().is_empty() {
        let detail = {
            let trimmed = stderr.trim();
            if trimmed.is_empty() {
                empty_assistant_text("codex")
            } else {
                trimmed.to_string()
            }
        };
        return ParsedCodex {
            text,
            native_id,
            result,
            error: Some(detail),
            usage: None,
        };
    }
    ParsedCodex {
        text,
        native_id,
        result,
        error: None,
        usage,
    }
}

pub fn run_codex(request: &AgentRequest) -> Result<AgentResult, QcError> {
    let args = build_codex_args(request);
    let capture = spawn_agent(
        &request.command,
        &args,
        Some(&request.prompt),
        &request.workdir,
        &request.env,
        "codex",
    )?;
    let parsed = parse_codex_output(
        &capture.stdout,
        request.native_id.as_deref().unwrap_or(""),
        &capture.stderr,
    );
    Ok(AgentResult {
        text: parsed.text,
        native_id: parsed.native_id,
        exit: capture.exit,
        result: parsed.result,
        stderr: capture.stderr,
        argv: args,
        stdout_bytes: utf16_units(&capture.stdout),
        error: parsed.error,
        usage: parsed.usage,
    })
}
