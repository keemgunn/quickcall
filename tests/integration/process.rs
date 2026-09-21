use std::collections::HashMap;
use std::fs;
use std::os::fd::AsRawFd;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use chrono::Utc;
use nix::sys::signal::{Signal, kill};
use nix::sys::wait::{WaitPidFlag, WaitStatus, waitpid};
use nix::unistd::{ForkResult, Pid, fork};
use quickcall::parsers::utc_millis;
use quickcall::session::{SessionMapping, load_session, save_session};
use quickcall::tools::ToolName;
use quickcall::tools::spawn::{spawn_agent, spawn_interactive};
use quickcall::tools::tui::build_tui_args;

use crate::common::{
    GroupGuard, fixture_env, install_fixture_names, pid_alive, unique_scratch, wait_for_file,
};

fn waitpid_deadline(pid: Pid, timeout: Duration) -> WaitStatus {
    let deadline = Instant::now() + timeout;
    loop {
        match waitpid(pid, Some(WaitPidFlag::WNOHANG)).expect("waitpid") {
            WaitStatus::StillAlive => {
                if Instant::now() >= deadline {
                    let _ = kill(pid, Signal::SIGKILL);
                    let _ = waitpid(pid, None);
                    panic!("timed out waiting for pid {}", pid);
                }
                thread::sleep(Duration::from_millis(20));
            }
            status => return status,
        }
    }
}

fn sh_env(scratch: &crate::common::Scratch) -> HashMap<String, String> {
    let mut environment = HashMap::new();
    environment.insert("HOME".into(), scratch.home().to_string_lossy().into_owned());
    environment.insert("PATH".into(), "/usr/bin:/bin".into());
    environment.insert("SHELL".into(), "/bin/sh".into());
    environment
}

#[test]
fn captures_stdin_stdout_stderr_and_numeric_exit() {
    let scratch = unique_scratch("spawn-io");
    let capture = spawn_agent(
        "/bin/sh",
        &[
            "-c".into(),
            "cat; printf 'out-data'; printf 'err-data' >&2; exit 7".into(),
        ],
        Some("stdin-payload"),
        scratch.cwd().to_str().unwrap(),
        &sh_env(&scratch),
        "sh",
    )
    .expect("spawn");
    let _guard = GroupGuard {
        pgid: capture.pid as i32,
    };
    assert_eq!(capture.exit, 7);
    assert!(capture.stdout.contains("out-data"), "{}", capture.stdout);
    assert!(capture.stderr.contains("err-data"), "{}", capture.stderr);
}

#[test]
fn drains_both_pipes_larger_than_capacity_while_stdin_is_large() {
    let scratch = unique_scratch("spawn-large");
    let stdin = "x".repeat(256 * 1024);
    let started = Instant::now();
    let capture = spawn_agent(
        "/bin/sh",
        &["-c".into(), "dd of=/dev/null bs=4096 2>/dev/null; head -c 262144 /dev/zero; head -c 262144 /dev/zero 1>&2".into()],
        Some(&stdin),
        scratch.cwd().to_str().unwrap(),
        &sh_env(&scratch),
        "sh",
    )
    .expect("spawn");
    let _guard = GroupGuard {
        pgid: capture.pid as i32,
    };
    assert_eq!(capture.exit, 0);
    assert!(
        capture.stdout.len() >= 200_000,
        "stdout {}",
        capture.stdout.len()
    );
    assert!(
        capture.stderr.len() >= 200_000,
        "stderr {}",
        capture.stderr.len()
    );
    assert!(started.elapsed() < Duration::from_secs(8));
}

#[test]
fn waits_for_direct_child_after_pipes_close_early() {
    let scratch = unique_scratch("spawn-early-eof");
    let started = Instant::now();
    let capture = spawn_agent(
        "/bin/sh",
        &[
            "-c".into(),
            "printf 'after-eof\\n'; exec >&-; exec 2>&-; sleep 0.4; exit 0".into(),
        ],
        None,
        scratch.cwd().to_str().unwrap(),
        &sh_env(&scratch),
        "sh",
    )
    .expect("spawn");
    let elapsed = started.elapsed();
    let _guard = GroupGuard {
        pgid: capture.pid as i32,
    };
    assert_eq!(capture.exit, 0);
    assert!(capture.stdout.contains("after-eof"), "{}", capture.stdout);
    assert!(
        elapsed >= Duration::from_millis(300),
        "returned too early {elapsed:?}"
    );
}

