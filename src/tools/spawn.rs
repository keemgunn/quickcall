//! Headless and interactive provider spawn. §6 process contract.

use std::collections::HashMap;
use std::ffi::CString;
use std::io::{self, ErrorKind};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use libc::{self, c_int};
use nix::errno::Errno;
use nix::sys::signal::{SigSet, SigmaskHow, Signal, kill, killpg, pthread_sigmask};
use nix::sys::wait::waitpid;
use nix::unistd::Pid;

use crate::errors::QcError;
use crate::messages::missing_binary;

const DRAIN: Duration = Duration::from_millis(2_000);
const CLEANUP_GRACE: Duration = Duration::from_millis(1_000);
const POLL_TICK: Duration = Duration::from_millis(50);

static SPAWN_GATE: Mutex<()> = Mutex::new(());
static TARGET_PID: AtomicI32 = AtomicI32::new(0);
static FORWARD_GROUP: AtomicBool = AtomicBool::new(false);
static INT_SEEN: AtomicBool = AtomicBool::new(false);
static TERM_SEEN: AtomicBool = AtomicBool::new(false);
static WAKE_WR: AtomicI32 = AtomicI32::new(-1);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpawnCapture {
    pub stdout: String,
    pub stderr: String,
    pub exit: i32,
    pub pid: u32,
}

fn gate() -> MutexGuard<'static, ()> {
    SPAWN_GATE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
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
    // SAFETY: cstr is a valid path C string; access does not dereference user memory.
    unsafe { libc::access(cstr.as_ptr(), libc::X_OK) == 0 }
}

fn resolve_command(command: &str, environment: &HashMap<String, String>) -> Option<PathBuf> {
    let path = Path::new(command);
    if path.is_absolute() || command.contains('/') {
        return Some(PathBuf::from(command));
    }
    let search = environment.get("PATH").map(String::as_str).unwrap_or("");
    for part in search.split(':') {
        let candidate = if part.is_empty() {
            PathBuf::from(command)
        } else {
            Path::new(part).join(command)
        };
        if is_executable_file(&candidate) {
            return Some(candidate);
        }
    }
    None
}

fn spawn_fail(tool_label: &str, command: &str, err: io::Error) -> QcError {
    if err.kind() == ErrorKind::NotFound {
        QcError::new(missing_binary(tool_label, command))
    } else {
        QcError::new(format!("could not start {command}: {err}"))
    }
}

fn set_cloexec_nonblock(fd: RawFd) -> io::Result<()> {
    // SAFETY: fd is a freshly created pipe end owned by this process.
    unsafe {
        let flags = libc::fcntl(fd, libc::F_GETFD);
        if flags < 0 {
            return Err(io::Error::last_os_error());
        }
        if libc::fcntl(fd, libc::F_SETFD, flags | libc::FD_CLOEXEC) < 0 {
            return Err(io::Error::last_os_error());
        }
        let flags = libc::fcntl(fd, libc::F_GETFL);
        if flags < 0 {
            return Err(io::Error::last_os_error());
        }
        if libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) < 0 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}

fn make_wakeup_pipe() -> io::Result<(OwnedFd, OwnedFd)> {
    let mut fds = [0; 2];
    // SAFETY: pipe writes two valid fds on success.
    let rc = unsafe { libc::pipe(fds.as_mut_ptr()) };
    if rc != 0 {
        return Err(io::Error::last_os_error());
    }
    set_cloexec_nonblock(fds[0])?;
    set_cloexec_nonblock(fds[1])?;
    // SAFETY: unique ownership of the new pipe ends.
    unsafe { Ok((OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1]))) }
}

fn set_nonblock(fd: RawFd) -> io::Result<()> {
    // SAFETY: fd is a live stdio pipe from Command.
    unsafe {
        let flags = libc::fcntl(fd, libc::F_GETFL);
        if flags < 0 {
            return Err(io::Error::last_os_error());
        }
        if libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) < 0 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}

