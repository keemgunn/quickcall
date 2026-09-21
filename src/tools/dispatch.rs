use crate::errors::QcError;
use crate::tools::antigravity::run_antigravity;
use crate::tools::claude::run_claude;
use crate::tools::codex::run_codex;
use crate::tools::cursor::run_cursor;
use crate::tools::opencode::run_opencode;
use crate::tools::pi::run_pi;
use crate::tools::types::{AgentRequest, AgentResult, ToolName};

/// Deep seam: one call site for every headless JSON adapter.
pub fn run_agent(tool: ToolName, request: &AgentRequest) -> Result<AgentResult, QcError> {
    match tool {
        ToolName::Pi => run_pi(request),
        ToolName::Cursor => run_cursor(request),
        ToolName::Claude => run_claude(request),
        ToolName::Opencode => run_opencode(request),
        ToolName::Antigravity => run_antigravity(request),
        ToolName::Codex => run_codex(request),
    }
}