#[test]
fn descendant_holding_stdout_is_bounded_and_cleaned_up() {
    let scratch = unique_scratch("spawn-hold");
    install_fixture_names(&scratch.bin());
    let sleeper = scratch.root.join("sleeper.pid");
    let mut env = fixture_env(&scratch);
    env.insert("QC_AGENT_TEXT".into(), "drained".into());
    env.insert("QC_PI_HOLD_STDOUT".into(), "1".into());
    env.insert(
        "QC_PI_SLEEPER_PID".into(),
        sleeper.to_string_lossy().into_owned(),
    );
    let started = Instant::now();
    let capture = spawn_agent(
        "pi",
        &[
            "--mode".into(),
            "json".into(),
            "-a".into(),
            "--session-id".into(),
            "sid".into(),
        ],
        Some("prompt"),
        scratch.cwd().to_str().unwrap(),
        &env,
        "pi",
    )
    .expect("spawn");
    let elapsed = started.elapsed();
    let _guard = GroupGuard {
        pgid: capture.pid as i32,
    };
    assert_eq!(capture.exit, 0);
    assert!(capture.stdout.contains("drained") || capture.stdout.contains("pi-assistant"));
    assert!(elapsed >= Duration::from_millis(1500), "{elapsed:?}");
    assert!(elapsed < Duration::from_secs(8), "{elapsed:?}");
    wait_for_file(&sleeper, 2_000);
    let sleeper_pid: i32 = fs::read_to_string(&sleeper)
        .expect("sleeper pid")
        .trim()
        .parse()
        .expect("pid");
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline && pid_alive(sleeper_pid) {
        thread::sleep(Duration::from_millis(50));
    }
    assert!(!pid_alive(sleeper_pid), "TERM-responsive descendant leaked");
}

#[test]
fn term_ignoring_descendant_is_killed_after_cleanup_grace() {
    let scratch = unique_scratch("spawn-ignore-term");
    install_fixture_names(&scratch.bin());
    let sleeper = scratch.root.join("sleeper.pid");
    let mut env = fixture_env(&scratch);
    env.insert("QC_AGENT_TEXT".into(), "drained".into());
    env.insert("QC_PI_HOLD_STDOUT".into(), "1".into());
    env.insert("QC_PI_DESCENDANT_IGNORE_TERM".into(), "1".into());
    env.insert(
        "QC_PI_SLEEPER_PID".into(),
        sleeper.to_string_lossy().into_owned(),
    );
    let capture = spawn_agent(
        "pi",
        &[
            "--mode".into(),
            "json".into(),
            "-a".into(),
            "--session-id".into(),
            "sid".into(),
        ],
        Some("prompt"),
        scratch.cwd().to_str().unwrap(),
        &env,
        "pi",
    )
    .expect("spawn");
    let _guard = GroupGuard {
        pgid: capture.pid as i32,
    };
    wait_for_file(&sleeper, 2_000);
    let sleeper_pid: i32 = fs::read_to_string(&sleeper)
        .expect("sleeper pid")
        .trim()
        .parse()
        .expect("pid");
    let deadline = Instant::now() + Duration::from_secs(4);
    while Instant::now() < deadline && pid_alive(sleeper_pid) {
        thread::sleep(Duration::from_millis(50));
    }
    assert!(
        !pid_alive(sleeper_pid),
        "TERM-ignoring descendant should be KILLed"
    );
}