extern "C" fn handle_signal(sig: c_int) {
    let seen = if sig == libc::SIGINT {
        &INT_SEEN
    } else {
        &TERM_SEEN
    };
    if seen.swap(true, Ordering::SeqCst) {
        // Repeated same signal: restore default-terminate.
        // SAFETY: async-signal-safe signal/kill.
        unsafe {
            libc::signal(sig, libc::SIG_DFL);
            libc::kill(libc::getpid(), sig);
        }
        return;
    }
    let pid = TARGET_PID.load(Ordering::SeqCst);
    if pid > 0 {
        let target = if FORWARD_GROUP.load(Ordering::SeqCst) {
            -pid
        } else {
            pid
        };
        // SAFETY: kill is async-signal-safe; pid is the recorded child.
        unsafe {
            libc::kill(target, sig);
        }
    }
    let wr = WAKE_WR.load(Ordering::SeqCst);
    if wr >= 0 {
        let byte = sig as u8;
        // SAFETY: wakeup pipe write end stays open for the spawn lifetime.
        unsafe {
            libc::write(wr, std::ptr::from_ref(&byte).cast(), 1);
        }
    }
}

struct PreviousHandler {
    signal: c_int,
    action: libc::sigaction,
}

fn empty_sigaction() -> libc::sigaction {
    // SAFETY: zeroed sigaction is the starting point before we fill fields.
    unsafe { std::mem::zeroed() }
}

fn install_int_term() -> io::Result<Vec<PreviousHandler>> {
    let mut previous = Vec::new();
    for sig in [libc::SIGINT, libc::SIGTERM] {
        let mut old = empty_sigaction();
        let mut new = empty_sigaction();
        new.sa_sigaction = handle_signal as *const () as usize;
        new.sa_flags = libc::SA_RESTART;
        // SAFETY: installing a process-wide handler; caller holds SPAWN_GATE.
        let rc = unsafe { libc::sigaction(sig, &new, &mut old) };
        if rc != 0 {
            restore_int_term(&previous);
            return Err(io::Error::last_os_error());
        }
        previous.push(PreviousHandler {
            signal: sig,
            action: old,
        });
    }
    Ok(previous)
}

fn restore_int_term(previous: &[PreviousHandler]) {
    for handler in previous {
        // SAFETY: restoring the action we saved at install.
        unsafe {
            libc::sigaction(handler.signal, &handler.action, std::ptr::null_mut());
        }
    }
}

fn child_pre_exec(use_setsid: bool) -> io::Result<()> {
    if use_setsid {
        // SAFETY: pre-exec, async-signal-safe; creates a new session/group.
        if unsafe { libc::setsid() } == -1 {
            return Err(io::Error::last_os_error());
        }
    }
    // SAFETY: restore default dispositions before exec; async-signal-safe.
    unsafe {
        libc::signal(libc::SIGINT, libc::SIG_DFL);
        libc::signal(libc::SIGTERM, libc::SIG_DFL);
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
        let mut set = std::mem::zeroed();
        libc::sigemptyset(&mut set);
        libc::sigaddset(&mut set, libc::SIGINT);
        libc::sigaddset(&mut set, libc::SIGTERM);
        libc::sigaddset(&mut set, libc::SIGCHLD);
        libc::pthread_sigmask(libc::SIG_UNBLOCK, &set, std::ptr::null_mut());
    }
    Ok(())
}

fn block_int_term() -> nix::Result<SigSet> {
    let mut set = SigSet::empty();
    set.add(Signal::SIGINT);
    set.add(Signal::SIGTERM);
    let mut old = SigSet::empty();
    pthread_sigmask(SigmaskHow::SIG_BLOCK, Some(&set), Some(&mut old))?;
    Ok(old)
}

fn restore_mask(old: &SigSet) {
    let _ = pthread_sigmask(SigmaskHow::SIG_SETMASK, Some(old), None);
}

fn observe_exit(pid: Pid) -> Option<i32> {
    let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
    // SAFETY: waitid with WNOWAIT observes without reaping; info is a valid out-param.
    let rc = unsafe {
        libc::waitid(
            libc::P_PID,
            pid.as_raw() as libc::id_t,
            &mut info,
            libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
        )
    };
    if rc != 0 {
        return None;
    }
    // SAFETY: waitid filled siginfo; si_pid 0 means WNOHANG found nobody.
    let child = unsafe { info.si_pid() };
    if child == 0 {
        return None;
    }
    let status = unsafe { info.si_status() };
    match info.si_code {
        libc::CLD_EXITED => Some(status),
        libc::CLD_KILLED | libc::CLD_DUMPED => Some(128 + status),
        _ => Some(1),
    }
}

fn poll_fds(fds: &mut [libc::pollfd], timeout: Duration) {
    let ms = timeout.as_millis().min(i32::MAX as u128) as c_int;
    // SAFETY: fds points at live pollfd entries for the duration of the call.
    unsafe {
        libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, ms);
    }
}

