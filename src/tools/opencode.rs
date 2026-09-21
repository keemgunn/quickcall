use serde_json::Value;

use crate::errors::QcError;
use crate::messages::empty_assistant_text;
use crate::tools::spawn::spawn_agent;
use crate::tools::types::{AgentRequest, AgentResult, AgentUsage, usage_from_record, utf16_units};

pub fn build_opencode_args(request: &AgentRequest) -> Vec<String> {
    let mut args = vec![
        "run".into(),
        "--auto".into(),
        "--format".into(),
        "json".into(),
        "--dir".into(),
        request.workdir.clone(),
    ];
    if request.native_id.is_none() {
        args.push("--title".into());
        args.push(request.session_id.clone());
    } else {
        args.push("-s".into());
        args.push(request.native_id.clone().unwrap());
    }
    if let Some(model) = &request.model {
        args.push("-m".into());
        args.push(model.clone());
    }
    if let Some(thinking) = &request.thinking {
        args.push("--variant".into());
        args.push(thinking.clone());
    }
    args.push(request.prompt.clone());
    args
}

fn opencode_error_detail(error: &Value, stderr: &str) -> String {
    if let Some(obj) = error.as_object() {
        if let Some(message) = obj
            .get("data")
            .and_then(Value::as_object)
            .and_then(|d| d.get("message"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            return message.to_string();
        }
        if let Some(message) = obj
            .get("message")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            return message.to_string();
        }
        if let Some(name) = obj
            .get("name")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            return name.to_string();
        }
    }
    if let Some(text) = error.as_str().map(str::trim).filter(|s| !s.is_empty()) {
        return text.to_string();
    }
    let trimmed = stderr.trim();
    if !trimmed.is_empty() {
        return trimmed.to_string();
    }
    "opencode reported a run error".into()
}

fn event_reported_error(event: &Value) -> Option<&Value> {
    let kind = event.get("type").and_then(Value::as_str);
    if kind == Some("error") {
        return event.get("error");
    }
    if kind == Some("session.error") {
        if let Some(error) = event.get("properties").and_then(|p| p.get("error")) {
            return Some(error);
        }
        return event.get("error");
    }
    None
}

fn event_usage_candidate(event: &Value) -> Option<&Value> {
    if event.get("usage").is_some() {
        return event.get("usage");
    }
    if let Some(usage) = event.get("properties").and_then(|p| p.get("usage")) {
        return Some(usage);
    }
    event.get("part").and_then(|p| p.get("usage"))
}

fn event_cost_candidate(event: &Value) -> Option<f64> {
    if let Some(n) = event.get("cost").and_then(Value::as_f64) {
        return Some(n);
    }
    event
        .get("properties")
        .and_then(|p| p.get("cost"))
        .and_then(Value::as_f64)
}

fn opencode_native_candidate(event: &Value) -> String {
    if let Some(id) = event
        .get("sessionID")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
    {
        return id.to_string();
    }
    if let Some(id) = event
        .get("session_id")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
    {
        return id.to_string();
    }
    if event.get("type").and_then(Value::as_str) == Some("session")
        && let Some(id) = event.get("id").and_then(Value::as_str)
        && id.starts_with("ses_")
    {
        return id.to_string();
    }
    String::new()
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

fn seed_opencode_native_id(fallback: &str) -> String {
    if fallback.is_empty() || pretty_id(fallback) {
        String::new()
    } else {
        fallback.to_string()
    }
}

pub struct ParsedOpenCode {
    pub text: String,
    pub native_id: String,
    pub result: Value,
    pub error: Option<String>,
    pub usage: Option<AgentUsage>,
}

pub fn parse_opencode_output(
    stdout: &str,
    fallback_native_id: &str,
    stderr: &str,
) -> ParsedOpenCode {
    let mut events = Vec::new();
    let mut texts = Vec::new();
    let mut native_id = seed_opencode_native_id(fallback_native_id);
    let mut reported_error = None;
    let mut usage = None;

    for line in stdout.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        events.push(event.clone());
        let candidate = opencode_native_candidate(&event);
        if !candidate.is_empty() {
            native_id = candidate;
        }
        if let Some(error) = event_reported_error(&event) {
            reported_error = Some(error.clone());
        }
        let cost = event_cost_candidate(&event);
        if let Some(lifted) = usage_from_record(event_usage_candidate(&event), cost, None) {
            usage = Some(lifted);
        }
        if event.get("type").and_then(Value::as_str) == Some("text") {
            if let Some(text) = event
                .get("part")
                .and_then(|p| p.get("text"))
                .and_then(Value::as_str)
            {
                texts.push(text.to_string());
            } else if let Some(text) = event.get("text").and_then(Value::as_str) {
                texts.push(text.to_string());
            }
        }
        let kind = event.get("type").and_then(Value::as_str);
        if kind != Some("error")
            && kind != Some("session.error")
            && let Some(result) = event.get("result").and_then(Value::as_str)
        {
            texts.push(result.to_string());
        }
    }

    let text = texts.join("");
    let result = if events.is_empty() {
        Value::String(stdout.to_string())
    } else {
        Value::Array(events)
    };
    if let Some(error) = reported_error {
        return ParsedOpenCode {
            text,
            native_id,
            result,
            error: Some(opencode_error_detail(&error, stderr)),
            usage: None,
        };
    }
    if text.trim().is_empty() {
        let detail = {
            let trimmed = stderr.trim();
            if trimmed.is_empty() {
                empty_assistant_text("opencode")
            } else {
                trimmed.to_string()
            }
        };
        return ParsedOpenCode {
            text,
            native_id,
            result,
            error: Some(detail),
            usage: None,
        };
    }
    ParsedOpenCode {
        text,
        native_id,
        result,
        error: None,
        usage,
    }
}

pub fn run_opencode(request: &AgentRequest) -> Result<AgentResult, QcError> {
    let args = build_opencode_args(request);
    let capture = spawn_agent(
        &request.command,
        &args,
        None,
        &request.workdir,
        &request.env,
        "opencode",
    )?;
    let parsed = parse_opencode_output(
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
