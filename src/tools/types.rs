use std::collections::HashMap;

use serde_json::Value;

/// Shared tool identity.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToolName {
    Pi,
    Cursor,
    Claude,
    Opencode,
    Antigravity,
    Codex,
}

pub const TOOL_NAMES: [ToolName; 6] = [
    ToolName::Pi,
    ToolName::Cursor,
    ToolName::Claude,
    ToolName::Opencode,
    ToolName::Antigravity,
    ToolName::Codex,
];

/// Claude `--effort` values. Others warn+ignore in settings.
pub const CLAUDE_EFFORT: &[&str] = &["low", "medium", "high", "xhigh", "max"];

/// Antigravity `--effort` values. Others warn+ignore in settings.
pub const AGY_EFFORT: &[&str] = &["low", "medium", "high"];

/// Codex qc-side thinking tokens. `off` maps to Codex `none` in the adapter.
pub const CODEX_EFFORT: &[&str] = &["off", "minimal", "low", "medium", "high", "xhigh", "max"];

impl ToolName {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pi => "pi",
            Self::Cursor => "cursor",
            Self::Claude => "claude",
            Self::Opencode => "opencode",
            Self::Antigravity => "antigravity",
            Self::Codex => "codex",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "pi" => Some(Self::Pi),
            "cursor" => Some(Self::Cursor),
            "claude" => Some(Self::Claude),
            "opencode" => Some(Self::Opencode),
            "antigravity" => Some(Self::Antigravity),
            "codex" => Some(Self::Codex),
            _ => None,
        }
    }

    /// Built-in executable when config omits `[tool.<name>] command`.
    pub fn default_command(self) -> &'static str {
        match self {
            Self::Pi => "pi",
            Self::Cursor => "agent",
            Self::Claude => "claude",
            Self::Opencode => "opencode",
            Self::Antigravity => "agy",
            Self::Codex => "codex",
        }
    }
}

impl std::fmt::Display for ToolName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

pub fn is_tool_name(value: &str) -> bool {
    ToolName::parse(value).is_some()
}

/// Callers pass resolved settings + prompt; adapters own argv and JSON.
#[derive(Debug, Clone)]
pub struct AgentRequest {
    pub prompt: String,
    pub model: Option<String>,
    pub thinking: Option<String>,
    pub workdir: String,
    /// qc pretty session id (Pi `--session-id`, Claude `--name`, OpenCode `--title`).
    pub session_id: String,
    /// Native tool session id when continuing; None on create.
    pub native_id: Option<String>,
    pub no_skills: Option<bool>,
    pub skill_path: Option<String>,
    pub command: String,
    pub env: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AgentResult {
    pub text: String,
    pub native_id: String,
    pub exit: i32,
    pub result: Value,
    pub stderr: String,
    pub argv: Vec<String>,
    /// UTF-16 code units of captured stdout (Node `string.length`), not UTF-8 bytes.
    pub stdout_bytes: usize,
    pub error: Option<String>,
    pub usage: Option<AgentUsage>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct AgentUsage {
    pub input_tokens: Option<f64>,
    pub output_tokens: Option<f64>,
    pub cache_read_tokens: Option<f64>,
    pub thinking_tokens: Option<f64>,
    pub total_tokens: Option<f64>,
    pub cost: Option<f64>,
    pub cost_currency: Option<String>,
}

impl AgentUsage {
    pub fn is_empty(&self) -> bool {
        self.input_tokens.is_none()
            && self.output_tokens.is_none()
            && self.cache_read_tokens.is_none()
            && self.thinking_tokens.is_none()
            && self.total_tokens.is_none()
            && self.cost.is_none()
            && self.cost_currency.is_none()
    }
}

fn json_number(value: &Value) -> Option<f64> {
    value.as_f64()
}

/// Lift standard token/cost fields from a provider JSON object.
pub fn usage_from_record(
    raw: Option<&Value>,
    extras_cost: Option<f64>,
    extras_currency: Option<&str>,
) -> Option<AgentUsage> {
    let mut out = AgentUsage {
        cost: extras_cost,
        cost_currency: extras_currency
            .filter(|s| !s.is_empty())
            .map(str::to_string),
        ..AgentUsage::default()
    };
    if let Some(u) = raw.and_then(Value::as_object) {
        if let Some(n) = u.get("input_tokens").and_then(json_number) {
            out.input_tokens = Some(n);
        } else if let Some(n) = u.get("input").and_then(json_number) {
            out.input_tokens = Some(n);
        }
        if let Some(n) = u.get("output_tokens").and_then(json_number) {
            out.output_tokens = Some(n);
        } else if let Some(n) = u.get("output").and_then(json_number) {
            out.output_tokens = Some(n);
        }
        if let Some(n) = u.get("cache_read_tokens").and_then(json_number) {
            out.cache_read_tokens = Some(n);
        } else if let Some(n) = u.get("cache_read_input_tokens").and_then(json_number) {
            out.cache_read_tokens = Some(n);
        } else if let Some(n) = u.get("cacheRead").and_then(json_number) {
            out.cache_read_tokens = Some(n);
        }
        if let Some(n) = u.get("thinking_tokens").and_then(json_number) {
            out.thinking_tokens = Some(n);
        } else if let Some(n) = u.get("reasoning").and_then(json_number) {
            out.thinking_tokens = Some(n);
        }
        if let Some(n) = u.get("total_tokens").and_then(json_number) {
            out.total_tokens = Some(n);
        } else if let Some(n) = u.get("totalTokens").and_then(json_number) {
            out.total_tokens = Some(n);
        }
        if let Some(n) = u.get("cost").and_then(json_number) {
            out.cost = Some(n);
        } else if let Some(n) = u
            .get("cost")
            .and_then(Value::as_object)
            .and_then(|c| c.get("total"))
            .and_then(json_number)
        {
            out.cost = Some(n);
        }
        if let Some(s) = u
            .get("cost_currency")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
        {
            out.cost_currency = Some(s.to_string());
        }
    }
    if out.is_empty() { None } else { Some(out) }
}

/// Node `string.length`: UTF-16 code units after UTF-8 decode.
pub fn utf16_units(text: &str) -> usize {
    text.encode_utf16().count()
}
