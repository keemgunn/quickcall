use serde_json::{Map, Value};

use crate::tools::types::{AgentUsage, utf16_units};

/// Exact TypeScript `messages.help` body. CLI stdout adds one trailing newline.
pub const HELP: &str = "\
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

pub fn error(detail: &str) -> String {
    format!("[ERROR] {detail}")
}

pub fn warn(detail: &str) -> String {
    format!("[WARNING] {detail}")
}

pub fn debug_line(detail: &str) -> String {
    format!("[DEBUG] {detail}")
}

pub fn session_line(id: &str) -> String {
    format!("[QC-SESSION] {id}")
}

pub fn open_session_command(id: &str) -> String {
    format!("qc -o {id}")
}

const FAIL_STDERR_MAX: usize = 8192;

fn utf16_tail(text: &str, max_units: usize) -> String {
    let units: Vec<u16> = text.encode_utf16().collect();
    if units.len() <= max_units {
        return text.to_string();
    }
    String::from_utf16_lossy(&units[units.len() - max_units..])
}

/// Tagged fail reason, then child stderr when it is not already that reason.
pub fn fail_output(primary: &str, child_stderr: &str) -> String {
    let mut extra = child_stderr.trim().to_string();
    if utf16_units(&extra) > FAIL_STDERR_MAX {
        extra = utf16_tail(&extra, FAIL_STDERR_MAX);
    }
    if extra.is_empty() || extra == primary || primary.contains(&extra) {
        return format!("{}\n", error(primary));
    }
    format!("{}\n{extra}\n", error(primary))
}

