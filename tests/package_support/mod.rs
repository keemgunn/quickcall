//! Host-mode launcher / selector / packed-install helpers.

mod launcher;
mod selector;
mod tarball;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use crate::common::{Scratch, copy_dir_all, crate_root, host_triple, node_dir, require_cmd};

pub const NATIVE_TARGETS: [&str; 6] = [
    "aarch64-apple-darwin",
    "x86_64-apple-darwin",
    "aarch64-unknown-linux-gnu",
    "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-musl",
    "x86_64-unknown-linux-musl",
];

pub fn package_mode() -> String {
    std::env::var("QC_PACKAGE_MODE").unwrap_or_else(|_| "host".into())
}

pub fn is_host_mode() -> bool {
    package_mode() != "final-artifact"
}

pub fn chmod_755(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(path).expect("meta").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms).expect("chmod");
    }
}

pub fn write_manifest(dest: &Path, version: &str) {
    let source = fs::read_to_string(crate_root().join("package.json")).expect("package.json");
    let patched = source.replacen(
        "\"version\": \"0.0.0\"",
        &format!("\"version\": \"{version}\""),
        1,
    );
    fs::write(dest.join("package.json"), patched).expect("write manifest");
}

pub fn stage_installable_layout(dest: &Path, version: &str) -> PathBuf {
    fs::create_dir_all(dest).expect("pkg dest");
    let triple = host_triple();
    let dist = dest.join("dist").join(triple);
    fs::create_dir_all(&dist).expect("dist");
    let qc_src = PathBuf::from(env!("CARGO_BIN_EXE_qc"));
    let bootstrap_src = PathBuf::from(env!("CARGO_BIN_EXE_qc-bootstrap"));
    fs::copy(&qc_src, dist.join("qc")).expect("copy qc");
    fs::copy(&bootstrap_src, dist.join("qc-bootstrap")).expect("copy bootstrap");
    chmod_755(&dist.join("qc"));
    chmod_755(&dist.join("qc-bootstrap"));

    let bin_dir = dest.join("bin");
    fs::create_dir_all(&bin_dir).expect("bin");
    fs::copy(crate_root().join("bin/qc"), bin_dir.join("qc")).expect("copy launcher");
    chmod_755(&bin_dir.join("qc"));

    let scripts = dest.join("scripts");
    fs::create_dir_all(&scripts).expect("scripts");
    for name in ["native.mjs", "postinstall.mjs"] {
        fs::copy(crate_root().join("scripts").join(name), scripts.join(name)).expect(name);
    }
    // Whole scripts dir ships in the npm files list; copy siblings so pack layout matches.
    for name in ["build.sh", "test.sh"] {
        let src = crate_root().join("scripts").join(name);
        if src.is_file() {
            fs::copy(&src, scripts.join(name)).expect(name);
        }
    }

    copy_dir_all(&crate_root().join("share"), &dest.join("share"));
    for name in ["README.md", "USAGE.md", "PROVIDERS.md", "LICENSE"] {
        let src = crate_root().join(name);
        if src.is_file() {
            fs::copy(&src, dest.join(name)).expect(name);
        }
    }
    write_manifest(dest, version);
    dest.join("bin").join("qc")
}

pub fn selector_js() -> PathBuf {
    crate_root().join("scripts/native.mjs")
}

pub fn run_node(args: &[&str], home: &Path, extra_env: &[(&str, &str)]) -> Output {
    let mut cmd = Command::new(require_cmd("node"));
    cmd.args(args)
        .env_clear()
        .env("HOME", home)
        .env("PATH", format!("{}:/usr/bin:/bin", node_dir().display()))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, value) in extra_env {
        cmd.env(key, value);
    }
    cmd.output().expect("node")
}

pub fn resolve_target_json(platform: &str, arch: &str, libc: Option<&str>, home: &Path) -> Output {
    let libc_js = match libc {
        None => "null".to_string(),
        Some(value) => format!("\"{value}\""),
    };
    let script = format!(
        "import {{ resolveTarget }} from '{url}';
const libcFamily = {libc};
try {{
  const triple = resolveTarget({{ platform: '{platform}', arch: '{arch}', libcFamily }});
  process.stdout.write(triple);
}} catch (error) {{
  process.stderr.write(error.message + '\\n');
  process.exit(1);
}}
",
        url = path_to_file_url(&selector_js()),
        libc = libc_js,
        platform = platform,
        arch = arch,
    );
    run_node(&["--input-type=module", "-e", &script], home, &[])
}

pub fn path_to_file_url(path: &Path) -> String {
    format!("file://{}", path.display())
}

pub fn launcher_path_env(scratch: &Scratch) -> String {
    format!(
        "{}:{}:/usr/bin:/bin",
        scratch.bin().display(),
        node_dir().display()
    )
}

pub fn run_launcher(
    launcher: &Path,
    scratch: &Scratch,
    cwd: &Path,
    args: &[&str],
    extra: &[(&str, &str)],
) -> Output {
    let mut cmd = Command::new(launcher);
    cmd.args(args)
        .current_dir(cwd)
        .env_clear()
        .env("HOME", scratch.home())
        .env("PATH", launcher_path_env(scratch))
        .env("SHELL", "/bin/sh")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, value) in extra {
        cmd.env(key, value);
    }
    cmd.output().expect("launcher")
}

pub fn process_comm(pid: u32) -> String {
    let output = Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "comm="])
        .output()
        .expect("ps comm");
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

