use serde_json::Value;

use crate::errors::QcError;
use crate::tools::spawn::spawn_agent;
use crate::tools::types::{AgentRequest, AgentResult, AgentUsage, utf16_units};

pub fn build_pi_args(request: &AgentRequest) -> Vec<String> {
    let mut args = vec![
        "--mode".into(),
        "json".into(),
        "-a".into(),
        "--session-id".into(),
        request.session_id.clone(),
    ];
    if let Some(model) = &request.model {
        args.push("--model".into());
        args.push(model.clone());
    }
    if let Some(thinking) = &request.thinking {
        args.push("--thinking".into());
        args.push(thinking.clone());
    }
    if request.no_skills == Some(true) {
        args.push("--no-skills".into());
    }
    if let Some(skill) = &request.skill_path {
        args.push("--skill".into());
        args.push(skill.clone());
    }
    args
}

fn text_parts_from_content(content: &Value) -> Vec<String> {
    if let Some(text) = content.as_str() {
        return vec![text.to_string()];
    }
    let Some(items) = content.as_array() else {
        return Vec::new();
    };
    let mut texts = Vec::new();
    for part in items {
        if part.get("type").and_then(Value::as_str) == Some("text")
            && let Some(text) = part.get("text").and_then(Value::as_str)
        {
            texts.push(text.to_string());
        }
    }
    texts
}

fn usage_from_pi(raw: Option<&Value>) -> Option<AgentUsage> {
    let u = raw.and_then(Value::as_object)?;
    let mut out = AgentUsage::default();
    if let Some(n) = u.get("input").and_then(Value::as_f64) {
        out.input_tokens = Some(n);
    }
    if let Some(n) = u.get("output").and_then(Value::as_f64) {
        out.output_tokens = Some(n);
    }
    if let Some(n) = u.get("cacheRead").and_then(Value::as_f64) {
        out.cache_read_tokens = Some(n);
    }
    if let Some(n) = u.get("reasoning").and_then(Value::as_f64) {
        out.thinking_tokens = Some(n);
    }
    if let Some(n) = u.get("totalTokens").and_then(Value::as_f64) {
        out.total_tokens = Some(n);
    }
    if let Some(n) = u
        .get("cost")
        .and_then(Value::as_object)
        .and_then(|c| c.get("total"))
        .and_then(Value::as_f64)
    {
        out.cost = Some(n);
    }
    if out.is_empty() { None } else { Some(out) }
}

fn event_usage(event: &Value) -> Option<&Value> {
    if event.get("usage").is_some() {
        return event.get("usage");
    }
    event.get("message").and_then(|m| m.get("usage"))
}

pub struct ParsedPi {
    pub text: String,
    pub native_id: String,
    pub result: Value,
    pub error: Option<String>,
    pub usage: Option<AgentUsage>,
}

/// Live `--mode json` assistant `message_end`; legacy message/text fallback.
pub fn parse_pi_output(stdout: &str, fallback_session_id: &str) -> ParsedPi {
    let mut native_id = fallback_session_id.to_string();
    let mut live_texts = Vec::new();
    let mut fallback_texts = Vec::new();
    let mut events = Vec::new();
    let mut error = None;
    let mut end_usage = None;
    let mut update_usage = None;

    for line in stdout.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        events.push(event.clone());
        if event.get("type").and_then(Value::as_str) == Some("session")
            && let Some(id) = event
                .get("id")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
        {
            native_id = id.to_string();
        }
        if event.get("type").and_then(Value::as_str) == Some("message_update")
            && let Some(raw) = event_usage(&event)
        {
            update_usage = Some(raw.clone());
        }
        if event.get("type").and_then(Value::as_str) == Some("message_end")
            && let Some(message) = event.get("message")
        {
            if message.get("role").and_then(Value::as_str) == Some("assistant") {
                if let Some(content) = message.get("content") {
                    live_texts.extend(text_parts_from_content(content));
                }
                if let Some(usage) = message.get("usage") {
                    end_usage = Some(usage.clone());
                }
            }
            if message.get("stopReason").and_then(Value::as_str) == Some("error") {
                let detail = message
                    .get("errorMessage")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .unwrap_or("pi reported a run error");
                error = Some(detail.to_string());
            }
        }
        if event.get("type").and_then(Value::as_str) == Some("message")
            && event.get("role").and_then(Value::as_str) == Some("assistant")
            && let Some(content) = event.get("content")
        {
            fallback_texts.extend(text_parts_from_content(content));
        }
        if event.get("type").and_then(Value::as_str) == Some("text")
            && let Some(text) = event.get("text").and_then(Value::as_str)
        {
            fallback_texts.push(text.to_string());
        }
    }

    let text = if live_texts.is_empty() {
        fallback_texts.join("")
    } else {
        live_texts.join("")
    };
    ParsedPi {
        text,
        native_id,
        result: Value::Array(events),
        error,
        usage: usage_from_pi(end_usage.as_ref().or(update_usage.as_ref())),
    }
}

pub fn run_pi(request: &AgentRequest) -> Result<AgentResult, QcError> {
    let args = build_pi_args(request);
    let capture = spawn_agent(
        &request.command,
        &args,
        Some(&request.prompt),
        &request.workdir,
        &request.env,
        "pi",
    )?;
    let parsed = parse_pi_output(&capture.stdout, &request.session_id);
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