fn read_fd(fd: RawFd, buf: &mut Vec<u8>) -> bool {
    let mut tmp = [0u8; 8192];
    loop {
        // SAFETY: fd is a live nonblocking pipe.
        let n = unsafe { libc::read(fd, tmp.as_mut_ptr().cast(), tmp.len()) };
        if n > 0 {
            buf.extend_from_slice(&tmp[..n as usize]);
            continue;
        }
        if n == 0 {
            return true;
        }
        let err = io::Error::last_os_error();
        match err.raw_os_error() {
            Some(libc::EAGAIN) | Some(libc::EINTR) => return false,
            _ => return true,
        }
    }
}

fn write_fd(fd: RawFd, data: &[u8], offset: &mut usize) -> bool {
    while *offset < data.len() {
        let rest = &data[*offset..];
        // SAFETY: fd is a live nonblocking stdin pipe.
        let n = unsafe { libc::write(fd, rest.as_ptr().cast(), rest.len()) };
        if n > 0 {
            *offset += n as usize;
            continue;
        }
        if n == 0 {
            return true;
        }
        let err = io::Error::last_os_error();
        match err.raw_os_error() {
            Some(libc::EAGAIN) | Some(libc::EINTR) => return false,
            Some(libc::EPIPE) => return true,
            _ => return true,
        }
    }
    true
}

fn drain_wakeup(fd: RawFd) {
    let mut tmp = [0u8; 32];
    loop {
        // SAFETY: wakeup pipe is nonblocking and owned by this spawn.
        let n = unsafe { libc::read(fd, tmp.as_mut_ptr().cast(), tmp.len()) };
        if n > 0 {
            continue;
        }
        break;
    }
}

fn group_exists(pgid: Pid) -> bool {
    match kill(Pid::from_raw(-pgid.as_raw()), None) {
        Ok(()) => true,
        Err(Errno::ESRCH) => false,
        Err(_) => true,
    }
}

