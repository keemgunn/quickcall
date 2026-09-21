use std::collections::HashMap;
use std::ffi::CString;
use std::io::Read;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;

use crate::config::Config;
use crate::errors::QcError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expression {
    pub start: usize,
    pub end: usize,
    pub command: String,
}

/// Exact `!` + backtick … backtick expressions. Unclosed left-hand text stays literal.
pub fn expressions(text: &str) -> Vec<Expression> {
    let bytes = text.as_bytes();
    let mut found = Vec::new();
    let mut index = 0;
    while index + 2 < bytes.len() {
        if bytes[index] != b'!' || bytes[index + 1] != b'`' {
            index += 1;
            continue;
        }
        let mut cursor = index + 2;
        let mut command = String::new();
        let mut closed = false;
        while cursor < bytes.len() {
            if bytes[cursor] == b'\\' && cursor + 1 < bytes.len() {
                command.push(bytes[cursor] as char);
                command.push(bytes[cursor + 1] as char);
                cursor += 2;
                continue;
            }
            if bytes[cursor] == b'`' {
                closed = true;
                break;
            }
            command.push(bytes[cursor] as char);
            cursor += 1;
        }
        if !closed {
            index += 1;
            continue;
        }
        found.push(Expression {
            start: index,
            end: cursor + 1,
            command,
        });
        index = cursor + 1;
    }
    found
}

fn is_executable_file(path: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    if !meta.is_file() {
        return false;
    }
    let Ok(cstr) = CString::new(path.as_os_str().as_bytes()) else {
        return false;
    };
    unsafe { libc::access(cstr.as_ptr(), libc::X_OK) == 0 }
}

fn shell_candidates(shell: &str, environment: &HashMap<String, String>) -> Vec<PathBuf> {
    if Path::new(shell).is_absolute() || shell.contains('/') {
        vec![PathBuf::from(shell)]
    } else {
        environment
            .get("PATH")
            .map(String::as_str)
            .unwrap_or("")
            .split(':')
            .map(|part| {
                if part.is_empty() {
                    PathBuf::from(shell)
                } else {
                    Path::new(part).join(shell)
                }
            })
            .collect()
    }
}

fn not_started(shell: &str) -> QcError {
    QcError::new(format!("shell executable could not be started: {shell}"))
}

fn drain_stderr(mut stderr: impl Read + Send + 'static) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut buf = [0u8; 8192];
        while stderr.read(&mut buf).unwrap_or(0) > 0 {}
    })
}

/// Probe even when there are no expressions. Spawn has no cwd (Node default:
/// process cwd = invocation cwd). `cli.ts` still passes effective workdir into
/// `expandShell` as the expression cwd.
fn probe_shell(shell: &str, environment: &HashMap<String, String>) -> Result<(), QcError> {
    let mut selected = None;
    for candidate in shell_candidates(shell, environment) {
        if is_executable_file(&candidate) {
            selected = Some(candidate);
            break;
        }
    }
    if selected.is_none() {
        return Err(not_started(shell));
    }
    let mut child = Command::new(shell)
        .arg("-c")
        .arg(":")
        .env_clear()
        .envs(environment)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| not_started(shell))?;
    let stderr = child.stderr.take();
    let drain = stderr.map(drain_stderr);
    let status = child.wait().map_err(|_| not_started(shell))?;
    if let Some(handle) = drain {
        let _ = handle.join();
    }
    if status.success() {
        Ok(())
    } else {
        Err(not_started(shell))
    }
}

struct Running {
    child: std::process::Child,
    stdout: std::process::ChildStdout,
    stderr_thread: thread::JoinHandle<()>,
}

fn start_expression(
    shell: &str,
    command: &str,
    cwd: &Path,
    environment: &HashMap<String, String>,
) -> Result<Running, QcError> {
    let mut child = Command::new(shell)
        .arg("-c")
        .arg(command)
        .current_dir(cwd)
        .env_clear()
        .envs(environment)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| not_started(shell))?;
    let stdout = child.stdout.take().ok_or_else(|| not_started(shell))?;
    let stderr = child.stderr.take().ok_or_else(|| not_started(shell))?;
    Ok(Running {
        child,
        stdout,
        stderr_thread: drain_stderr(stderr),
    })
}

fn finish_expression(mut running: Running) -> String {
    let mut out = Vec::new();
    let _ = running.stdout.read_to_end(&mut out);
    let _ = running.child.wait();
    let _ = running.stderr_thread.join();
    String::from_utf8_lossy(&out).into_owned()
}

pub fn expand_shell(
    text: &str,
    shell: &str,
    _config: &Config,
    cwd: &Path,
    environment: &HashMap<String, String>,
) -> Result<String, QcError> {
    let found = expressions(text);
    probe_shell(shell, environment)?;
    if found.is_empty() {
        return Ok(text.to_string());
    }
    // Own lifecycle: concurrent spawn + wait. No provider setsid, 2s drain, or group KILL.
    let mut running = Vec::with_capacity(found.len());
    for expression in &found {
        running.push(start_expression(
            shell,
            &expression.command,
            cwd,
            environment,
        )?);
    }
    let outputs: Vec<String> = running.into_iter().map(finish_expression).collect();
    let mut output = String::new();
    let mut cursor = 0;
    for (expression, replacement) in found.iter().zip(outputs.iter()) {
        output.push_str(&text[cursor..expression.start]);
        output.push_str(replacement);
        cursor = expression.end;
    }
    output.push_str(&text[cursor..]);
    Ok(output)
}
