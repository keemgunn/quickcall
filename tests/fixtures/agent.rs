//! Fixture provider binary. Tests copy or symlink this to `pi`, `agent`, `claude`,
//! `opencode`, `agy`, and `codex`. Never a real agent. Never staged into `dist/`.

use std::env;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

fn env_var(name: &str) -> Option<String> {
    // Node `process.env.X ?? default` treats empty string as set.
    env::var(name).ok()
}

fn env_flag(name: &str) -> bool {
    env::var(name).ok().as_deref() == Some("1")
}

fn argv() -> Vec<String> {
    env::args().skip(1).collect()
}

fn binary_name() -> String {
    env::args()
        .next()
        .and_then(|path| {
            Path::new(&path)
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| "qc-test-agent".to_string())
}

fn write_record(argv: &[String], stdin: &str) {
    if let Some(path) = env_var("QC_RECORD") {
        let record = serde_json::json!({
            "argv": argv,
            "stdin": stdin,
            "cwd": env::current_dir().expect("cwd").to_string_lossy(),
        });
        fs::write(path, serde_json::to_string(&record).expect("record json")).expect("QC_RECORD");
    }
}

fn read_stdin() -> String {
    let mut bytes = Vec::new();
    io::stdin().read_to_end(&mut bytes).expect("stdin");
    String::from_utf8_lossy(&bytes).into_owned()
}

fn after_flag<'a>(argv: &'a [String], flag: &str) -> Option<&'a str> {
    argv.iter()
        .position(|token| token == flag)
        .and_then(|index| argv.get(index + 1))
        .map(String::as_str)
}

fn parse_exit(name: &str) -> i32 {
    env_var(name)
        .and_then(|value| value.parse().ok())
        .unwrap_or(0)
}

fn send_self_signal(name: &str) {
    let signal = match name {
        "SIGINT" | "2" => libc::SIGINT,
        "SIGTERM" | "15" => libc::SIGTERM,
        "SIGKILL" | "9" => libc::SIGKILL,
        other => panic!("unsupported fixture signal {other}"),
    };
    // Test-only self-signal; production qc does not use this path.
    unsafe {
        libc::kill(libc::getpid(), signal);
    }
}

fn ignore_term_and_int() {
    unsafe {
        libc::signal(libc::SIGTERM, libc::SIG_IGN);
        libc::signal(libc::SIGINT, libc::SIG_IGN);
    }
}

fn run_sleeper() -> ! {
    if env_flag("QC_PI_DESCENDANT_IGNORE_TERM") {
        ignore_term_and_int();
    }
    loop {
        thread::sleep(Duration::from_secs(1));
    }
}

fn spawn_stdout_holder() {
    let exe = env::current_exe().expect("current_exe");
    let mut child = Command::new(exe);
    child.env("QC_TEST_AGENT_INTERNAL", "sleeper");
    if env_flag("QC_PI_DESCENDANT_IGNORE_TERM") {
        child.env("QC_PI_DESCENDANT_IGNORE_TERM", "1");
    }
    child.stdin(Stdio::null());
    child.stdout(Stdio::inherit());
    child.stderr(Stdio::null());
    let spawned = child.spawn().expect("stdout holder");
    if let Some(path) = env_var("QC_PI_SLEEPER_PID") {
        fs::write(path, spawned.id().to_string()).expect("QC_PI_SLEEPER_PID");
    }
    // Keep the descendant running after this process exits; Child::drop would not
    // kill it, but clippy requires we not leave an unreaped handle.
    std::mem::forget(spawned);
}

static WAIT_SIGNAL_RECORD: OnceLock<PathBuf> = OnceLock::new();
static WAIT_IGNORE: AtomicBool = AtomicBool::new(false);

fn wait_for_signal() -> ! {
    let ready =
        env_var("QC_PI_READY").expect("QC_PI_WAIT requires QC_PI_READY and QC_PI_SIGNAL_RECORD");
    let signal_record = env_var("QC_PI_SIGNAL_RECORD")
        .expect("QC_PI_WAIT requires QC_PI_READY and QC_PI_SIGNAL_RECORD");
    WAIT_IGNORE.store(env_flag("QC_PI_IGNORE_SIGNALS"), Ordering::SeqCst);
    let _ = WAIT_SIGNAL_RECORD.set(PathBuf::from(signal_record));
    unsafe {
        libc::signal(
            libc::SIGINT,
            wait_handler as *const () as libc::sighandler_t,
        );
        libc::signal(
            libc::SIGTERM,
            wait_handler as *const () as libc::sighandler_t,
        );
    }
    fs::write(ready, "ready").expect("QC_PI_READY");
    loop {
        thread::sleep(Duration::from_secs(1));
    }
}

