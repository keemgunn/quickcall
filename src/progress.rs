use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::messages::{error, wait_line, warn};

pub trait Progress: Send {
    fn start(&self);
    fn stop(&self);
    fn warn(&self, detail: &str);
    fn fail(&self, detail: &str);
    /// Test seam: apply one timer tick using the injected clock.
    fn tick(&self);
    fn captured(&self) -> String;
}

struct Noop;

impl Progress for Noop {
    fn start(&self) {}
    fn stop(&self) {}
    fn warn(&self, _detail: &str) {}
    fn fail(&self, _detail: &str) {}
    fn tick(&self) {}
    fn captured(&self) -> String {
        String::new()
    }
}

#[derive(Clone, Default)]
pub struct Capture {
    inner: Arc<Mutex<Vec<u8>>>,
}

impl Capture {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.inner.lock().expect("capture")).into_owned()
    }
}

impl Write for Capture {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.lock().expect("capture").extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub struct CreateProgressOptions {
    pub live: bool,
    pub tool: String,
    pub model: Option<String>,
    /// Injected writer for tests. Production uses an internal buffer plus optional TTY paint.
    pub stream: Option<Capture>,
    pub is_tty: bool,
    pub now_ms: Option<Arc<dyn Fn() -> u64 + Send + Sync>>,
}

struct LiveProgress {
    capture: Capture,
    tty: bool,
    paint_stderr: bool,
    tool: String,
    model: Option<String>,
    now_ms: Arc<dyn Fn() -> u64 + Send + Sync>,
    started_ms: Mutex<u64>,
    spinning: AtomicBool,
    timer: Mutex<Option<JoinHandle<()>>>,
    stop: Arc<AtomicBool>,
}

fn stderr_is_tty() -> bool {
    // SAFETY: isatty on STDERR_FILENO is a query; it does not mutate terminal state.
    unsafe { libc::isatty(libc::STDERR_FILENO) != 0 }
}

fn wall_clock() -> u64 {
    static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    let start = START.get_or_init(Instant::now);
    start.elapsed().as_millis() as u64
}

fn clear_line() {
    let _ = write!(io::stderr(), "\r\x1b[2K");
    let _ = io::stderr().flush();
}

fn bouncing_frame(i: u64) -> &'static str {
    // Frames may differ from ora bouncingBar; waitLine text must match.
    const FRAMES: [&str; 4] = ["[=---]", "[-=--]", "[--=-]", "[---=]"];
    FRAMES[(i as usize) % FRAMES.len()]
}

impl LiveProgress {
    fn persist(&self, line: &str) {
        let rendered = format!("{line}\n");
        let mut stream = self.capture.clone();
        let _ = stream.write_all(rendered.as_bytes());
        if self.paint_stderr {
            if self.spinning.load(Ordering::SeqCst) {
                clear_line();
            }
            let _ = write!(io::stderr(), "{rendered}");
            let _ = io::stderr().flush();
            if self.spinning.load(Ordering::SeqCst) {
                self.render_spinner(false);
            }
        }
    }

    fn elapsed_seconds(&self) -> u64 {
        let now = (self.now_ms)();
        let started = *self.started_ms.lock().expect("started");
        now.saturating_sub(started) / 1000
    }

    fn wait_text(&self) -> String {
        wait_line(&self.tool, self.elapsed_seconds(), self.model.as_deref())
    }

    fn render_spinner(&self, capture: bool) {
        if !self.tty {
            return;
        }
        let text = format!(
            "{}{}",
            bouncing_frame(self.elapsed_seconds()),
            self.wait_text()
        );
        if self.paint_stderr {
            let _ = write!(io::stderr(), "\r{text}");
            let _ = io::stderr().flush();
        }
        if capture {
            let mut stream = self.capture.clone();
            let _ = write!(stream, "{text}");
        }
    }
}

impl Progress for LiveProgress {
    fn start(&self) {
        *self.started_ms.lock().expect("started") = (self.now_ms)();
        if self.tty {
            self.spinning.store(true, Ordering::SeqCst);
            // Tests assert waitLine on the capture; production paints stderr only.
            self.render_spinner(!self.paint_stderr);
        }
        if self.paint_stderr && self.tty {
            let stop = Arc::clone(&self.stop);
            let tool = self.tool.clone();
            let model = self.model.clone();
            let now = Arc::clone(&self.now_ms);
            let started_at = *self.started_ms.lock().expect("started");
            let handle = thread::spawn(move || {
                let mut i = 0u64;
                while !stop.load(Ordering::SeqCst) {
                    thread::sleep(Duration::from_millis(80));
                    if stop.load(Ordering::SeqCst) {
                        break;
                    }
                    let elapsed = now().saturating_sub(started_at) / 1000;
                    let frame = bouncing_frame(i);
                    i += 1;
                    let text = wait_line(&tool, elapsed, model.as_deref());
                    let _ = write!(io::stderr(), "\r{frame}{text}");
                    let _ = io::stderr().flush();
                }
            });
            *self.timer.lock().expect("timer") = Some(handle);
        }
    }

    fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self.timer.lock().expect("timer").take() {
            let _ = handle.join();
        }
        if self.spinning.swap(false, Ordering::SeqCst) && self.paint_stderr {
            clear_line();
        }
    }

    fn warn(&self, detail: &str) {
        self.persist(&warn(detail));
    }

    fn fail(&self, detail: &str) {
        self.persist(&error(detail));
    }

    fn tick(&self) {
        if self.tty {
            self.render_spinner(true);
        }
    }

    fn captured(&self) -> String {
        self.capture.text()
    }
}

impl Drop for LiveProgress {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Live stderr spinner + tagged lines, or a quiet no-op. Spinner wraps `run_agent` only.
/// Never consumes stdin (`discardStdin: false`).
pub fn create_progress(options: CreateProgressOptions) -> Box<dyn Progress> {
    if !options.live {
        return Box::new(Noop);
    }
    let injected = options.stream.is_some();
    let capture = options.stream.unwrap_or_default();
    let tty = options.is_tty;
    let now_ms: Arc<dyn Fn() -> u64 + Send + Sync> =
        options.now_ms.unwrap_or_else(|| Arc::new(wall_clock));
    let paint_stderr = tty && !injected && stderr_is_tty();
    Box::new(LiveProgress {
        capture,
        tty,
        paint_stderr,
        tool: options.tool,
        model: options.model,
        now_ms,
        started_ms: Mutex::new(0),
        spinning: AtomicBool::new(false),
        timer: Mutex::new(None),
        stop: Arc::new(AtomicBool::new(false)),
    })
}

pub fn stderr_tty() -> bool {
    stderr_is_tty()
}