pub fn process_args(pid: u32) -> String {
    let output = Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "args="])
        .output()
        .expect("ps args");
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

pub fn tarball_paths(tgz: &Path) -> Vec<String> {
    let output = Command::new("tar")
        .args(["-tzf", tgz.to_str().expect("tgz utf8")])
        .output()
        .expect("tar tzf");
    assert!(
        output.status.success(),
        "tar: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .collect()
}

pub fn sha256_hex(path: &Path) -> String {
    let output = if crate::common::command_on_path("shasum").is_some() {
        Command::new("shasum")
            .args(["-a", "256"])
            .arg(path)
            .output()
            .expect("shasum")
    } else {
        Command::new("sha256sum")
            .arg(path)
            .output()
            .expect("sha256sum")
    };
    assert!(
        output.status.success(),
        "checksum failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .next()
        .expect("hex")
        .to_string()
}

pub fn assert_checksum(path: &Path, expected: &str) {
    let got = sha256_hex(path);
    assert_eq!(
        got,
        expected,
        "checksum mismatch for {}: got {got}",
        path.display()
    );
}

pub fn npm_pack(layout: &Path, dest: &Path, home: &Path) -> PathBuf {
    fs::create_dir_all(dest).expect("pack dest");
    let npm = require_cmd("npm");
    let output = Command::new(npm)
        .args(["pack", "--pack-destination"])
        .arg(dest)
        .current_dir(layout)
        .env("HOME", home)
        .env("npm_config_cache", dest.join("npm-cache"))
        .env("npm_config_update_notifier", "false")
        .output()
        .expect("npm pack");
    assert!(
        output.status.success(),
        "npm pack failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let name = stdout
        .lines()
        .rev()
        .find(|line| line.ends_with(".tgz"))
        .expect("tgz name")
        .trim();
    let packed = dest.join(Path::new(name).file_name().expect("tgz file"));
    assert!(packed.is_file(), "missing pack {packed:?}");
    packed
}

#[allow(dead_code)]
pub struct PrefixInstall {
    pub prefix: PathBuf,
    pub bin_qc: PathBuf,
    pub package_root: PathBuf,
}

pub fn npm_install_tarball(
    tgz: &Path,
    prefix: &Path,
    home: &Path,
    ignore_scripts: bool,
) -> PrefixInstall {
    fs::create_dir_all(prefix).expect("prefix");
    let npm = require_cmd("npm");
    let mut args = vec![
        "install".to_string(),
        "-g".to_string(),
        tgz.to_string_lossy().into_owned(),
        "--prefix".to_string(),
        prefix.to_string_lossy().into_owned(),
    ];
    if ignore_scripts {
        args.push("--ignore-scripts".into());
    }
    let output = Command::new(npm)
        .args(&args)
        .env("HOME", home)
        .env("npm_config_cache", prefix.join(".npm-cache"))
        .env("npm_config_update_notifier", "false")
        .output()
        .expect("npm install");
    assert!(
        output.status.success(),
        "npm install failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let bin_qc = prefix.join("bin/qc");
    let package_root = prefix.join("lib/node_modules/@keemgunn/quickcall");
    assert!(
        bin_qc.is_file() || bin_qc.is_symlink(),
        "missing {bin_qc:?}"
    );
    assert!(package_root.is_dir(), "missing {package_root:?}");
    PrefixInstall {
        prefix: prefix.to_path_buf(),
        bin_qc,
        package_root,
    }
}

pub fn pnpm_install_tarball(
    tgz: &Path,
    scratch: &Path,
    home: &Path,
    ignore_scripts: bool,
) -> Option<PrefixInstall> {
    let pnpm = crate::common::command_on_path("pnpm")?;
    // pnpm v12: PNPM_HOME/<bin> must be on PATH. Do not use --global-bin-dir.
    let pnpm_home = scratch.join("pnpm-home");
    let bin_dir = pnpm_home.join("bin");
    let store = scratch.join("pnpm-store");
    fs::create_dir_all(&bin_dir).expect("pnpm bin");
    fs::create_dir_all(&store).expect("pnpm store");
    let path = format!(
        "{}:{}",
        bin_dir.display(),
        std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".into())
    );
    let mut args = vec![
        "add".to_string(),
        "-g".to_string(),
        tgz.to_string_lossy().into_owned(),
        "--store-dir".to_string(),
        store.to_string_lossy().into_owned(),
    ];
    if ignore_scripts {
        args.push("--ignore-scripts".into());
    }
    let output = Command::new(pnpm)
        .args(&args)
        .env("HOME", home)
        .env("PNPM_HOME", &pnpm_home)
        .env("PATH", &path)
        .output()
        .expect("pnpm add -g");
    if !output.status.success() {
        panic!(
            "pnpm add -g failed: stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let bin_qc = bin_dir.join("qc");
    let package_root = find_installed_package(&pnpm_home)
        .unwrap_or_else(|| panic!("pnpm package root under {}", pnpm_home.display()));
    Some(PrefixInstall {
        prefix: scratch.to_path_buf(),
        bin_qc,
        package_root,
    })
}

fn find_installed_package(root: &Path) -> Option<PathBuf> {
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let manifest = dir.join("package.json");
        if manifest.is_file()
            && let Ok(body) = fs::read_to_string(&manifest)
            && body.contains("\"name\": \"@keemgunn/quickcall\"")
        {
            return Some(dir);
        }
        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries.flatten() {
                if entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
                    stack.push(entry.path());
                }
            }
        }
    }
    None
}