fn forwards_signal_to_the_child_group(signal: Signal, label: &str, expected_exit: i32) {
    let scratch = unique_scratch(&format!("spawn-{}", label.to_lowercase()));
    install_fixture_names(&scratch.bin());
    let ready = scratch.root.join("ready");
    let record = scratch.root.join("signal");
    let mut env = fixture_env(&scratch);
    env.insert("QC_PI_WAIT".into(), "1".into());
    env.insert("QC_PI_READY".into(), ready.to_string_lossy().into_owned());
    env.insert(
        "QC_PI_SIGNAL_RECORD".into(),
        record.to_string_lossy().into_owned(),
    );
    let sent = Arc::new(AtomicBool::new(false));
    let sent2 = sent.clone();
    let ready2 = ready.clone();
    thread::spawn(move || {
        wait_for_file(&ready2, 8_000);
        thread::sleep(Duration::from_millis(50));
        sent2.store(true, Ordering::SeqCst);
        let _ = kill(Pid::from_raw(std::process::id() as i32), signal);
    });
    let capture = spawn_agent(
        "pi",
        &[
            "--mode".into(),
            "json".into(),
            "-a".into(),
            "--session-id".into(),
            "sid".into(),
        ],
        Some("prompt"),
        scratch.cwd().to_str().unwrap(),
        &env,
        "pi",
    )
    .expect("spawn");
    let _guard = GroupGuard {
        pgid: capture.pid as i32,
    };
    assert!(sent.load(Ordering::SeqCst));
    wait_for_file(&record, 2_000);
    assert_eq!(fs::read_to_string(&record).expect("record").trim(), label);
    assert_eq!(capture.exit, expected_exit);
}

#[test]
fn forwards_sigint_to_the_child_group() {
    forwards_signal_to_the_child_group(Signal::SIGINT, "SIGINT", 130);
}

#[test]
fn forwards_sigterm_to_the_child_group() {
    forwards_signal_to_the_child_group(Signal::SIGTERM, "SIGTERM", 143);
}

#[test]
fn missing_executable_has_no_spawn_evidence() {
    let scratch = unique_scratch("spawn-missing");
    let err = spawn_agent(
        "definitely-missing-qc-agent-xyzzy",
        &[],
        Some("prompt"),
        scratch.cwd().to_str().unwrap(),
        &sh_env(&scratch),
        "pi",
    )
    .expect_err("missing");
    assert!(
        err.message
            .contains("pi executable 'definitely-missing-qc-agent-xyzzy' was not found"),
        "{}",
        err.message
    );
}

#[test]
fn signal_exit_from_self_signal_is_128_plus_signo() {
    let scratch = unique_scratch("spawn-sigexit");
    install_fixture_names(&scratch.bin());
    let mut env = fixture_env(&scratch);
    env.insert("QC_AGENT_TEXT".into(), "kept".into());
    env.insert("QC_PI_SIGNAL_AFTER_TEXT".into(), "SIGINT".into());
    let capture = spawn_agent(
        "pi",
        &[
            "--mode".into(),
            "json".into(),
            "-a".into(),
            "--session-id".into(),
            "sid".into(),
        ],
        Some("prompt"),
        scratch.cwd().to_str().unwrap(),
        &env,
        "pi",
    )
    .expect("spawn");
    let _guard = GroupGuard {
        pgid: capture.pid as i32,
    };
    assert_eq!(capture.exit, 130);
    assert!(capture.stdout.contains("kept") || capture.stdout.contains("message_end"));
}

#[test]
fn forwards_signal_during_drain_without_overwriting_frozen_exit() {
    let scratch = unique_scratch("spawn-drain-sig");
    install_fixture_names(&scratch.bin());
    let sleeper = scratch.root.join("sleeper.pid");
    let mut env = fixture_env(&scratch);
    env.insert("QC_AGENT_TEXT".into(), "drained".into());
    env.insert("QC_PI_HOLD_STDOUT".into(), "1".into());
    env.insert(
        "QC_PI_SLEEPER_PID".into(),
        sleeper.to_string_lossy().into_owned(),
    );
    let sleeper2 = sleeper.clone();
    thread::spawn(move || {
        wait_for_file(&sleeper2, 8_000);
        thread::sleep(Duration::from_millis(200));
        let _ = kill(Pid::from_raw(std::process::id() as i32), Signal::SIGINT);
    });
    let capture = spawn_agent(
        "pi",
        &[
            "--mode".into(),
            "json".into(),
            "-a".into(),
            "--session-id".into(),
            "sid".into(),
        ],
        Some("prompt"),
        scratch.cwd().to_str().unwrap(),
        &env,
        "pi",
    )
    .expect("spawn");
    let _guard = GroupGuard {
        pgid: capture.pid as i32,
    };
    assert_eq!(
        capture.exit, 0,
        "drain-period SIGINT must not overwrite frozen status"
    );
}

