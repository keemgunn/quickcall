use serde_json::Value;

use crate::errors::QcError;
use crate::tools::spawn::spawn_agent;
use crate::tools::types::{AgentRequest, AgentResult, AgentUsage, usage_from_record, utf16_units};

pub fn build_cursor_args(request: &AgentRequest) -> Vec<String> {
    let mut args = vec![
        "-p".into(),
        "--force".into(),
        "--yolo".into(),
        "--approve-mcps".into(),
        "--trust".into(),
        "--output-format".into(),
        "json".into(),
        "--workspace".into(),
        request.workdir.clone(),
    ];
    if let Some(model) = &request.model {
        args.push("--model".into());
        args.push(model.clone());
    }
    if let Some(native) = &request.native_id {
        args.push("--resume".into());
        args.push(native.clone());
    }
    args.push(request.prompt.clone());
    args
}

pub struct ParsedCursor {
    pub text: String,
    pub native_id: String,
    pub result: Value,
    pub error: Option<String>,
    pub usage: Option<AgentUsage>,
}

pub fn parse_cursor_output(stdout: &str) -> ParsedCursor {
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return ParsedCursor {
            text: String::new(),
            native_id: String::new(),
            result: Value::String(stdout.to_string()),
            error: Some("cursor agent produced empty JSON output".into()),
            usage: None,
        };
    }
    let Ok(payload) = serde_json::from_str::<Value>(trimmed) else {
        return ParsedCursor {
            text: String::new(),
            native_id: String::new(),
            result: Value::String(stdout.to_string()),
            error: Some("cursor agent produced invalid JSON output".into()),
            usage: None,
        };
    };
    let text = payload
        .get("result")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let native_id = payload
        .get("session_id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let usage = usage_from_record(payload.get("usage"), None, None);
    if native_id.is_empty() {
        return ParsedCursor {
            text,
            native_id: String::new(),
            result: payload,
            error: Some("cursor agent JSON missing session_id".into()),
            usage,
        };
    }
    ParsedCursor {
        text,
        native_id,
        result: payload,
        error: None,
        usage,
    }
}

pub fn run_cursor(request: &AgentRequest) -> Result<AgentResult, QcError> {
    let args = build_cursor_args(request);
    let capture = spawn_agent(
        &request.command,
        &args,
        None,
        &request.workdir,
        &request.env,
        "cursor",
    )?;
    let parsed = parse_cursor_output(&capture.stdout);
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
