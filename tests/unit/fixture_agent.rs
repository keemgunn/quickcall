use std::fs;
use std::io::Write;
use std::os::unix::process::ExitStatusExt;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use crate::common::{install_fixture_names, unique_scratch};

fn agent() -> Command {
    Command::new(env!("CARGO_BIN_EXE_qc-test-agent"))
}

#[test]
fn records_pi_argv_stdin_and_cwd_without_invoking_qc() {
    let scratch = unique_scratch("fx-pi");
    install_fixture_names(&scratch.bin());
    let record = scratch.root.join("record.json");
    let cwd = scratch.cwd();
    let mut child = Command::new(scratch.bin().join("pi"));
    child
        .current_dir(&cwd)
        .env_clear()
        .env("HOME", scratch.home())
        .env("QC_RECORD", &record)
        .env("QC_PI_TEXT", "hello-from-fixture")
        .env("PATH", "/usr/bin:/bin")
        .args(["--mode", "json", "-a", "--session-id", "sid-1"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut spawned = child.spawn().expect("spawn pi");
    spawned
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(b"prompt-body")
        .expect("write stdin");
    drop(spawned.stdin.take());
    let output = spawned.wait_with_output().expect("wait pi");
    assert_eq!(output.status.code(), Some(0));
    let recorded: serde_json::Value =
        serde_json::from_slice(&fs::read(&record).expect("record")).expect("json");
    assert_eq!(
        recorded["argv"],
        serde_json::json!(["--mode", "json", "-a", "--session-id", "sid-1"])
    );
    assert_eq!(recorded["stdin"], "prompt-body");
    assert_eq!(recorded["cwd"], cwd.to_string_lossy().as_ref());
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    assert!(stdout.contains("hello-from-fixture"));
    assert!(stdout.contains("message_end"));
}

#[test]
fn preserves_text_error_usage_and_exit_controls() {
    let scratch = unique_scratch("fx-controls");
    install_fixture_names(&scratch.bin());

    let pi = Command::new(scratch.bin().join("pi"))
        .env_clear()
        .env("HOME", scratch.home())
        .env("QC_PI_STDERR", "pi-stderr")
        .env("QC_PI_USAGE", "1")
        .env("QC_PI_EXIT", "3")
        .args(["--mode", "json"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("pi");
    // Close stdin so headless Pi reaches EOF.
    let mut pi = pi;
    drop(pi.stdin.take());
    let out = pi.wait_with_output().expect("pi wait");
    assert_eq!(out.status.code(), Some(3));
    assert!(String::from_utf8_lossy(&out.stdout).contains("totalTokens"));
    assert_eq!(String::from_utf8_lossy(&out.stderr), "pi-stderr");

    let agent = Command::new(scratch.bin().join("agent"))
        .env_clear()
        .env("HOME", scratch.home())
        .env("QC_AGENT_EMPTY_JSON", "1")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("agent");
    assert_eq!(agent.code(), Some(1));

    let claude = Command::new(scratch.bin().join("claude"))
        .env_clear()
        .env("HOME", scratch.home())
        .env("QC_CLAUDE_ERROR", "bad-model")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("claude");
    assert_eq!(claude.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&claude.stdout).contains("is_error"));

    let opencode = Command::new(scratch.bin().join("opencode"))
        .env_clear()
        .env("HOME", scratch.home())
        .env("QC_OPENCODE_ERROR", "boom")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .expect("opencode");
    assert_eq!(opencode.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&opencode.stdout).contains("UnknownError"));

    let agy = Command::new(scratch.bin().join("agy"))
        .env_clear()
        .env("HOME", scratch.home())
        .env("QC_AGY_ERROR", "invalid")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .expect("agy");
    assert_eq!(agy.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&agy.stdout).contains("\"status\":\"ERROR\""));

    let mut codex = Command::new(scratch.bin().join("codex"));
    codex
        .env_clear()
        .env("HOME", scratch.home())
        .env("QC_CODEX_ERROR", "codex-boom")
        .env("QC_CODEX_STDERR", "codex-stderr")
        .args(["exec", "--json", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut spawned = codex.spawn().expect("codex");
    drop(spawned.stdin.take());
    let out = spawned.wait_with_output().expect("codex wait");
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stdout).contains("turn.failed"));
    assert_eq!(String::from_utf8_lossy(&out.stderr), "codex-stderr");
}

#[test]
fn signal_and_descendant_controls_do_not_invoke_qc() {
    let scratch = unique_scratch("fx-signal");
    let mut child = agent();
    child
        .env_clear()
        .env("HOME", scratch.home())
        .env("QC_PI_SIGNAL", "SIGTERM")
        .args(["--mode", "json"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut spawned = child.spawn().expect("signal spawn");
    drop(spawned.stdin.take());
    let status = spawned.wait().expect("signal wait");
    assert_eq!(status.signal(), Some(libc::SIGTERM));

    let sleeper_pid_path = scratch.root.join("sleeper.pid");
    let mut hold = agent();
    hold.env_clear()
        .env("HOME", scratch.home())
        .env("QC_PI_HOLD_STDOUT", "1")
        .env("QC_PI_SLEEPER_PID", &sleeper_pid_path)
        .args(["--mode", "json"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut held = hold.spawn().expect("hold spawn");
    drop(held.stdin.take());
    let status = held.wait().expect("parent wait");
    assert_eq!(status.code(), Some(0));
    let pid_text = fs::read_to_string(&sleeper_pid_path).expect("sleeper pid");
    let pid: i32 = pid_text.trim().parse().expect("pid");
    let still_alive = unsafe { libc::kill(pid, 0) == 0 };
    assert!(still_alive, "descendant should still be running");
    unsafe {
        libc::kill(pid, libc::SIGKILL);
    }
    thread::sleep(Duration::from_millis(50));
}

#[test]
fn records_codex_argv_and_stdin_without_invoking_qc() {
    let scratch = unique_scratch("fx-codex");
    install_fixture_names(&scratch.bin());
    let record = scratch.root.join("record.json");
    let cwd = scratch.cwd();
    let mut child = Command::new(scratch.bin().join("codex"));
    child
        .current_dir(&cwd)
        .env_clear()
        .env("HOME", scratch.home())
        .env("QC_RECORD", &record)
        .env("QC_AGENT_TEXT", "hello-from-codex")
        .env("QC_CODEX_USAGE", "1")
        .env("PATH", "/usr/bin:/bin")
        .args([
            "exec",
            "--json",
            "--cd",
            cwd.to_str().unwrap(),
            "--dangerously-bypass-approvals-and-sandbox",
            "--dangerously-bypass-hook-trust",
            "-",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut spawned = child.spawn().expect("spawn codex");
    spawned
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(b"prompt-body")
        .expect("write stdin");
    drop(spawned.stdin.take());
    let output = spawned.wait_with_output().expect("wait codex");
    assert_eq!(output.status.code(), Some(0));
    let recorded: serde_json::Value =
        serde_json::from_slice(&fs::read(&record).expect("record")).expect("json");
    assert_eq!(recorded["stdin"], "prompt-body");
    assert_eq!(recorded["cwd"], cwd.to_string_lossy().as_ref());
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    assert!(stdout.contains("hello-from-codex"));
    assert!(stdout.contains("thread.started"));
    assert!(stdout.contains("cached_input_tokens"));
}
