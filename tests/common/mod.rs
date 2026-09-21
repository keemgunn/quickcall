#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static COUNTER: AtomicU64 = AtomicU64::new(0);

pub fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

pub fn scratch_root() -> PathBuf {
    crate_root().join(".tmp")
}

pub struct Scratch {
    pub root: PathBuf,
}

impl Scratch {
    pub fn home(&self) -> PathBuf {
        self.root.join("home")
    }

    pub fn cwd(&self) -> PathBuf {
        self.root.join("work")
    }

    pub fn bin(&self) -> PathBuf {
        self.root.join("bin")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

pub fn copy_dir_all(src: &Path, dest: &Path) {
    fs::create_dir_all(dest).expect("copy dest");
    for entry in fs::read_dir(src).expect("copy src") {
        let entry = entry.expect("copy entry");
        let to = dest.join(entry.file_name());
        if entry.file_type().expect("copy type").is_dir() {
            copy_dir_all(&entry.path(), &to);
        } else {
            fs::copy(entry.path(), to).expect("copy file");
        }
    }
}

pub fn write_file(path: &Path, body: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("write parent");
    }
    fs::write(path, body).expect("write file");
}

pub fn unique_scratch(label: &str) -> Scratch {
    let root = scratch_root();
    fs::create_dir_all(&root).expect("child .tmp");
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = root.join(format!("{label}-{}-{stamp}-{n}", std::process::id()));
    fs::create_dir_all(&path).expect("scratch");
    fs::create_dir_all(path.join("home")).expect("home");
    fs::create_dir_all(path.join("work")).expect("cwd");
    fs::create_dir_all(path.join("bin")).expect("bin");
    Scratch { root: path }
}

#[allow(dead_code)]
pub fn host_triple() -> &'static str {
    match (std::env::consts::ARCH, std::env::consts::OS) {
        ("aarch64", "macos") => "aarch64-apple-darwin",
        ("x86_64", "macos") => "x86_64-apple-darwin",
        ("x86_64", "linux") => "x86_64-unknown-linux-gnu",
        ("aarch64", "linux") => "aarch64-unknown-linux-gnu",
        _ => panic!(
            "unsupported host {}-{}",
            std::env::consts::ARCH,
            std::env::consts::OS
        ),
    }
}

#[allow(dead_code)]
pub fn stage_package(scratch: &Scratch, version: &str) -> PathBuf {
    let triple = host_triple();
    let dist = scratch.root.join("dist").join(triple);
    fs::create_dir_all(&dist).expect("dist triple");
    let qc_src = PathBuf::from(env!("CARGO_BIN_EXE_qc"));
    let bootstrap_src = PathBuf::from(env!("CARGO_BIN_EXE_qc-bootstrap"));
    let qc_dest = dist.join("qc");
    fs::copy(&qc_src, &qc_dest).expect("copy qc");
    fs::copy(&bootstrap_src, dist.join("qc-bootstrap")).expect("copy qc-bootstrap");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&qc_dest).expect("qc meta").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&qc_dest, perms.clone()).expect("chmod qc");
        fs::set_permissions(dist.join("qc-bootstrap"), perms).expect("chmod bootstrap");
    }
    copy_dir_all(&crate_root().join("share"), &scratch.root.join("share"));
    fs::write(
        scratch.root.join("package.json"),
        format!("{{\n  \"name\": \"@keemgunn/quickcall\",\n  \"version\": \"{version}\"\n}}\n"),
    )
    .expect("package.json");
    qc_dest
}

#[cfg(feature = "test-agent")]
pub fn install_fixture_names(bin_dir: &Path) {
    let agent = PathBuf::from(env!("CARGO_BIN_EXE_qc-test-agent"));
    for name in ["pi", "agent", "claude", "opencode", "agy", "codex"] {
        let dest = bin_dir.join(name);
        #[cfg(unix)]
        {
            let _ = fs::remove_file(&dest);
            std::os::unix::fs::symlink(&agent, &dest).expect("symlink fixture");
        }
        #[cfg(not(unix))]
        {
            fs::copy(&agent, &dest).expect("copy fixture");
        }
    }
}

pub fn pid_alive(pid: i32) -> bool {
    nix::sys::signal::kill(nix::unistd::Pid::from_raw(pid), None).is_ok()
}

pub struct GroupGuard {
    pub pgid: i32,
}

impl Drop for GroupGuard {
    fn drop(&mut self) {
        if self.pgid > 0 {
            let _ = nix::sys::signal::killpg(
                nix::unistd::Pid::from_raw(self.pgid),
                nix::sys::signal::Signal::SIGKILL,
            );
        }
    }
}

fn chmod_755(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(path).expect("meta").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms).expect("chmod");
    }
}

/// Installed-layout dummy `qc` so `resolve_package` works for in-process `app::run`.
pub fn dummy_installed(scratch: &Scratch) -> PathBuf {
    let dist = scratch.root.join("dist").join("test-triple");
    fs::create_dir_all(&dist).expect("dist");
    let exe = dist.join("qc");
    fs::write(&exe, b"dummy").expect("dummy qc");
    chmod_755(&exe);
    copy_dir_all(&crate_root().join("share"), &scratch.root.join("share"));
    fs::write(
        scratch.root.join("package.json"),
        r#"{ "name": "@keemgunn/quickcall", "version": "0.0.0" }"#,
    )
    .expect("manifest");
    exe
}

pub fn args(tokens: &[&str]) -> Vec<String> {
    tokens.iter().map(|token| (*token).to_string()).collect()
}

pub fn fixture_env(scratch: &Scratch) -> std::collections::HashMap<String, String> {
    let mut environment = std::collections::HashMap::new();
    environment.insert("HOME".into(), scratch.home().to_string_lossy().into_owned());
    environment.insert(
        "PATH".into(),
        format!("{}:/usr/bin:/bin", scratch.bin().display()),
    );
    environment.insert("SHELL".into(), "/bin/sh".into());
    environment
}

pub fn wait_for_file(path: &Path, timeout_ms: u64) {
    let start = std::time::Instant::now();
    while start.elapsed() < std::time::Duration::from_millis(timeout_ms) {
        if path.is_file() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    panic!("timed out waiting for {}", path.display());
}

pub fn command_on_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

pub fn require_cmd(name: &str) -> PathBuf {
    command_on_path(name).unwrap_or_else(|| panic!("{name} must be on PATH"))
}

pub fn node_dir() -> PathBuf {
    require_cmd("node")
        .parent()
        .expect("node parent")
        .to_path_buf()
}
