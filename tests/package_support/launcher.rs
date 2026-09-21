use std::fs;
use std::os::unix::process::ExitStatusExt;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use nix::sys::signal::{Signal, kill};
use nix::unistd::Pid;

use crate::common::{
    crate_root, host_triple, install_fixture_names, unique_scratch, wait_for_file,
};
use crate::package_support::{
    chmod_755, launcher_path_env, process_args, process_comm, run_launcher, run_node,
    stage_installable_layout,
};

fn stdout(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn launcher_execs_native_version_without_node_parent() {
    let scratch = unique_scratch("launch-ver");
    let pkg = scratch.root.join("pkg");
    let launcher = stage_installable_layout(&pkg, "8.8.8-host");
    let output = run_launcher(&launcher, &scratch, &scratch.cwd(), &["--version"], &[]);
    assert_eq!(output.status.code(), Some(0), "stderr={}", stderr(&output));
    assert_eq!(stdout(&output), "8.8.8-host\n");
}

#[test]
fn launcher_passthrough_preserves_cwd_spaces_unicode_and_argv() {
    let scratch = unique_scratch("launch-argv");
    install_fixture_names(&scratch.bin());
    let pkg = scratch.root.join("pkg");
    let launcher = stage_installable_layout(&pkg, "0.0.0");
    let cwd = scratch.root.join("work dir").join("项目");
    fs::create_dir_all(&cwd).expect("unicode cwd");
    let record = scratch.root.join("record.json");
    let output = run_launcher(
        &launcher,
        &scratch,
        &cwd,
        &["--append", "hello 世界 and spaces", "-q"],
        &[
            ("QC_RECORD", record.to_str().unwrap()),
            ("QC_AGENT_TEXT", "ok"),
        ],
    );
    assert_eq!(output.status.code(), Some(0), "stderr={}", stderr(&output));
    let recorded: serde_json::Value =
        serde_json::from_slice(&fs::read(&record).expect("record")).expect("json");
    assert_eq!(recorded["cwd"].as_str().unwrap(), cwd.to_string_lossy());
    assert_eq!(recorded["stdin"].as_str().unwrap(), "hello 世界 and spaces");
}

#[test]
fn host_selector_cli_prints_absolute_host_binary() {
    let scratch = unique_scratch("sel-cli");
    let pkg = scratch.root.join("pkg");
    stage_installable_layout(&pkg, "0.0.0");
    let selector = pkg.join("scripts/native.mjs");
    let output = run_node(
        &[
            selector.to_str().unwrap(),
            "--select",
            "qc",
            "--package-root",
            pkg.to_str().unwrap(),
        ],
        &scratch.home(),
        &[],
    );
    assert_eq!(output.status.code(), Some(0), "stderr={}", stderr(&output));
    let selected = stdout(&output).trim().to_string();
    assert!(selected.starts_with('/'), "{selected}");
    assert!(
        selected.ends_with(&format!("dist/{}/qc", host_triple())),
        "{selected}"
    );
    assert!(!scratch.home().join(".qc").exists());
}

#[test]
fn skip_scripts_layout_still_selects_binary() {
    let scratch = unique_scratch("launch-skip");
    let pkg = scratch.root.join("pkg");
    let launcher = stage_installable_layout(&pkg, "0.0.0");
    // Selector must work when postinstall never ran.
    assert!(!scratch.home().join(".qc").exists());
    let output = run_launcher(&launcher, &scratch, &scratch.cwd(), &["--version"], &[]);
    assert_eq!(output.status.code(), Some(0), "stderr={}", stderr(&output));
    assert_eq!(stdout(&output), "0.0.0\n");
}

#[test]
fn relative_symlink_and_path_invocation_resolve_package() {
    let scratch = unique_scratch("launch-link");
    let pkg = scratch.root.join("pkg");
    stage_installable_layout(&pkg, "4.4.4");
    let link_dir = scratch.root.join("links");
    fs::create_dir_all(&link_dir).expect("links");
    // Relative target, as npm/pnpm often install.
    std::os::unix::fs::symlink("../pkg/bin/qc", link_dir.join("qc")).expect("symlink");
    let output = run_launcher(
        &link_dir.join("qc"),
        &scratch,
        &scratch.cwd(),
        &["--version"],
        &[],
    );
    assert_eq!(output.status.code(), Some(0), "stderr={}", stderr(&output));
    assert_eq!(stdout(&output), "4.4.4\n");
}

#[test]
fn launcher_exec_replaces_shell_and_forwards_sigterm() {
    let scratch = unique_scratch("launch-exec");
    install_fixture_names(&scratch.bin());
    let pkg = scratch.root.join("pkg");
    let launcher = stage_installable_layout(&pkg, "0.0.0");
    let ready = scratch.root.join("ready");
    let record = scratch.root.join("signal");
    let mut child = Command::new(&launcher)
        .args(["--append", "wait-for-signal", "-q"])
        .current_dir(scratch.cwd())
        .env_clear()
        .env("HOME", scratch.home())
        .env("PATH", launcher_path_env(&scratch))
        .env("SHELL", "/bin/sh")
        .env("QC_PI_WAIT", "1")
        .env("QC_PI_READY", &ready)
        .env("QC_PI_SIGNAL_RECORD", &record)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn launcher");
    let pid = child.id();
    wait_for_file(&ready, 12_000);
    thread::sleep(Duration::from_millis(50));
    let comm = process_comm(pid);
    assert!(
        comm == "qc" || comm.ends_with("/qc"),
        "exec must replace sh/node; comm={comm:?} args={:?}",
        process_args(pid)
    );
    let args = process_args(pid);
    assert!(
        !args.contains("node ") && !args.contains("native.mjs"),
        "node selector must not remain: {args}"
    );
    kill(Pid::from_raw(pid as i32), Signal::SIGTERM).expect("sigterm");
    let status = child.wait().expect("wait");
    wait_for_file(&record, 4_000);
    assert_eq!(fs::read_to_string(&record).expect("sig").trim(), "SIGTERM");
    let code = status
        .code()
        .or_else(|| status.signal().map(|sig| 128 + sig));
    assert_eq!(
        code,
        Some(143),
        "qc should surface SIGTERM as 143 after exec"
    );
}

#[test]
fn postinstall_refresh_then_harness_and_soft_fail_when_missing() {
    let scratch = unique_scratch("postinstall");
    let pkg = scratch.root.join("pkg");
    stage_installable_layout(&pkg, "0.0.0");
    let script = pkg.join("scripts/postinstall.mjs");
    let output = run_node(
        &[script.to_str().unwrap()],
        &scratch.home(),
        &[("PATH", "/usr/bin:/bin")],
    );
    // PATH without the native dir is fine; postinstall uses an absolute selected path.
    assert_eq!(output.status.code(), Some(0), "stderr={}", stderr(&output));
    assert!(scratch.home().join(".qc/config.toml").is_file());
    assert!(
        scratch
            .home()
            .join(".qc/.default-settings/config.toml")
            .is_file()
    );

    fs::write(scratch.home().join(".qc/config.toml"), "custom = true\n").expect("customize");
    crate::common::write_file(&scratch.home().join(".agents/skills/qc/SKILL.md"), "stale");
    let again = run_node(&[script.to_str().unwrap()], &scratch.home(), &[]);
    assert_eq!(again.status.code(), Some(0), "stderr={}", stderr(&again));
    assert_eq!(
        fs::read_to_string(scratch.home().join(".qc/config.toml")).expect("kept"),
        "custom = true\n"
    );
    let skill =
        fs::read_to_string(scratch.home().join(".agents/skills/qc/SKILL.md")).expect("skill");
    assert!(skill.contains("name: qc"));

    let empty = unique_scratch("postinstall-missing");
    let missing_pkg = empty.root.join("pkg");
    fs::create_dir_all(missing_pkg.join("scripts")).expect("scripts");
    fs::copy(
        crate_root().join("scripts/native.mjs"),
        missing_pkg.join("scripts/native.mjs"),
    )
    .expect("native");
    fs::copy(
        crate_root().join("scripts/postinstall.mjs"),
        missing_pkg.join("scripts/postinstall.mjs"),
    )
    .expect("postinstall");
    chmod_755(&missing_pkg.join("scripts/postinstall.mjs"));
    let missing = run_node(
        &[missing_pkg
            .join("scripts/postinstall.mjs")
            .to_str()
            .unwrap()],
        &empty.home(),
        &[],
    );
    assert_eq!(missing.status.code(), Some(0));
    let warn = stderr(&missing);
    assert!(warn.contains("postinstall"), "{warn}");
    assert!(!empty.home().join(".qc").exists());
}

#[test]
fn launcher_mode_bits_stay_executable() {
    let scratch = unique_scratch("launch-mode");
    let pkg = scratch.root.join("pkg");
    let launcher = stage_installable_layout(&pkg, "0.0.0");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(&launcher).expect("meta").permissions().mode() & 0o777;
        assert_eq!(mode, 0o755);
        let native = pkg.join("dist").join(host_triple()).join("qc");
        let native_mode = fs::metadata(&native).expect("native").permissions().mode() & 0o777;
        assert_eq!(native_mode, 0o755);
    }
}

#[test]
fn missing_native_pair_fails_before_home_mutation() {
    let scratch = unique_scratch("launch-missing");
    let pkg = scratch.root.join("pkg");
    let launcher = stage_installable_layout(&pkg, "0.0.0");
    fs::remove_file(pkg.join("dist").join(host_triple()).join("qc")).expect("rm qc");
    let output = run_launcher(&launcher, &scratch, &scratch.cwd(), &["--version"], &[]);
    assert_ne!(output.status.code(), Some(0));
    assert!(stderr(&output).contains("qc:"));
    assert!(!scratch.home().join(".qc").exists());
}
