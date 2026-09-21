use crate::errors::QcError;
use crate::tools::types::ToolName;

/// Interactive resume only: no JSON, force-allow, model, thinking, or workspace flags.
pub fn build_pi_tui_args(native_id: &str) -> Vec<String> {
    vec!["--session-id".into(), native_id.to_string()]
}

pub fn build_cursor_tui_args(native_id: &str) -> Vec<String> {
    vec!["--resume".into(), native_id.to_string()]
}

pub fn build_claude_tui_args(native_id: &str) -> Vec<String> {
    vec!["--resume".into(), native_id.to_string()]
}

pub fn build_opencode_tui_args(native_id: &str) -> Vec<String> {
    vec!["-s".into(), native_id.to_string()]
}

pub fn build_antigravity_tui_args(native_id: &str) -> Vec<String> {
    vec!["--conversation".into(), native_id.to_string()]
}

pub fn build_codex_tui_args(native_id: &str) -> Vec<String> {
    vec!["resume".into(), native_id.to_string()]
}

pub fn build_tui_args(tool: ToolName, native_id: &str) -> Result<Vec<String>, QcError> {
    Ok(match tool {
        ToolName::Pi => build_pi_tui_args(native_id),
        ToolName::Cursor => build_cursor_tui_args(native_id),
        ToolName::Claude => build_claude_tui_args(native_id),
        ToolName::Opencode => build_opencode_tui_args(native_id),
        ToolName::Antigravity => build_antigravity_tui_args(native_id),
        ToolName::Codex => build_codex_tui_args(native_id),
    })
}