extern "C" fn wait_handler(signal: libc::c_int) {
    let name = match signal {
        libc::SIGINT => "SIGINT",
        libc::SIGTERM => "SIGTERM",
        _ => "UNKNOWN",
    };
    if let Some(path) = WAIT_SIGNAL_RECORD.get() {
        let _ = fs::write(path, name);
    }
    if WAIT_IGNORE.load(Ordering::SeqCst) {
        return;
    }
    unsafe {
        libc::signal(signal, libc::SIG_DFL);
        libc::kill(libc::getpid(), signal);
    }
}

fn finish_pi(argv: &[String], stdin: &str) {
    write_record(argv, stdin);
    if let Some(path) = env_var("QC_PI_PID") {
        fs::write(path, std::process::id().to_string()).expect("QC_PI_PID");
    }

    let session_id = after_flag(argv, "--session-id")
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| "fixture-pi-session".to_string());
    let text = env_var("QC_PI_TEXT")
        .or_else(|| env_var("QC_AGENT_TEXT"))
        .unwrap_or_else(|| "pi-assistant".to_string());
    let tui = {
        let mode = argv.iter().position(|token| token == "--mode");
        let json_mode = mode
            .and_then(|index| argv.get(index + 1))
            .is_some_and(|value| value == "json");
        let print_mode = argv.iter().any(|token| token == "-p" || token == "--print");
        !json_mode && !print_mode
    };

    if let Some(path) = env_var("QC_PI_MARKER") {
        use std::fs::OpenOptions;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .expect("QC_PI_MARKER");
        file.write_all(b"pi\n").expect("QC_PI_MARKER write");
    }
    if let Some(stderr) = env_var("QC_PI_STDERR") {
        eprint!("{stderr}");
    }

    if env_flag("QC_PI_WAIT") {
        wait_for_signal();
    }

    if let Some(signal) = env_var("QC_PI_SIGNAL") {
        send_self_signal(&signal);
    }

    if env_flag("QC_PI_HOLD_STDOUT") {
        spawn_stdout_holder();
    }

    if !tui {
        let session = serde_json::json!({
            "type": "session",
            "version": 3,
            "id": session_id,
            "timestamp": "2026-09-01T00:00:00Z",
            "cwd": env::current_dir().expect("cwd").to_string_lossy(),
        });
        println!("{}", serde_json::to_string(&session).expect("session"));
        let mut message = serde_json::json!({
            "role": "assistant",
            "content": [{ "type": "text", "text": text }],
        });
        if env_flag("QC_PI_USAGE") {
            let cost_total = env_var("QC_PI_USAGE_COST")
                .and_then(|value| value.parse::<f64>().ok())
                .unwrap_or(0.012);
            message["usage"] = serde_json::json!({
                "input": 1200,
                "output": 400,
                "cacheRead": 50,
                "reasoning": 10,
                "totalTokens": 1660,
                "cost": { "total": cost_total },
            });
        }
        let message_end = serde_json::json!({
            "type": "message_end",
            "message": message,
        });
        println!(
            "{}",
            serde_json::to_string(&message_end).expect("message_end")
        );
    }

    if env_flag("QC_PI_CLOSE_STDIO") {
        let hold_ms = env_var("QC_PI_CLOSE_STDIO_MS")
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(400);
        drop(io::stdout());
        drop(io::stderr());
        thread::sleep(Duration::from_millis(hold_ms));
        std::process::exit(parse_exit("QC_PI_EXIT"));
    }

    if let Some(signal) = env_var("QC_PI_SIGNAL_AFTER_TEXT") {
        send_self_signal(&signal);
        return;
    }

    std::process::exit(parse_exit("QC_PI_EXIT"));
}

