use std::collections::HashMap;
use std::fs;

use quickcall::app::run;
use quickcall::package::resolve_package;

use crate::common::{copy_dir_all, crate_root, unique_scratch};

fn chmod_755(path: &std::path::Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(path).expect("meta").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms).expect("chmod");
    }
}

fn dummy_installed(scratch: &crate::common::Scratch, version: &str) -> std::path::PathBuf {
    let dist = scratch.root.join("dist").join("test-triple");
    fs::create_dir_all(&dist).expect("dist");
    let exe = dist.join("qc");
    fs::write(&exe, b"dummy").expect("dummy qc");
    chmod_755(&exe);
    copy_dir_all(&crate_root().join("share"), &scratch.root.join("share"));
    fs::write(
        scratch.root.join("package.json"),
        format!(r#"{{ "version": "{version}" }}"#),
    )
    .expect("manifest");
    exe
}

fn assert_repaired_home(home: &std::path::Path) {
    let expected_config = fs::read_to_string(crate_root().join("share/settings/config.toml"))
        .expect("packaged config");
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
fn resolve_package_reads_adjacent_manifest_and_starter() {
    let scratch = unique_scratch("pkg-resolve");
    let dist = scratch.root.join("dist").join("test-triple");
    fs::create_dir_all(&dist).expect("dist");
    let exe = dist.join("qc");
    fs::write(&exe, b"dummy").expect("dummy qc");
    chmod_755(&exe);
    fs::create_dir_all(scratch.root.join("share/settings")).expect("share");
    fs::copy(
        crate_root().join("share/settings/config.toml"),
        scratch.root.join("share/settings/config.toml"),
    )
    .expect("starter");
    fs::write(
        scratch.root.join("package.json"),
        r#"{ "name": "@keemgunn/quickcall", "version": "9.9.9-test" }"#,
    )
    .expect("manifest");

    let layout = resolve_package(&exe).expect("resolve");
    assert_eq!(layout.version, "9.9.9-test");
    assert_eq!(
        layout.starter_config,
        scratch.root.join("share/settings/config.toml")
    );
    let starter = fs::read_to_string(&layout.starter_config).expect("read starter");
    assert!(starter.contains("[tool.pi]"));
}

#[test]
fn resolve_package_rejects_raw_target_layout() {
    let scratch = unique_scratch("pkg-target");
    let debug = scratch.root.join("target").join("debug");
    fs::create_dir_all(&debug).expect("target");
    let exe = debug.join("qc");
    fs::write(&exe, b"dummy").expect("dummy");
    chmod_755(&exe);
    let err = resolve_package(&exe).expect_err("raw target is not installed layout");
    assert!(err.message.contains("dist/<triple>/"));
}

#[test]
fn help_and_version_repair_owned_files_without_loading_project_config() {
    let scratch = unique_scratch("pkg-app");
    let exe = dummy_installed(&scratch, "9.9.9-test");
    fs::create_dir_all(scratch.cwd().join(".qc")).expect("project qc");
    fs::write(scratch.cwd().join(".qc/config.toml"), "not = [").expect("bad project config");

    let mut env = HashMap::new();
    env.insert("HOME".into(), scratch.home().to_string_lossy().into_owned());

    let help = run(&["--help".to_string()], &scratch.cwd(), &env, &exe).expect("help");
    assert_eq!(help.exit, 0);
    assert!(help.stdout.starts_with("Usage: qc"));
    assert_eq!(
        fs::read_to_string(scratch.cwd().join(".qc/config.toml")).expect("project"),
        "not = ["
    );
    assert_repaired_home(&scratch.home());

    let version = run(&["--version".to_string()], &scratch.cwd(), &env, &exe).expect("version");
    assert_eq!(version.stdout, "9.9.9-test\n");
    assert_repaired_home(&scratch.home());
}

#[test]
fn standalone_install_sample_prompts_bypasses_ordinary_repair() {
    let scratch = unique_scratch("pkg-install-samples");
    let exe = dummy_installed(&scratch, "0.0.0");
    let mut env = HashMap::new();
    env.insert("HOME".into(), scratch.home().to_string_lossy().into_owned());
    let out = run(
        &["--install-sample-prompts".to_string()],
        &scratch.cwd(),
        &env,
        &exe,
    )
    .expect("install samples");
    assert_eq!(out.exit, 0);
    assert!(out.stdout.is_empty());
    assert!(scratch.home().join(".qc/prompts/samples/joke.md").is_file());
    assert!(!scratch.home().join(".qc/config.toml").exists());
    assert!(!scratch.home().join(".qc/.gitignore").exists());
}

#[test]
fn standalone_install_agent_harness_bypasses_ordinary_repair() {
    let scratch = unique_scratch("pkg-install-harness");
    let exe = dummy_installed(&scratch, "0.0.0");
    let mut env = HashMap::new();
    env.insert("HOME".into(), scratch.home().to_string_lossy().into_owned());
    let out = run(
        &["--install-agent-harness".to_string()],
        &scratch.cwd(),
        &env,
        &exe,
    )
    .expect("install harness");
    assert_eq!(out.exit, 0);
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
fn turns_repair_then_fail_missing_binary_without_a_session_file() {
    let scratch = unique_scratch("pkg-turn");
    let exe = dummy_installed(&scratch, "0.0.0");
    let mut env = HashMap::new();
    env.insert("HOME".into(), scratch.home().to_string_lossy().into_owned());
    env.insert("PATH".into(), scratch.bin().display().to_string());
    env.insert("SHELL".into(), "/bin/sh".into());
    let out = run(
        &["--append".to_string(), "inspect".to_string()],
        &scratch.cwd(),
        &env,
        &exe,
    )
    .expect("missing binary is a fail envelope");
    assert_eq!(out.exit, 1);
    assert!(out.stdout.is_empty());
    assert!(
        out.stderr.contains("pi executable 'pi' was not found"),
        "{}",
        out.stderr
    );
    assert_repaired_home(&scratch.home());
    assert!(!scratch.home().join(".qc/sessions").exists());
}

#[test]
fn invalid_args_do_not_require_package_or_mutate_home() {
    let scratch = unique_scratch("pkg-invalid");
    let mut env = HashMap::new();
    env.insert("HOME".into(), scratch.home().to_string_lossy().into_owned());
    let err = run(
        &["--wat".to_string()],
        &scratch.cwd(),
        &env,
        &scratch.root.join("missing-qc"),
    )
    .expect_err("unknown option");
    assert_eq!(err.message, "unknown option '--wat'");
    assert!(!scratch.home().join(".qc").exists());
}

#[test]
fn missing_home_fails_after_successful_parse() {
    let scratch = unique_scratch("pkg-nohome");
    let dist = scratch.root.join("dist").join("test-triple");
    fs::create_dir_all(&dist).expect("dist");
    let exe = dist.join("qc");
    fs::write(&exe, b"dummy").expect("dummy");
    chmod_755(&exe);
    let err = run(
        &["--version".to_string()],
        &scratch.cwd(),
        &HashMap::new(),
        &exe,
    )
    .expect_err("HOME");
    assert_eq!(err.message, "HOME is required to locate qc configuration");
}