#[test]
fn interactive_tui_updates_mapping_on_successful_spawn() {
    let scratch = unique_scratch("tui-map");
    install_fixture_names(&scratch.bin());
    let home = scratch.home();
    let id = "260901-1200--pi--a1b2c3";
    let created = "2026-09-01T00:00:00.000Z";
    let updated = "2026-09-01T00:01:00.000Z";
    save_session(
        &home,
        id,
        &SessionMapping {
            tool: ToolName::Pi,
            native_id: "native-pi-1".into(),
            cwd: scratch.cwd().to_string_lossy().into_owned(),
            created: created.into(),
            updated: updated.into(),
            warnings: vec!["kept".into()],
        },
    )
    .expect("save");
    let env = fixture_env(&scratch);
    let args = build_tui_args(ToolName::Pi, "native-pi-1").expect("argv");
    assert!(
        !args
            .iter()
            .any(|t| t == "--mode" || t == "json" || t == "-p")
    );
    let home2 = home.clone();
    let exit = spawn_interactive(
        "pi",
        &args,
        scratch.cwd().to_str().unwrap(),
        &env,
        "pi",
        Some(&|| {
            let mut mapping = load_session(&home2, id).expect("load");
            mapping.updated = utc_millis(Utc::now());
            save_session(&home2, id, &mapping)
        }),
    )
    .expect("tui");
    assert_eq!(exit, 0);
    let mapping = load_session(&home, id).expect("reload");
    assert_eq!(mapping.native_id, "native-pi-1");
    assert_eq!(mapping.warnings, vec!["kept".to_string()]);
    assert!(mapping.updated.as_str() > updated, "{}", mapping.updated);
}

#[test]
fn interactive_missing_binary_has_no_spawn_evidence() {
    let scratch = unique_scratch("tui-missing");
    let err = spawn_interactive(
        "definitely-missing-qc-tui-xyzzy",
        &["--session-id".into(), "n".into()],
        scratch.cwd().to_str().unwrap(),
        &sh_env(&scratch),
        "pi",
        Some(&|| panic!("on_spawn must not run")),
    )
    .expect_err("missing");
    assert!(
        err.message
            .contains("pi executable 'definitely-missing-qc-tui-xyzzy' was not found"),
        "{}",
        err.message
    );
}

#[test]
fn interactive_inherits_a_real_pty() {
    let scratch = unique_scratch("tui-pty");
    let pty = nix::pty::openpty(None, None).expect("openpty");
    match unsafe { fork() } {
        Ok(ForkResult::Child) => {
            let slave = pty.slave.as_raw_fd();
            unsafe {
                libc::dup2(slave, 0);
                libc::dup2(slave, 1);
                libc::dup2(slave, 2);
            }
            drop(pty.master);
            drop(pty.slave);
            let code = spawn_interactive(
                "/bin/sh",
                &[
                    "-c".into(),
                    "if [ -t 0 ] && [ -t 1 ]; then echo tty-ok; else exit 2; fi".into(),
                ],
                scratch.cwd().to_str().unwrap(),
                &sh_env(&scratch),
                "sh",
                None,
            )
            .unwrap_or(1);
            // SAFETY: child of fork must not unwind.
            unsafe { libc::_exit(code) };
        }
        Ok(ForkResult::Parent { child }) => {
            drop(pty.slave);
            let master = pty.master.as_raw_fd();
            // SAFETY: master is the live PTY fd we opened in this test.
            unsafe {
                let flags = libc::fcntl(master, libc::F_GETFL);
                if flags >= 0 {
                    libc::fcntl(master, libc::F_SETFL, flags | libc::O_NONBLOCK);
                }
            }
            let mut drain = [0u8; 256];
            let mut saw_tty = false;
            let status = {
                let deadline = Instant::now() + Duration::from_secs(5);
                loop {
                    // SAFETY: nonblocking read of the PTY master we still own.
                    let n = unsafe { libc::read(master, drain.as_mut_ptr().cast(), drain.len()) };
                    if n > 0 {
                        let chunk = String::from_utf8_lossy(&drain[..n as usize]);
                        if chunk.contains("tty-ok") {
                            saw_tty = true;
                        }
                    }
                    match waitpid(child, Some(WaitPidFlag::WNOHANG)).expect("wait pty child") {
                        WaitStatus::StillAlive => {
                            if Instant::now() >= deadline {
                                let _ = kill(child, Signal::SIGKILL);
                                let _ = waitpid(child, None);
                                panic!("PTY child timed out");
                            }
                            thread::sleep(Duration::from_millis(20));
                        }
                        other => break other,
                    }
                }
            };
            match status {
                WaitStatus::Exited(_, 0) => {}
                other => panic!("pty child status {other:?}"),
            }
            assert!(saw_tty, "PTY child did not report a tty");
        }
        Err(err) => panic!("fork {err}"),
    }
}

