mod common;
mod package_support;

use std::fs;
use std::process::{Command, Stdio};

use common::{crate_root, host_triple, stage_package, unique_scratch};
use quickcall::messages::{BOOTSTRAP_USAGE, HELP};

fn run_staged(qc: &std::path::Path, home: &std::path::Path, args: &[&str]) -> std::process::Output {
    Command::new(qc)
        .args(args)
        .env_clear()
        .env("HOME", home)
        .env("PATH", "/usr/bin:/bin")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("run staged qc")
}

fn run_bootstrap(
    bootstrap: &std::path::Path,
    home: &std::path::Path,
    args: &[&str],
) -> std::process::Output {
    Command::new(bootstrap)
        .args(args)
        .env_clear()
        .env("HOME", home)
        .env("PATH", "/usr/bin:/bin")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("run staged qc-bootstrap")
}

fn assert_repaired_home(home: &std::path::Path) {
    let expected_config =
        fs::read_to_string(crate_root().join("share/settings/config.toml")).expect("packaged");
    assert_eq!(
        fs::read_to_string(home.join(".qc/config.toml")).expect("sentinel"),
        expected_config
    );
    assert_eq!(
        fs::read_to_string(home.join(".qc/.gitignore")).expect("gitignore"),
        ".default-settings/\nsessions/\n"
    );
    assert!(home.join(".qc/.default-settings/config.toml").is_file());
    for name in [
        "git-commit-push.md",
        "joke.md",
        "system-status.md",
        "web-surf.md",
    ] {
        let expected = fs::read_to_string(
            crate_root()
                .join("share/settings/prompts/samples")
                .join(name),
        )
        .expect(name);
        assert_eq!(
            fs::read_to_string(home.join(".qc/prompts/samples").join(name)).expect(name),
            expected
        );
    }
}

#[test]
fn staged_version_reads_package_json_not_cargo_version() {
    let scratch = unique_scratch("pkg-version");
    let qc = stage_package(&scratch, "9.9.9-test");
    let output = run_staged(&qc, &scratch.home(), &["--version"]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "9.9.9-test\n");
    assert!(output.stderr.is_empty());
    assert_repaired_home(&scratch.home());
}

#[test]
fn staged_help_repairs_then_matches_typescript_help_text() {
    let scratch = unique_scratch("pkg-help");
    let qc = stage_package(&scratch, "0.0.0");
    let output = run_staged(&qc, &scratch.home(), &["--help"]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), format!("{HELP}\n"));
    assert!(output.stderr.is_empty());
    assert_repaired_home(&scratch.home());
}

#[test]
fn staged_invalid_args_do_not_create_home_qc() {
    let scratch = unique_scratch("pkg-invalid-cli");
    let qc = stage_package(&scratch, "0.0.0");
    let output = run_staged(&qc, &scratch.home(), &["--wat"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        "[ERROR] unknown option '--wat'\n"
    );
    assert!(!scratch.home().join(".qc").exists());
}

#[test]
fn staged_missing_home_errors_without_node() {
    let scratch = unique_scratch("pkg-no-home");
    let qc = stage_package(&scratch, "0.0.0");
    let output = Command::new(&qc)
        .arg("--version")
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("run");
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        "[ERROR] HOME is required to locate qc configuration\n"
    );
    assert!(!scratch.home().join(".qc").exists());
}