fn run_pi() {
    let argv = argv();
    let mode = argv.iter().position(|token| token == "--mode");
    let json_mode = mode
        .and_then(|index| argv.get(index + 1))
        .is_some_and(|value| value == "json");
    let print_mode = argv.iter().any(|token| token == "-p" || token == "--print");
    let tui = !json_mode && !print_mode;
    if tui {
        finish_pi(&argv, "");
    } else {
        let stdin = read_stdin();
        finish_pi(&argv, &stdin);
    }
}

fn run_cursor() {
    let argv = argv();
    write_record(&argv, "");
    if let Some(stderr) = env_var("QC_AGENT_STDERR") {
        eprint!("{stderr}");
    }
    if env_var("QC_AGENT_EMPTY_JSON").is_some() {
        std::process::exit(parse_exit("QC_AGENT_EXIT").max(1));
    }
    let session_id = after_flag(&argv, "--resume")
        .map(ToOwned::to_owned)
        .or_else(|| env_var("QC_CURSOR_SESSION"))
        .unwrap_or_else(|| "11111111-2222-3333-4444-555555555555".to_string());
    let text = env_var("QC_AGENT_TEXT").unwrap_or_else(|| "cursor-assistant".to_string());
    println!(
        "{}",
        serde_json::to_string(&serde_json::json!({
            "result": text,
            "session_id": session_id,
        }))
        .expect("cursor json")
    );
    std::process::exit(parse_exit("QC_AGENT_EXIT"));
}

fn run_claude() {
    let argv = argv();
    write_record(&argv, "");
    if let Some(stderr) = env_var("QC_CLAUDE_STDERR") {
        eprint!("{stderr}");
    }
    let session_id = after_flag(&argv, "--resume")
        .map(ToOwned::to_owned)
        .or_else(|| after_flag(&argv, "--session-id").map(ToOwned::to_owned))
        .or_else(|| env_var("QC_CLAUDE_SESSION"))
        .unwrap_or_else(|| "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee".to_string());

    if let Some(detail) = env_var("QC_CLAUDE_ERROR") {
        if env_var("QC_CLAUDE_STDERR").is_none() {
            eprintln!("[claude-code:unrecognized_model]");
        }
        let is_error = env::var("QC_CLAUDE_IS_ERROR").ok().as_deref() != Some("0");
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({
                "type": "result",
                "subtype": "success",
                "is_error": is_error,
                "result": detail,
                "session_id": session_id,
            }))
            .expect("claude error json")
        );
        std::process::exit(if env_var("QC_CLAUDE_EXIT").is_some() {
            parse_exit("QC_CLAUDE_EXIT")
        } else {
            1
        });
    }

    let text = env_var("QC_AGENT_TEXT").unwrap_or_else(|| "claude-assistant".to_string());
    let mut payload = serde_json::json!({
        "result": text,
        "session_id": session_id,
    });
    if env_flag("QC_CLAUDE_USAGE") {
        payload["usage"] = serde_json::json!({
            "input_tokens": 100,
            "output_tokens": 20,
        });
        payload["total_cost_usd"] = serde_json::json!(0.012);
    }
    println!("{}", serde_json::to_string(&payload).expect("claude json"));
    std::process::exit(parse_exit("QC_CLAUDE_EXIT"));
}

fn run_opencode() {
    let argv = argv();
    write_record(&argv, "");
    if let Some(stderr) = env_var("QC_OPENCODE_STDERR") {
        eprint!("{stderr}");
    }
    let session_id = after_flag(&argv, "-s")
        .map(ToOwned::to_owned)
        .or_else(|| env_var("QC_OPENCODE_SESSION"))
        .unwrap_or_else(|| "ses_fixture_opencode".to_string());

    if let Some(detail) = env_var("QC_OPENCODE_ERROR") {
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({
                "type": "session",
                "sessionID": session_id,
            }))
            .expect("opencode session")
        );
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({
                "type": "error",
                "sessionID": session_id,
                "error": { "name": "UnknownError", "data": { "message": detail } },
            }))
            .expect("opencode error")
        );
        std::process::exit(if env_var("QC_OPENCODE_EXIT").is_some() {
            parse_exit("QC_OPENCODE_EXIT")
        } else {
            1
        });
    }

    let text = env_var("QC_AGENT_TEXT").unwrap_or_else(|| "opencode-assistant".to_string());
    println!(
        "{}",
        serde_json::to_string(&serde_json::json!({
            "type": "session",
            "sessionID": session_id,
        }))
        .expect("opencode session")
    );
    if let Some(title) = after_flag(&argv, "--title") {
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({
                "type": "title",
                "title": title,
            }))
            .expect("opencode title")
        );
    }
    println!(
        "{}",
        serde_json::to_string(&serde_json::json!({
            "type": "text",
            "part": { "type": "text", "text": text },
        }))
        .expect("opencode text")
    );
    std::process::exit(parse_exit("QC_OPENCODE_EXIT"));
}

