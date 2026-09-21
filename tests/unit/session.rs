use std::os::unix::fs::symlink;

use crate::common::{unique_scratch, write_file};
use chrono::{Local, TimeZone};
use quickcall::session::{
    SessionMapping, cwd_key, find_latest_session, load_session, mint_session_id, parse_session_id,
    save_session, session_path,
};
use quickcall::tools::ToolName;

#[test]
fn mints_local_time_pretty_ids_with_tool_and_6_char_suffix() {
    let now = Local.with_ymd_and_hms(2026, 9, 1, 17, 57, 0).unwrap();
    let id = mint_session_id(ToolName::Pi, Some(now));
    assert!(id.starts_with("260901-1757--pi--"), "{id}");
    assert_eq!(id.len(), "260901-1757--pi--".len() + 6);
    assert!(
        id[id.len() - 6..]
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit()),
        "{id}"
    );
    let parsed = parse_session_id(&id).expect("parse");
    assert_eq!(parsed.tool, ToolName::Pi);
    assert_eq!(parsed.stamp, "260901-1757");

    let codex = mint_session_id(ToolName::Codex, Some(now));
    assert!(codex.starts_with("260901-1757--codex--"), "{codex}");
    assert_eq!(
        parse_session_id(&codex).expect("codex").tool,
        ToolName::Codex
    );
}

#[test]
fn rejects_invalid_session_ids() {
    let bad = parse_session_id("bad").expect_err("bad");
    assert!(
        bad.message.contains("invalid session id"),
        "{}",
        bad.message
    );
    let tool = parse_session_id("260901-1757--nope--abcdef").expect_err("tool");
    assert!(
        tool.message.contains("invalid session id tool"),
        "{}",
        tool.message
    );
}

#[test]
fn saves_and_loads_session_mappings_under_qc_sessions() {
    let scratch = unique_scratch("session");
    let home = scratch.home();
    let id = "260901-1757--cursor--a1b2c3";
    save_session(
        &home,
        id,
        &SessionMapping {
            tool: ToolName::Cursor,
            native_id: "uuid-1".into(),
            cwd: "/work".into(),
            created: "2026-09-01T00:00:00.000Z".into(),
            updated: "2026-09-01T00:01:00.000Z".into(),
            warnings: vec!["qc_thinking ignored".into()],
        },
    )
    .expect("save");
    let raw = std::fs::read_to_string(session_path(&home, id)).expect("read");
    assert_eq!(
        raw,
        "{\n  \"tool\": \"cursor\",\n  \"native_id\": \"uuid-1\",\n  \"cwd\": \"/work\",\n  \"created\": \"2026-09-01T00:00:00.000Z\",\n  \"updated\": \"2026-09-01T00:01:00.000Z\",\n  \"warnings\": [\n    \"qc_thinking ignored\"\n  ]\n}\n"
    );
    let loaded = load_session(&home, id).expect("load");
    assert_eq!(loaded.tool, ToolName::Cursor);
    assert_eq!(loaded.native_id, "uuid-1");
    let missing = load_session(&home, "260901-1757--pi--zzzzzz").expect_err("missing");
    assert!(
        missing.message.contains("session not found"),
        "{}",
        missing.message
    );
}

#[test]
fn cwd_key_resolves_then_realpaths_existing_paths_and_does_not_walk_ancestors() {
    let scratch = unique_scratch("cwd-key");
    let real = scratch.root.join("real");
    let link = scratch.root.join("link");
    std::fs::create_dir(&real).expect("real");
    symlink(&real, &link).expect("link");
    let real_key = std::fs::canonicalize(&real)
        .expect("canon")
        .to_string_lossy()
        .into_owned();
    assert_eq!(cwd_key(link.to_str().expect("utf8")), real_key);
    assert_eq!(
        cwd_key(real.to_str().expect("utf8")),
        cwd_key(link.to_str().expect("utf8"))
    );
    let missing = scratch.root.join("missing").join("dir");
    assert_eq!(
        cwd_key(missing.to_str().expect("utf8")),
        missing.to_string_lossy()
    );
    assert_ne!(
        cwd_key(real.join("child").to_str().expect("utf8")),
        cwd_key(real.to_str().expect("utf8"))
    );
}

