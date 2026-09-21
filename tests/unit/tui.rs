use quickcall::tools::ToolName;
use quickcall::tools::tui::{
    build_antigravity_tui_args, build_claude_tui_args, build_codex_tui_args, build_cursor_tui_args,
    build_opencode_tui_args, build_pi_tui_args, build_tui_args,
};

const FORBIDDEN: &[&str] = &[
    "--mode",
    "json",
    "-p",
    "--print",
    "-a",
    "--yolo",
    "--dangerously-skip-permissions",
    "--auto",
    "--trust",
    "--force",
    "--approve-mcps",
    "--model",
    "--thinking",
    "--workspace",
    "--dir",
    "--add-dir",
    "--output-format",
    "run",
    "--json",
    "--cd",
    "exec",
    "-c",
    "--dangerously-bypass-approvals-and-sandbox",
    "--dangerously-bypass-hook-trust",
];

#[test]
fn builds_per_tool_tui_resume_argv_without_json_force_allow_model_thinking_or_workspace_flags() {
    let native_id = "native-session-1";
    assert_eq!(build_pi_tui_args(native_id), ["--session-id", native_id]);
    assert_eq!(build_cursor_tui_args(native_id), ["--resume", native_id]);
    assert_eq!(build_claude_tui_args(native_id), ["--resume", native_id]);
    assert_eq!(build_opencode_tui_args(native_id), ["-s", native_id]);
    assert_eq!(
        build_antigravity_tui_args(native_id),
        ["--conversation", native_id]
    );
    assert_eq!(build_codex_tui_args(native_id), ["resume", native_id]);
    assert_eq!(
        build_tui_args(ToolName::Pi, native_id).expect("pi"),
        build_pi_tui_args(native_id)
    );
    assert_eq!(
        build_tui_args(ToolName::Cursor, native_id).expect("cursor"),
        build_cursor_tui_args(native_id)
    );
    assert_eq!(
        build_tui_args(ToolName::Claude, native_id).expect("claude"),
        build_claude_tui_args(native_id)
    );
    assert_eq!(
        build_tui_args(ToolName::Opencode, native_id).expect("opencode"),
        build_opencode_tui_args(native_id)
    );
    assert_eq!(
        build_tui_args(ToolName::Antigravity, native_id).expect("agy"),
        build_antigravity_tui_args(native_id)
    );
    assert_eq!(
        build_tui_args(ToolName::Codex, native_id).expect("codex"),
        build_codex_tui_args(native_id)
    );
    for args in [
        build_pi_tui_args(native_id),
        build_cursor_tui_args(native_id),
        build_claude_tui_args(native_id),
        build_opencode_tui_args(native_id),
        build_antigravity_tui_args(native_id),
        build_codex_tui_args(native_id),
    ] {
        assert!(
            !args.iter().any(|token| FORBIDDEN.contains(&token.as_str())),
            "{args:?}"
        );
    }
}
