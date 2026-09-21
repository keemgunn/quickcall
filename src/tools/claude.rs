use serde_json::Value;

use crate::errors::QcError;
use crate::tools::spawn::spawn_agent;
use crate::tools::types::{
    AgentRequest, AgentResult, AgentUsage, CLAUDE_EFFORT, usage_from_record, utf16_units,
};

const CLAUDE_UNRECOGNIZED_MODEL: &str = "[claude-code:unrecognized_model]";

pub struct ClaudeArgs {
    pub args: Vec<String>,
    pub create_uuid: Option<String>,
}

fn os_random_16() -> [u8; 16] {
    let mut buf = [0u8; 16];
    // musl libc crate does not export getentropy; /dev/urandom is the fallback on every target.
    #[cfg(not(target_env = "musl"))]
    {
        // SAFETY: getentropy fills 16 bytes or fails; fallback is /dev/urandom.
        let rc = unsafe { libc::getentropy(buf.as_mut_ptr().cast(), buf.len()) };
        if rc == 0 {
            return buf;
        }
    }
    let mut file = std::fs::File::open("/dev/urandom").expect("OS randomness");
    use std::io::Read;
    file.read_exact(&mut buf).expect("OS randomness");
    buf
}

fn random_uuid_v4() -> String {
    let mut bytes = os_random_16();
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0],
        bytes[1],
        bytes[2],
        bytes[3],
        bytes[4],
        bytes[5],
        bytes[6],
        bytes[7],
        bytes[8],
        bytes[9],
        bytes[10],
        bytes[11],
        bytes[12],
        bytes[13],
        bytes[14],
        bytes[15]
    )
}

pub fn build_claude_args(request: &AgentRequest) -> ClaudeArgs {
    let mut args = vec![
        "-p".into(),
        "--dangerously-skip-permissions".into(),
        "--output-format".into(),
        "json".into(),
    ];
    if let Some(model) = &request.model {
        args.push("--model".into());
        args.push(model.clone());
    }
    if let Some(thinking) = &request.thinking
        && CLAUDE_EFFORT.contains(&thinking.as_str())
    {
        args.push("--effort".into());
        args.push(thinking.clone());
    }
    if let Some(native) = &request.native_id {
        args.push("--resume".into());
        args.push(native.clone());
        args.push(request.prompt.clone());
        return ClaudeArgs {
            args,
            create_uuid: None,
        };
    }
    let create_uuid = random_uuid_v4();
    args.push("--session-id".into());
    args.push(create_uuid.clone());
    args.push("--name".into());
    args.push(request.session_id.clone());
    args.push(request.prompt.clone());
    ClaudeArgs {
        args,
        create_uuid: Some(create_uuid),
    }
}

fn claude_run_error_detail(payload: &Value, stderr: &str) -> String {
    if let Some(text) = payload
        .get("error")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        return text.to_string();
    }
    if let Some(text) = payload
        .get("result")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        return text.to_string();
    }
    let trimmed = stderr.trim();
    if !trimmed.is_empty() {
        return trimmed.to_string();
    }
    "claude reported a run error".into()
}

fn claude_reported_run_error(payload: &Value, stderr: &str) -> bool {
    if payload.get("is_error").and_then(Value::as_bool) == Some(true) {
        return true;
    }
    if payload
        .get("subtype")
        .and_then(Value::as_str)
        .is_some_and(|s| s.starts_with("error"))
    {
        return true;
    }
    if payload
        .get("errors")
        .and_then(Value::as_array)
        .is_some_and(|a| !a.is_empty())
    {
        return true;
    }
    stderr.contains(CLAUDE_UNRECOGNIZED_MODEL)
}

pub struct ParsedClaude {
    pub text: String,
    pub native_id: String,
    pub result: Value,
    pub error: Option<String>,
    pub usage: Option<AgentUsage>,
}

pub fn parse_claude_output(
    stdout: &str,
    fallback_native_id: &str,
    stderr: &str,
) -> Result<ParsedClaude, QcError> {
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return Ok(ParsedClaude {
            text: String::new(),
            native_id: fallback_native_id.to_string(),
            result: Value::String(stdout.to_string()),
            error: Some("claude produced empty JSON output".into()),
            usage: None,
        });
    }
    let Ok(payload) = serde_json::from_str::<Value>(trimmed) else {
        return Ok(ParsedClaude {
            text: String::new(),
            native_id: fallback_native_id.to_string(),
            result: Value::String(stdout.to_string()),
            error: Some("claude produced invalid JSON output".into()),
            usage: None,
        });
    };
    let native_id = payload
        .get("session_id")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .unwrap_or(fallback_native_id)
        .to_string();
    let text = payload
        .get("result")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let extras_cost = payload.get("total_cost_usd").and_then(Value::as_f64);
    let extras_currency = extras_cost.map(|_| "USD");
    let usage = usage_from_record(payload.get("usage"), extras_cost, extras_currency);
    if claude_reported_run_error(&payload, stderr) {
        let detail = claude_run_error_detail(&payload, stderr);
        return Ok(ParsedClaude {
            text,
            native_id,
            result: payload,
            error: Some(detail),
            usage,
        });
    }
    if native_id.is_empty() {
        return Err(QcError::new("claude JSON missing session_id"));
    }
    Ok(ParsedClaude {
        text,
        native_id,
        result: payload,
        error: None,
        usage,
    })
}

pub fn run_claude(request: &AgentRequest) -> Result<AgentResult, QcError> {
    let built = build_claude_args(request);
    let capture = spawn_agent(
        &request.command,
        &built.args,
        None,
        &request.workdir,
        &request.env,
        "claude",
    )?;
    let fallback = request
        .native_id
        .clone()
        .or(built.create_uuid)
        .unwrap_or_default();
    let parsed = parse_claude_output(&capture.stdout, &fallback, &capture.stderr)?;
    Ok(AgentResult {
        text: parsed.text,
        native_id: parsed.native_id,
        exit: capture.exit,
        result: parsed.result,
        stderr: capture.stderr,
        argv: built.args,
        stdout_bytes: utf16_units(&capture.stdout),
        error: parsed.error,
        usage: parsed.usage,
    })
}