fn run_agy() {
    let argv = argv();
    write_record(&argv, "");
    if let Some(stderr) = env_var("QC_AGY_STDERR") {
        eprint!("{stderr}");
    }
    if let Some(detail) = env_var("QC_AGY_ERROR") {
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({
                "status": "ERROR",
                "conversation_id": "",
                "error": detail,
                "response": "",
            }))
            .expect("agy error")
        );
        std::process::exit(if env_var("QC_AGY_EXIT").is_some() {
            parse_exit("QC_AGY_EXIT")
        } else {
            1
        });
    }
    let conversation_id = after_flag(&argv, "--conversation")
        .map(ToOwned::to_owned)
        .or_else(|| env_var("QC_AGY_SESSION"))
        .unwrap_or_else(|| "conv_fixture_agy".to_string());
    let text = env_var("QC_AGENT_TEXT").unwrap_or_else(|| "agy-assistant".to_string());
    let mut payload = serde_json::json!({
        "response": text,
        "conversation_id": conversation_id,
    });
    if env_flag("QC_AGY_USAGE") {
        payload["usage"] = serde_json::json!({
            "input_tokens": 1200,
            "output_tokens": 400,
            "thinking_tokens": 20,
            "cache_read_tokens": 50,
            "total_tokens": 1660,
        });
    }
    println!("{}", serde_json::to_string(&payload).expect("agy json"));
    std::process::exit(parse_exit("QC_AGY_EXIT"));
}

fn main() {
    if env::var("QC_TEST_AGENT_INTERNAL").ok().as_deref() == Some("sleeper") {
        run_sleeper();
    }
    match binary_name().as_str() {
        "agent" => run_cursor(),
        "claude" => run_claude(),
        "opencode" => run_opencode(),
        "agy" => run_agy(),
        "codex" => run_codex(),
        _ => run_pi(),
    }
}

fn run_codex() {
    let argv = argv();
    let headless = argv
        .iter()
        .any(|token| token == "exec" || token == "--json");
    if !headless {
        write_record(&argv, "");
        std::process::exit(parse_exit("QC_CODEX_EXIT"));
    }
    let stdin = read_stdin();
    write_record(&argv, &stdin);
    if let Some(stderr) = env_var("QC_CODEX_STDERR") {
        eprint!("{stderr}");
    }
    let session_id = after_flag(&argv, "resume")
        .map(ToOwned::to_owned)
        .or_else(|| env_var("QC_CODEX_SESSION"))
        .unwrap_or_else(|| "thread_fixture_codex".to_string());

    println!(
        "{}",
        serde_json::to_string(&serde_json::json!({
            "type": "thread.started",
            "thread_id": session_id,
        }))
        .expect("codex thread")
    );

    if let Some(detail) = env_var("QC_CODEX_ERROR") {
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({
                "type": "turn.failed",
                "error": { "message": detail },
            }))
            .expect("codex error")
        );
        std::process::exit(if env_var("QC_CODEX_EXIT").is_some() {
            parse_exit("QC_CODEX_EXIT")
        } else {
            1
        });
    }

    let text = env_var("QC_AGENT_TEXT").unwrap_or_else(|| "codex-assistant".to_string());
    println!(
        "{}",
        serde_json::to_string(&serde_json::json!({
            "type": "item.completed",
            "item": { "type": "agent_message", "text": text },
        }))
        .expect("codex text")
    );
    if env_flag("QC_CODEX_USAGE") {
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({
                "type": "turn.completed",
                "usage": {
                    "input_tokens": 1200,
                    "output_tokens": 400,
                    "cached_input_tokens": 50,
                    "reasoning_output_tokens": 10,
                    "cache_write_input_tokens": 9
                },
            }))
            .expect("codex usage")
        );
    }
    std::process::exit(parse_exit("QC_CODEX_EXIT"));
}