#[test]
fn staged_install_sample_prompts_bypasses_repair() {
    let scratch = unique_scratch("pkg-samples");
    let qc = stage_package(&scratch, "0.0.0");
    let output = run_staged(&qc, &scratch.home(), &["--install-sample-prompts"]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    assert!(scratch.home().join(".qc/prompts/samples/joke.md").is_file());
    assert!(!scratch.home().join(".qc/config.toml").exists());
    assert!(!scratch.home().join(".qc/.gitignore").exists());
}

#[test]
fn staged_install_agent_harness_bypasses_repair() {
    let scratch = unique_scratch("pkg-harness");
    let qc = stage_package(&scratch, "0.0.0");
    let output = run_staged(&qc, &scratch.home(), &["--install-agent-harness"]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(scratch.home().join(".agents/skills/qc/SKILL.md").is_file());
    assert!(
        scratch
            .home()
            .join(".claude/skills/qc-create-prompt/SKILL.md")
            .is_file()
    );
    assert!(!scratch.home().join(".qc/config.toml").exists());
}

#[test]
fn staged_qc_bootstrap_settings_repair_and_refresh() {
    let scratch = unique_scratch("pkg-bootstrap-settings");
    let qc = stage_package(&scratch, "0.0.0");
    let bootstrap = qc.parent().expect("triple").join("qc-bootstrap");
    let repair = run_bootstrap(&bootstrap, &scratch.home(), &["settings", "repair"]);
    assert_eq!(
        repair.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&repair.stderr)
    );
    assert_repaired_home(&scratch.home());

    fs::write(scratch.home().join(".qc/.gitignore"), "stale\n").expect("stale gi");
    fs::write(
        scratch.home().join(".qc/.default-settings/stale.txt"),
        "old",
    )
    .expect("stale mirror");
    let refresh = run_bootstrap(&bootstrap, &scratch.home(), &["settings", "refresh"]);
    assert_eq!(
        refresh.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&refresh.stderr)
    );
    assert_eq!(
        fs::read_to_string(scratch.home().join(".qc/.gitignore")).expect("gi"),
        ".default-settings/\nsessions/\n"
    );
    assert!(
        !scratch
            .home()
            .join(".qc/.default-settings/stale.txt")
            .exists()
    );
    assert!(
        scratch
            .home()
            .join(".qc/.default-settings/config.toml")
            .is_file()
    );
}

#[test]
fn staged_qc_bootstrap_harness_refresh_known() {
    let scratch = unique_scratch("pkg-bootstrap-harness");
    let qc = stage_package(&scratch, "0.0.0");
    let bootstrap = qc.parent().expect("triple").join("qc-bootstrap");
    let empty = run_bootstrap(&bootstrap, &scratch.home(), &["harness", "refresh-known"]);
    assert_eq!(
        empty.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&empty.stderr)
    );
    assert!(!scratch.home().join(".agents/skills/qc").exists());

    common::write_file(&scratch.home().join(".agents/skills/qc/SKILL.md"), "stale");
    let known = run_bootstrap(&bootstrap, &scratch.home(), &["harness", "refresh-known"]);
    assert_eq!(
        known.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&known.stderr)
    );
    let skill = fs::read_to_string(scratch.home().join(".agents/skills/qc/SKILL.md")).expect("qc");
    assert!(skill.contains("name: qc"));
    assert!(!scratch.home().join(".claude/skills/qc").exists());
}

#[test]
fn staged_qc_bootstrap_usage_has_no_error_prefix() {
    let scratch = unique_scratch("pkg-bootstrap-usage");
    let qc = stage_package(&scratch, "0.0.0");
    let bootstrap = qc.parent().expect("triple").join("qc-bootstrap");
    let output = run_bootstrap(&bootstrap, &scratch.home(), &[]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        format!("{BOOTSTRAP_USAGE}\n")
    );
    assert!(!String::from_utf8_lossy(&output.stderr).contains("[ERROR]"));
}

#[test]
fn staged_layout_uses_host_triple_and_real_starter() {
    let scratch = unique_scratch("pkg-layout");
    let qc = stage_package(&scratch, "0.0.0");
    assert!(qc.ends_with(format!("dist/{}/qc", host_triple())));
    let starter = scratch.root.join("share/settings/config.toml");
    let source = fs::read_to_string(starter).expect("starter");
    assert!(source.contains("[tool.pi]"));
    assert!(scratch.root.join("share/skills/qc/SKILL.md").is_file());
}