#[test]
fn find_latest_session_picks_newest_updated_then_created_then_id_for_the_invocation_cwd() {
    let scratch = unique_scratch("latest-session");
    let home = scratch.home();
    let cwd = scratch.cwd();
    let older = "260901-1000--pi--aaaaaa";
    let newer = "260901-1000--pi--bbbbbb";
    let other_dir = "260901-1000--pi--cccccc";
    let mapping =
        |native: &str, cwd: &std::path::Path, created: &str, updated: &str| SessionMapping {
            tool: ToolName::Pi,
            native_id: native.into(),
            cwd: cwd.to_string_lossy().into_owned(),
            created: created.into(),
            updated: updated.into(),
            warnings: vec![],
        };
    save_session(
        &home,
        older,
        &mapping(
            "old-native",
            &cwd,
            "2026-09-01T02:00:00.000Z",
            "2026-09-01T03:00:00.000Z",
        ),
    )
    .expect("older");
    save_session(
        &home,
        newer,
        &mapping(
            "new-native",
            &cwd,
            "2026-09-01T01:00:00.000Z",
            "2026-09-01T04:00:00.000Z",
        ),
    )
    .expect("newer");
    save_session(
        &home,
        other_dir,
        &mapping(
            "elsewhere",
            &scratch.root.join("other"),
            "2026-09-01T09:00:00.000Z",
            "2026-09-01T09:00:00.000Z",
        ),
    )
    .expect("other");
    let found = find_latest_session(&home, cwd.to_str().expect("utf8"), None).expect("latest");
    assert_eq!(found.id, newer);
    assert_eq!(found.mapping.native_id, "new-native");
}

#[test]
fn find_latest_session_skips_corrupt_files_on_scan_and_filters_by_tool() {
    let scratch = unique_scratch("latest-skip");
    let home = scratch.home();
    let cwd = scratch.cwd();
    let pi_id = "260901-1200--pi--aaaaaa";
    let cursor_id = "260901-1200--cursor--bbbbbb";
    let mapping = |tool: ToolName, native: &str, created: &str| SessionMapping {
        tool,
        native_id: native.into(),
        cwd: cwd.to_string_lossy().into_owned(),
        created: created.into(),
        updated: created.into(),
        warnings: vec![],
    };
    save_session(
        &home,
        pi_id,
        &mapping(ToolName::Pi, "pi-native", "2026-09-01T00:00:00.000Z"),
    )
    .expect("pi");
    save_session(
        &home,
        cursor_id,
        &mapping(
            ToolName::Cursor,
            "cursor-native",
            "2026-09-01T01:00:00.000Z",
        ),
    )
    .expect("cursor");
    let dir = home.join(".qc").join("sessions");
    write_file(&dir.join("260901-1200--pi--zzzzzz.json"), "{not json");
    write_file(&dir.join("not-a-session.json"), "{\"tool\":\"pi\"}\n");
    let latest = find_latest_session(&home, cwd.to_str().expect("utf8"), None).expect("any");
    assert_eq!(latest.id, cursor_id);
    let pi =
        find_latest_session(&home, cwd.to_str().expect("utf8"), Some(ToolName::Pi)).expect("pi");
    assert_eq!(pi.id, pi_id);
    let claude = find_latest_session(&home, cwd.to_str().expect("utf8"), Some(ToolName::Claude))
        .expect_err("claude");
    assert!(
        claude
            .message
            .contains("no claude session found for this directory"),
        "{}",
        claude.message
    );
    let empty = find_latest_session(
        &home,
        scratch.root.join("empty").to_str().expect("utf8"),
        None,
    )
    .expect_err("empty");
    assert!(
        empty
            .message
            .contains("no session found for this directory"),
        "{}",
        empty.message
    );
}