fn repeated_signal_default_terminates_the_supervisor(signal: Signal, label: &str) {
    let scratch = unique_scratch(&format!("spawn-repeat-{}", label.to_lowercase()));
    install_fixture_names(&scratch.bin());
    let ready = scratch.root.join("ready");
    let record = scratch.root.join("signal");
    let mut env = fixture_env(&scratch);
    env.insert("QC_PI_WAIT".into(), "1".into());
    env.insert("QC_PI_IGNORE_SIGNALS".into(), "1".into());
    env.insert(
        "QC_PI_PID".into(),
        scratch.root.join("pi.pid").to_string_lossy().into_owned(),
    );
    env.insert("QC_PI_READY".into(), ready.to_string_lossy().into_owned());
    env.insert(
        "QC_PI_SIGNAL_RECORD".into(),
        record.to_string_lossy().into_owned(),
    );
    match unsafe { fork() } {
        Ok(ForkResult::Child) => {
            let _ = spawn_agent(
                "pi",
                &[
                    "--mode".into(),
                    "json".into(),
                    "-a".into(),
                    "--session-id".into(),
                    "sid".into(),
                ],
                Some("prompt"),
                scratch.cwd().to_str().unwrap(),
                &env,
                "pi",
            );
            unsafe { libc::_exit(0) };
        }
        Ok(ForkResult::Parent { child }) => {
            wait_for_file(&ready, 8_000);
            thread::sleep(Duration::from_millis(50));
            let _ = kill(child, signal);
            wait_for_file(&record, 8_000);
            assert_eq!(fs::read_to_string(&record).expect("record").trim(), label);
            let _ = kill(child, signal);
            let status = waitpid_deadline(child, Duration::from_secs(5));
            let fixture_pid_path = scratch.root.join("pi.pid");
            if let Ok(text) = fs::read_to_string(&fixture_pid_path)
                && let Ok(pid) = text.trim().parse::<i32>()
            {
                let _ = nix::sys::signal::killpg(Pid::from_raw(pid), Signal::SIGKILL);
                let _ = kill(Pid::from_raw(pid), Signal::SIGKILL);
            }
            let _ = kill(child, Signal::SIGKILL);
            match status {
                WaitStatus::Exited(_, code) => {
                    assert_ne!(code, 0, "second {label} should not look like success");
                }
                WaitStatus::Signaled(_, sig, _) => {
                    assert_eq!(sig, signal);
                }
                other => panic!("unexpected status {other:?}"),
            }
        }
        Err(err) => panic!("fork {err}"),
    }
}

#[test]
fn repeated_sigint_default_terminates_the_supervisor() {
    repeated_signal_default_terminates_the_supervisor(Signal::SIGINT, "SIGINT");
}

#[test]
fn repeated_sigterm_default_terminates_the_supervisor() {
    repeated_signal_default_terminates_the_supervisor(Signal::SIGTERM, "SIGTERM");
}
