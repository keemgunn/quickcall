use std::fs;
use std::process::{Command, Output, Stdio};

use crate::common::{Scratch, install_fixture_names, stage_package, unique_scratch, write_file};

fn run_qc(
    qc: &std::path::Path,
    scratch: &Scratch,
    argv: &[&str],
    extra: &[(&str, &str)],
) -> Output {
    let mut cmd = Command::new(qc);
    cmd.args(argv)
        .current_dir(scratch.cwd())
        .env_clear()
        .env("HOME", scratch.home())
        .env("PATH", format!("{}:/usr/bin:/bin", scratch.bin().display()))
        .env("SHELL", "/bin/sh")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, value) in extra {
        cmd.env(key, value);
    }
    cmd.output().expect("staged qc")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn session_id_from(text: &str) -> String {
    let marker = "[QC-SESSION] ";
    let start = text.find(marker).expect("session") + marker.len();
    let rest = &text[start..];
    rest.split_whitespace().next().unwrap_or("").to_string()
}

#[test]
fn staged_help_and_version_repair_without_agent() {
    let scratch = unique_scratch("smoke-help");
    install_fixture_names(&scratch.bin());
    let qc = stage_package(&scratch, "0.0.0");
    let marker = scratch.root.join("pi-marker");
    let help = run_qc(
        &qc,
        &scratch,
        &["--help"],
        &[("QC_PI_MARKER", marker.to_str().unwrap())],
    );
    assert_eq!(help.status.code(), Some(0));
    assert!(stdout(&help).starts_with("Usage: qc"));
    assert!(!marker.exists());
    assert!(scratch.home().join(".qc/config.toml").is_file());
}

#[test]
fn append_only_prompt_quiet_json_fail_continue_and_open() {
    let scratch = unique_scratch("smoke-turn");
    install_fixture_names(&scratch.bin());
    fs::create_dir_all(scratch.cwd().join(".qc/prompts")).expect("prompts");
    write_file(&scratch.cwd().join(".qc/prompts/plain.md"), "Plain");
    let qc = stage_package(&scratch, "0.0.0");
    let record = scratch.root.join("record.json");

    let append = run_qc(
        &qc,
        &scratch,
        &["--append", "inspect this repo"],
        &[
            ("QC_RECORD", record.to_str().unwrap()),
            ("QC_AGENT_TEXT", "ok"),
        ],
    );
    assert_eq!(append.status.code(), Some(0), "stderr={}", stderr(&append));
    assert!(stdout(&append).contains("ok"));
    assert!(stdout(&append).contains("[QC-SESSION]"));
    let recorded: serde_json::Value =
        serde_json::from_slice(&fs::read(&record).expect("record")).expect("json");
    assert_eq!(recorded["stdin"], "inspect this repo");

    let prompt = run_qc(&qc, &scratch, &["plain"], &[("QC_AGENT_TEXT", "joke")]);
    assert_eq!(prompt.status.code(), Some(0));
    assert!(stdout(&prompt).starts_with("\"\"\"\njoke\n\"\"\"\n[SUCCESS] pi ⋅ "));
    assert_eq!(stderr(&prompt), "");

    let quiet = run_qc(
        &qc,
        &scratch,
        &["-q", "plain"],
        &[("QC_AGENT_TEXT", "joke")],
    );
    assert_eq!(quiet.status.code(), Some(0));
    assert_eq!(stderr(&quiet), "");

    let json = run_qc(
        &qc,
        &scratch,
        &["plain", "--output", "json"],
        &[("QC_AGENT_TEXT", "joke")],
    );
    assert_eq!(json.status.code(), Some(0));
    let envelope: serde_json::Value = serde_json::from_str(&stdout(&json)).expect("json");
    assert_eq!(envelope["tool"], "pi");
    assert_eq!(envelope["exit"], 0);
    assert!(envelope["model"].is_null() || envelope["model"].is_string());

    let failed = run_qc(&qc, &scratch, &["plain"], &[("QC_AGENT_TEXT", "")]);
    assert_eq!(failed.status.code(), Some(1));
    assert_eq!(stdout(&failed), "");
    assert!(stderr(&failed).contains("[ERROR] pi produced empty assistant text"));
    let session_id = session_id_from(&stderr(&failed));
    assert!(
        scratch
            .home()
            .join(".qc/sessions")
            .join(format!("{session_id}.json"))
            .is_file()
    );

    let continued = run_qc(
        &qc,
        &scratch,
        &["-c", &session_id, "--append", "retry"],
        &[("QC_AGENT_TEXT", "resumed")],
    );
    assert_eq!(continued.status.code(), Some(0));
    assert!(stdout(&continued).contains("resumed"));
    assert!(stdout(&continued).contains(&format!("[QC-SESSION] {session_id}")));

    let opened = run_qc(
        &qc,
        &scratch,
        &["-o", &session_id],
        &[("QC_RECORD", record.to_str().unwrap())],
    );
    assert_eq!(opened.status.code(), Some(0), "stderr={}", stderr(&opened));
    let tui: serde_json::Value =
        serde_json::from_slice(&fs::read(&record).expect("record")).expect("json");
    assert_eq!(tui["argv"][0], "--session-id");
}

#[test]
fn missing_binary_does_not_write_a_mapping() {
    let scratch = unique_scratch("smoke-missing");
    fs::create_dir_all(scratch.cwd().join(".qc/prompts")).expect("prompts");
    write_file(&scratch.cwd().join(".qc/prompts/plain.md"), "Plain");
    let qc = stage_package(&scratch, "0.0.0");
    let output = Command::new(&qc)
        .args(["plain"])
        .current_dir(scratch.cwd())
        .env_clear()
        .env("HOME", scratch.home())
        .env("PATH", "/usr/bin:/bin")
        .env("SHELL", "/bin/sh")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("qc");
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("pi executable 'pi' was not found"));
    assert!(!scratch.home().join(".qc/sessions").exists());
}

#[test]
fn parse_error_creates_no_home_qc() {
    let scratch = unique_scratch("smoke-parse");
    let qc = stage_package(&scratch, "0.0.0");
    let output = run_qc(&qc, &scratch, &["--wat"], &[]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stderr(&output), "[ERROR] unknown option '--wat'\n");
    assert!(!scratch.home().join(".qc").exists());
}

#[test]
fn debug_redacts_prompt_and_uses_utf16_unit_labels() {
    let scratch = unique_scratch("smoke-debug");
    install_fixture_names(&scratch.bin());
    fs::create_dir_all(scratch.cwd().join(".qc/prompts")).expect("prompts");
    write_file(&scratch.cwd().join(".qc/prompts/plain.md"), "SECRET-PROMPT");
    let qc = stage_package(&scratch, "0.0.0");
    let output = run_qc(
        &qc,
        &scratch,
        &["plain", "--debug"],
        &[("QC_AGENT_TEXT", "joke")],
    );
    assert_eq!(output.status.code(), Some(0));
    let out = stdout(&output);
    assert!(out.contains("[DEBUG]"));
    assert!(!out.contains("SECRET-PROMPT"));
    assert!(out.contains("<prompt>") || out.contains("argv="));
}