#[test]
fn load_session_fails_closed_on_malformed_mappings_while_scan_still_skips_them() {
    let scratch = unique_scratch("session-malformed");
    let home = scratch.home();
    let cwd = scratch.cwd();
    let valid_id = "260901-1200--pi--aaaaaa";
    save_session(
        &home,
        valid_id,
        &SessionMapping {
            tool: ToolName::Pi,
            native_id: "pi-native".into(),
            cwd: cwd.to_string_lossy().into_owned(),
            created: "2026-09-01T00:00:00.000Z".into(),
            updated: "2026-09-01T00:00:00.000Z".into(),
            warnings: vec![],
        },
    )
    .expect("valid");
    let dir = home.join(".qc").join("sessions");

    let corrupt_id = "260901-1200--pi--bbbbbb";
    write_file(&dir.join(format!("{corrupt_id}.json")), "{not json");
    let corrupt = load_session(&home, corrupt_id).expect_err("corrupt");
    assert_eq!(
        corrupt.message,
        "corrupt session mapping: 260901-1200--pi--bbbbbb"
    );

    let empty_native = "260901-1200--pi--cccccc";
    write_file(
        &dir.join(format!("{empty_native}.json")),
        &format!(
            "{}\n",
            serde_json::json!({
                "tool": "pi",
                "native_id": "",
                "cwd": cwd,
                "created": "t",
                "updated": "t",
                "warnings": []
            })
        ),
    );
    let empty = load_session(&home, empty_native).expect_err("empty native");
    assert_eq!(
        empty.message,
        format!("corrupt session mapping: {empty_native}")
    );

    let bad_tool = "260901-1200--pi--dddddd";
    write_file(
        &dir.join(format!("{bad_tool}.json")),
        &format!(
            "{}\n",
            serde_json::json!({
                "tool": "nope",
                "native_id": "x",
                "cwd": cwd,
                "created": "t",
                "updated": "t",
                "warnings": []
            })
        ),
    );
    let bad = load_session(&home, bad_tool).expect_err("bad tool");
    assert_eq!(bad.message, format!("corrupt session mapping: {bad_tool}"));

    let empty_cwd = "260901-1200--pi--eeeeee";
    write_file(
        &dir.join(format!("{empty_cwd}.json")),
        "{\n  \"tool\": \"pi\",\n  \"native_id\": \"x\",\n  \"cwd\": \"\",\n  \"created\": \"t\",\n  \"updated\": \"t\",\n  \"warnings\": []\n}\n",
    );
    let cwd_err = load_session(&home, empty_cwd).expect_err("empty cwd");
    assert_eq!(
        cwd_err.message,
        format!("corrupt session mapping: {empty_cwd}")
    );

    let coerced = "260901-1200--pi--ffffff";
    write_file(
        &dir.join(format!("{coerced}.json")),
        &format!(
            "{}\n",
            serde_json::json!({
                "tool": "pi",
                "native_id": "n",
                "cwd": cwd,
                "warnings": [1, false]
            })
        ),
    );
    let loaded = load_session(&home, coerced).expect("coerced");
    assert_eq!(loaded.tool, ToolName::Pi);
    assert_eq!(loaded.native_id, "n");
    assert_eq!(loaded.cwd, cwd.to_string_lossy());
    assert_eq!(loaded.created, "");
    assert_eq!(loaded.updated, "");
    assert_eq!(loaded.warnings, ["1".to_string(), "false".to_string()]);

    let found = find_latest_session(&home, cwd.to_str().expect("utf8"), None).expect("scan");
    assert_eq!(found.id, valid_id);
}

#[test]
fn save_round_trips_legacy_shaped_fixture_with_empty_warnings_array() {
    let scratch = unique_scratch("session-legacy");
    let home = scratch.home();
    let id = "260910-0300--pi--abc123";
    let mapping = SessionMapping {
        tool: ToolName::Pi,
        native_id: "provider-native-id".into(),
        cwd: "/absolute/example".into(),
        created: "2026-09-10T03:00:00.000Z".into(),
        updated: "2026-09-10T03:00:00.000Z".into(),
        warnings: vec![],
    };
    save_session(&home, id, &mapping).expect("save");
    let raw = std::fs::read_to_string(session_path(&home, id)).expect("raw");
    assert_eq!(
        raw,
        "{\n  \"tool\": \"pi\",\n  \"native_id\": \"provider-native-id\",\n  \"cwd\": \"/absolute/example\",\n  \"created\": \"2026-09-10T03:00:00.000Z\",\n  \"updated\": \"2026-09-10T03:00:00.000Z\",\n  \"warnings\": []\n}\n"
    );
    assert_eq!(load_session(&home, id).expect("load"), mapping);
}
