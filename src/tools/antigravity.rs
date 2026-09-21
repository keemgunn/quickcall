use serde_json::Value;

use crate::errors::QcError;
use crate::tools::spawn::spawn_agent;
use crate::tools::types::{
    AGY_EFFORT, AgentRequest, AgentResult, AgentUsage, usage_from_record, utf16_units,
};

pub fn build_antigravity_args(request: &AgentRequest) -> Vec<String> {
    let mut args = vec![
        "--dangerously-skip-permissions".into(),
        "--output-format".into(),
        "json".into(),
    ];
    if let Some(model) = &request.model {
        args.push("--model".into());
        args.push(model.clone());
    }
    if let Some(thinking) = &request.thinking
        && AGY_EFFORT.contains(&thinking.as_str())
    {
        args.push("--effort".into());
        args.push(thinking.clone());
    }
    if let Some(native) = &request.native_id {
        args.push("--conversation".into());
        args.push(native.clone());
    }
    args.push("--add-dir".into());
    args.push(request.workdir.clone());
    args.push("-p".into());
    args.push(request.prompt.clone());
    args
}

pub struct ParsedAntigravity {
    pub text: String,
    pub native_id: String,
    pub result: Value,
    pub error: Option<String>,
    pub usage: Option<AgentUsage>,
}

pub fn parse_antigravity_output(stdout: &str, stderr: &str) -> ParsedAntigravity {
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        let detail = {
            let t = stderr.trim();
            if t.is_empty() {
                "agy produced empty JSON output".into()
            } else {
                t.to_string()
            }
        };
        return ParsedAntigravity {
            text: String::new(),
            native_id: String::new(),
            result: Value::String(stdout.to_string()),
            error: Some(detail),
            usage: None,
        };
    }
    let Ok(payload) = serde_json::from_str::<Value>(trimmed) else {
        let detail = {
            let t = stderr.trim();
            if t.is_empty() {
                "agy produced invalid JSON output".into()
            } else {
                t.to_string()
            }
        };
        return ParsedAntigravity {
            text: String::new(),
            native_id: String::new(),
            result: Value::String(stdout.to_string()),
            error: Some(detail),
            usage: None,
        };
    };
    let text = payload
        .get("response")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let native_id = payload
        .get("conversation_id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let payload_error = payload
        .get("error")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("")
        .to_string();
    let status = payload.get("status").and_then(Value::as_str).unwrap_or("");
    let usage = usage_from_record(payload.get("usage"), None, None);
    if status == "ERROR" {
        let detail = if !payload_error.is_empty() {
            payload_error
        } else {
            let t = stderr.trim();
            if t.is_empty() {
                "agy reported a run error".into()
            } else {
                t.to_string()
            }
        };
        return ParsedAntigravity {
            text,
            native_id,
            result: payload,
            error: Some(detail),
            usage,
        };
    }
    if native_id.is_empty() {
        let detail = if !payload_error.is_empty() {
            payload_error
        } else {
            let t = stderr.trim();
            if t.is_empty() {
                "agy JSON missing conversation_id".into()
            } else {
                t.to_string()
            }
        };
        return ParsedAntigravity {
            text,
            native_id: String::new(),
            result: payload,
            error: Some(detail),
            usage,
        };
    }
    ParsedAntigravity {
        text,
        native_id,
        result: payload,
        error: None,
        usage,
    }
}

pub fn run_antigravity(request: &AgentRequest) -> Result<AgentResult, QcError> {
    let args = build_antigravity_args(request);
    let capture = spawn_agent(
        &request.command,
        &args,
        None,
        &request.workdir,
        &request.env,
        "antigravity",
    )?;
    let parsed = parse_antigravity_output(&capture.stdout, &capture.stderr);
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