fn cleanup_group(pid: Pid) {
    let _ = killpg(pid, Signal::SIGTERM);
    let _ = waitpid(pid, None);
    if !group_exists(pid) {
        return;
    }
    let deadline = Instant::now() + CLEANUP_GRACE;
    while Instant::now() < deadline {
        if !group_exists(pid) {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    if group_exists(pid) {
        let _ = killpg(pid, Signal::SIGKILL);
    }
}

fn decode(bytes: Vec<u8>) -> String {
    String::from_utf8_lossy(&bytes).into_owned()
}

/// Spawn a tool binary, feed stdin, capture stdout and stderr. Never shells the prompt.
pub fn spawn_agent(
    command: &str,
    args: &[String],
    prompt: Option<&str>,
    cwd: &str,
    environment: &HashMap<String, String>,
    tool_label: &str,
) -> Result<SpawnCapture, QcError> {
    let _gate = gate();
    let resolved = resolve_command(command, environment)
        .ok_or_else(|| QcError::new(missing_binary(tool_label, command)))?;

    INT_SEEN.store(false, Ordering::SeqCst);
    TERM_SEEN.store(false, Ordering::SeqCst);
    TARGET_PID.store(0, Ordering::SeqCst);
    FORWARD_GROUP.store(true, Ordering::SeqCst);

    let old_mask = block_int_term().map_err(|err| QcError::new(err.to_string()))?;
    let wakeup = match make_wakeup_pipe() {
        Ok(pair) => pair,
        Err(err) => {
            restore_mask(&old_mask);
            return Err(QcError::new(format!("could not start {command}: {err}")));
        }
    };
    let (wake_rd, wake_wr) = wakeup;
    WAKE_WR.store(wake_wr.as_raw_fd(), Ordering::SeqCst);

    let handlers = match install_int_term() {
        Ok(prev) => prev,
        Err(err) => {
            WAKE_WR.store(-1, Ordering::SeqCst);
            restore_mask(&old_mask);
            return Err(QcError::new(format!("could not start {command}: {err}")));
        }
    };
    let chld = wake_wr
        .try_clone()
        .ok()
        .and_then(|dup| signal_hook::low_level::pipe::register(libc::SIGCHLD, dup).ok());

    let stdin_cfg = if prompt.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    };
    let mut cmd = Command::new(&resolved);
    cmd.args(args)
        .current_dir(cwd)
        .env_clear()
        .envs(environment)
        .stdin(stdin_cfg)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // SAFETY: pre-exec only calls setsid/signal/sigprocmask (async-signal-safe).
    unsafe {
        cmd.pre_exec(|| child_pre_exec(true));
    }

    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(err) => {
            if let Some(id) = chld {
                signal_hook::low_level::unregister(id);
            }
            restore_int_term(&handlers);
            WAKE_WR.store(-1, Ordering::SeqCst);
            restore_mask(&old_mask);
            return Err(spawn_fail(tool_label, command, err));
        }
    };

    let pid_u = child.id();
    let pid = Pid::from_raw(pid_u as i32);
    TARGET_PID.store(pid.as_raw(), Ordering::SeqCst);
    restore_mask(&old_mask);

    let mut stdin = child.stdin.take();
    let mut stdout = child.stdout.take();
    let mut stderr = child.stderr.take();
    if let Some(ref s) = stdin {
        let _ = set_nonblock(s.as_raw_fd());
    }
    if let Some(ref s) = stdout {
        let _ = set_nonblock(s.as_raw_fd());
    }
    if let Some(ref s) = stderr {
        let _ = set_nonblock(s.as_raw_fd());
    }

    let stdin_bytes = prompt.map(|p| p.as_bytes().to_vec());
    let mut stdin_off = 0usize;
    let mut stdout_buf = Vec::new();
    let mut stderr_buf = Vec::new();
    let mut frozen: Option<i32> = None;
    let mut drain_deadline: Option<Instant> = None;

    let result = (|| -> Result<SpawnCapture, QcError> {
        loop {
            if frozen.is_none()
                && let Some(status) = observe_exit(pid)
            {
                frozen = Some(status);
                drain_deadline = Some(Instant::now() + DRAIN);
            }

            let both_closed = stdout.is_none() && stderr.is_none();
            if let Some(exit) = frozen
                && (both_closed || drain_deadline.is_some_and(|d| Instant::now() >= d))
            {
                return Ok(SpawnCapture {
                    stdout: decode(std::mem::take(&mut stdout_buf)),
                    stderr: decode(std::mem::take(&mut stderr_buf)),
                    exit,
                    pid: pid_u,
                });
            }

            let timeout = match drain_deadline {
                Some(deadline) => {
                    let remain = deadline.saturating_duration_since(Instant::now());
                    remain.min(POLL_TICK)
                }
                None => POLL_TICK,
            };

            let mut fds: Vec<libc::pollfd> = Vec::new();
            let mut map: Vec<u8> = Vec::new();
            if let Some(ref s) = stdout {
                fds.push(libc::pollfd {
                    fd: s.as_raw_fd(),
                    events: libc::POLLIN | libc::POLLHUP,
                    revents: 0,
                });
                map.push(0);
            }
            if let Some(ref s) = stderr {
                fds.push(libc::pollfd {
                    fd: s.as_raw_fd(),
                    events: libc::POLLIN | libc::POLLHUP,
                    revents: 0,
                });
                map.push(1);
            }
            if let Some(s) = stdin.as_ref()
                && stdin_bytes.as_ref().is_some_and(|b| stdin_off < b.len())
            {
                fds.push(libc::pollfd {
                    fd: s.as_raw_fd(),
                    events: libc::POLLOUT | libc::POLLHUP,
                    revents: 0,
                });
                map.push(2);
            }
            fds.push(libc::pollfd {
                fd: wake_rd.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            });
            map.push(3);
            poll_fds(&mut fds, timeout);

            let mut stdout_hang = false;
            let mut stderr_hang = false;
            let mut stdin_hang = false;
            for (slot, kind) in map.iter().enumerate() {
                let revents = fds.get(slot).map(|p| p.revents).unwrap_or(0);
                let hung = revents & (libc::POLLHUP | libc::POLLERR | libc::POLLNVAL) != 0;
                let ready = revents & (libc::POLLIN | libc::POLLOUT) != 0;
                match kind {
                    0 => {
                        if (ready || hung)
                            && let Some(ref s) = stdout
                        {
                            let eof = read_fd(s.as_raw_fd(), &mut stdout_buf);
                            stdout_hang = eof || hung;
                        }
                    }
                    1 => {
                        if (ready || hung)
                            && let Some(ref s) = stderr
                        {
                            let eof = read_fd(s.as_raw_fd(), &mut stderr_buf);
                            stderr_hang = eof || hung;
                        }
                    }
                    2 => {
                        if (ready || hung)
                            && let (Some(s), Some(bytes)) = (&stdin, &stdin_bytes)
                        {
                            let done = write_fd(s.as_raw_fd(), bytes, &mut stdin_off);
                            stdin_hang = done || hung;
                        }
                    }
                    3 if ready => drain_wakeup(wake_rd.as_raw_fd()),
                    _ => {}
                }
            }
            if stdout_hang {
                stdout.take();
            }
            if stderr_hang {
                stderr.take();
            }
            if stdin_hang || stdin_bytes.as_ref().is_some_and(|b| stdin_off >= b.len()) {
                stdin.take();
            }
        }
    })();

    drop(stdin);
    drop(stdout);
    drop(stderr);

    cleanup_group(pid);

    TARGET_PID.store(0, Ordering::SeqCst);
    FORWARD_GROUP.store(false, Ordering::SeqCst);
    if let Some(id) = chld {
        signal_hook::low_level::unregister(id);
    }
    restore_int_term(&handlers);
    WAKE_WR.store(-1, Ordering::SeqCst);
    drop(wake_wr);
    drop(wake_rd);
    drop(child);

    result
}