/// Copy-paste native binary for live fails with no saved mapping.
pub fn inspect_command(command: &str) -> String {
    if command.chars().any(|ch| ch.is_whitespace() || ch == '\'') {
        return format!("'{}'", command.replace('\'', r#"'\''"#));
    }
    command.to_string()
}

/// Spinner timer suffix: 000s, 012s, 999s, 1000s (width grows after 999).
pub fn elapsed_label(seconds: u64) -> String {
    format!("{seconds:03}s")
}

fn status_segments(tool: &str, elapsed: &str, model: Option<&str>) -> String {
    if let Some(model) = model.filter(|text| !text.is_empty()) {
        format!("{tool} ⋅ {model} ⋅ {elapsed}")
    } else {
        format!("{tool} ⋅ {elapsed}")
    }
}

/// Live spinner text after the bouncing bar.
pub fn wait_line(tool: &str, seconds: u64, model: Option<&str>) -> String {
    format!(
        " ⋅ {}",
        status_segments(tool, &elapsed_label(seconds), model)
    )
}

pub fn quoted_text(text: &str) -> String {
    let body = text.strip_suffix('\n').unwrap_or(text);
    format!("\"\"\"\n{body}\n\"\"\"")
}

pub fn success_line(tool: &str, duration_s: i64, model: Option<&str>) -> String {
    format!(
        "[SUCCESS] {}",
        status_segments(tool, &format!("{duration_s}s"), model)
    )
}

/// JS `String(n)` for finite numbers used in usage text.
fn js_number_string(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() <= 9_007_199_254_740_991.0 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

pub fn compact_tokens(count: f64) -> String {
    if count.abs() < 1000.0 {
        return js_number_string(count);
    }
    // JS `Number((count / 1000).toFixed(1))` — one decimal, trailing zero dropped.
    let tenths = (count / 100.0).round() / 10.0;
    format!("{}k", js_number_string(tenths))
}

fn json_number(n: f64) -> Value {
    if n.fract() == 0.0 && n.abs() <= i64::MAX as f64 {
        Value::from(n as i64)
    } else {
        serde_json::Number::from_f64(n)
            .map(Value::Number)
            .unwrap_or(Value::Null)
    }
}

/// Tagged usage line, or None when nothing reportable was present.
pub fn usage_line(usage: Option<&AgentUsage>) -> Option<String> {
    let usage = usage?;
    let mut parts: Vec<String> = Vec::new();
    if let Some(n) = usage.input_tokens {
        parts.push(format!("{} in", compact_tokens(n)));
    }
    if let Some(n) = usage.output_tokens {
        parts.push(format!("{} out", compact_tokens(n)));
    }
    if let Some(n) = usage.cache_read_tokens {
        parts.push(format!("{} cache", compact_tokens(n)));
    }
    if let Some(n) = usage.thinking_tokens {
        parts.push(format!("{} think", compact_tokens(n)));
    }
    if let Some(n) = usage.total_tokens {
        parts.push(format!("{} total", compact_tokens(n)));
    }
    if let Some(cost) = usage.cost.filter(|cost| *cost != 0.0) {
        let formatted = js_number_string(cost);
        match usage.cost_currency.as_deref() {
            Some("USD") => parts.push(format!("${formatted}")),
            Some(currency) => parts.push(format!("{formatted} {currency}")),
            None => parts.push(formatted),
        }
    }
    if parts.is_empty() {
        return None;
    }
    Some(format!("[USAGE] {}", parts.join(" ⋅ ")))
}

pub fn usage_fields(usage: Option<&AgentUsage>) -> Option<Map<String, Value>> {
    let usage = usage?;
    let mut out = Map::new();
    if let Some(n) = usage.input_tokens {
        out.insert("input_tokens".into(), json_number(n));
    }
    if let Some(n) = usage.output_tokens {
        out.insert("output_tokens".into(), json_number(n));
    }
    if let Some(n) = usage.cache_read_tokens {
        out.insert("cache_read_tokens".into(), json_number(n));
    }
    if let Some(n) = usage.thinking_tokens {
        out.insert("thinking_tokens".into(), json_number(n));
    }
    if let Some(n) = usage.total_tokens {
        out.insert("total_tokens".into(), json_number(n));
    }
    if let Some(n) = usage.cost {
        out.insert("cost".into(), json_number(n));
    }
    if let Some(currency) = usage
        .cost_currency
        .as_deref()
        .filter(|text| !text.is_empty())
    {
        out.insert("cost_currency".into(), Value::String(currency.to_string()));
    }
    if out.is_empty() { None } else { Some(out) }
}

pub struct TextEnvelopeInput<'a> {
    pub text: &'a str,
    pub tool: &'a str,
    pub duration_s: i64,
    pub session_id: &'a str,
    pub model: Option<&'a str>,
    pub warnings: &'a [String],
    pub debug_lines: Option<&'a [String]>,
    pub usage: Option<&'a AgentUsage>,
}

pub fn text_envelope(input: TextEnvelopeInput<'_>) -> String {
    let mut parts = vec![
        quoted_text(input.text),
        success_line(input.tool, input.duration_s, input.model),
    ];
    if let Some(usage) = usage_line(input.usage) {
        parts.push(usage);
    }
    parts.push(session_line(input.session_id));
    if let Some(debug_lines) = input.debug_lines {
        for warning in input.warnings {
            parts.push(warn(warning));
        }
        for line in debug_lines {
            parts.push(debug_line(line));
        }
    }
    format!("{}\n", parts.join("\n"))
}

pub fn renamed_config_key(old_key: &str, replacement: &str) -> String {
    format!("config key '{old_key}' was removed; use {replacement}")
}

pub fn renamed_frontmatter_key(old_key: &str, replacement: &str) -> String {
    format!("frontmatter key '{old_key}' was removed; use {replacement}")
}

pub fn missing_binary(tool: &str, command: &str) -> String {
    format!("{tool} executable '{command}' was not found on PATH. Install it and try again.")
}

pub fn empty_assistant_text(tool: &str) -> String {
    format!("{tool} produced empty assistant text")
}

pub fn interrupted(signal: i32) -> String {
    format!("interrupted (signal {signal})")
}

/// Replace the prompt token so `--debug` argv never dumps the user turn.
pub fn redact_argv(argv: &[String], prompt: &str) -> Vec<String> {
    argv.iter()
        .map(|token| {
            if token == prompt {
                "<prompt>".to_string()
            } else {
                token.clone()
            }
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
pub fn debug_facts(
    tool: &str,
    model: Option<&str>,
    command: &str,
    argv: &[String],
    prompt: &str,
    cwd: &str,
    exit: i32,
    stdout_bytes: usize,
    stderr: &str,
    native_id: &str,
    duration_s: i64,
) -> Vec<String> {
    let mut lines = vec![
        format!("tool={tool}"),
        format!("model={}", model.unwrap_or("")),
        format!("command={command}"),
        format!("argv={}", redact_argv(argv, prompt).join(" ")),
        format!("cwd={cwd}"),
        format!("child_exit={exit}"),
        format!(
            "stdout_bytes={stdout_bytes} stderr_bytes={}",
            utf16_units(stderr)
        ),
        format!("native_id={native_id}"),
        format!("elapsed_s={duration_s}"),
    ];
    if !stderr.is_empty() {
        let trimmed = stderr.strip_suffix('\n').unwrap_or(stderr);
        lines.push(format!("child_stderr:\n{trimmed}"));
    }
    lines
}

/// One pretty-printed object matching `JSON.stringify(envelope, null, 2)`.
#[allow(clippy::too_many_arguments)]
pub fn json_envelope(
    tool: &str,
    session_id: &str,
    native_id: &str,
    result: &Value,
    warnings: &[String],
    exit: i32,
    model: Option<&str>,
    duration_s: i64,
    usage: Option<&AgentUsage>,
    debug_lines: Option<&[String]>,
) -> String {
    let mut envelope = Map::new();
    envelope.insert("tool".into(), Value::String(tool.to_string()));
    envelope.insert("session_id".into(), Value::String(session_id.to_string()));
    envelope.insert("native_id".into(), Value::String(native_id.to_string()));
    envelope.insert("result".into(), result.clone());
    envelope.insert(
        "warnings".into(),
        Value::Array(warnings.iter().map(|w| Value::String(w.clone())).collect()),
    );
    envelope.insert("exit".into(), Value::from(exit));
    envelope.insert(
        "model".into(),
        match model {
            Some(value) => Value::String(value.to_string()),
            None => Value::Null,
        },
    );
    envelope.insert("duration_s".into(), Value::from(duration_s));
    if let Some(usage) = usage_fields(usage) {
        envelope.insert("usage".into(), Value::Object(usage));
    }
    if let Some(debug_lines) = debug_lines {
        envelope.insert(
            "debug".into(),
            Value::Array(
                debug_lines
                    .iter()
                    .map(|l| Value::String(l.clone()))
                    .collect(),
            ),
        );
    }
    format!(
        "{}\n",
        serde_json::to_string_pretty(&Value::Object(envelope)).unwrap_or_else(|_| "{}".into())
    )
}

/// Helper usage on stderr. No `[ERROR]` prefix — matches the TypeScript helpers.
pub const BOOTSTRAP_USAGE: &str = "\
usage: qc-bootstrap settings <repair|refresh>
       qc-bootstrap harness <refresh-known>";