/// Silent interactive spawn: inherit stdio. SIGINT/SIGTERM forwarded to the direct PID.
pub fn spawn_interactive(
    command: &str,
    args: &[String],
    cwd: &str,
    environment: &HashMap<String, String>,
    tool_label: &str,
    on_spawn: Option<&dyn Fn() -> Result<(), QcError>>,
) -> Result<i32, QcError> {
    let _gate = gate();
    let resolved = resolve_command(command, environment)
        .ok_or_else(|| QcError::new(missing_binary(tool_label, command)))?;

    INT_SEEN.store(false, Ordering::SeqCst);
    TERM_SEEN.store(false, Ordering::SeqCst);
    TARGET_PID.store(0, Ordering::SeqCst);
    FORWARD_GROUP.store(false, Ordering::SeqCst);

    let old_mask = block_int_term().map_err(|err| QcError::new(err.to_string()))?;
    let wakeup = match make_wakeup_pipe() {
        Ok(pair) => pair,
        Err(err) => {
            restore_mask(&old_mask);
            return Err(QcError::new(format!("could not start {command}: {err}")));
        }
    };
    let (wake_rd, wake_wr) = wakeup;
    WAKE_WR.store(wake_wr.as_raw_fd(), Ordering::SeqCst);
    let handlers = match install_int_term() {
        Ok(prev) => prev,
        Err(err) => {
            WAKE_WR.store(-1, Ordering::SeqCst);
            restore_mask(&old_mask);
            return Err(QcError::new(format!("could not start {command}: {err}")));
        }
    };
    let chld = wake_wr
        .try_clone()
        .ok()
        .and_then(|dup| signal_hook::low_level::pipe::register(libc::SIGCHLD, dup).ok());

    let mut cmd = Command::new(&resolved);
    cmd.args(args)
        .current_dir(cwd)
        .env_clear()
        .envs(environment)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    // SAFETY: pre-exec restores mask/dispositions only; no setsid for TUI.
    unsafe {
        cmd.pre_exec(|| child_pre_exec(false));
    }

    let child = match cmd.spawn() {
        Ok(child) => child,
        Err(err) => {
            if let Some(id) = chld {
                signal_hook::low_level::unregister(id);
            }
            restore_int_term(&handlers);
            WAKE_WR.store(-1, Ordering::SeqCst);
            restore_mask(&old_mask);
            return Err(spawn_fail(tool_label, command, err));
        }
    };

    let pid_u = child.id();
    let pid = Pid::from_raw(pid_u as i32);
    TARGET_PID.store(pid.as_raw(), Ordering::SeqCst);
    restore_mask(&old_mask);

    let on_spawn_err = match on_spawn {
        Some(hook) => hook().err(),
        None => None,
    };

    let exit = loop {
        if let Some(code) = observe_exit(pid) {
            let _ = waitpid(pid, None);
            break code;
        }
        let mut fds = [libc::pollfd {
            fd: wake_rd.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        }];
        poll_fds(&mut fds, POLL_TICK);
        drain_wakeup(wake_rd.as_raw_fd());
    };

    TARGET_PID.store(0, Ordering::SeqCst);
    if let Some(id) = chld {
        signal_hook::low_level::unregister(id);
    }
    restore_int_term(&handlers);
    WAKE_WR.store(-1, Ordering::SeqCst);
    drop(wake_wr);
    drop(wake_rd);
    drop(child);

    if let Some(err) = on_spawn_err {
        return Err(err);
    }
    Ok(exit)
}
