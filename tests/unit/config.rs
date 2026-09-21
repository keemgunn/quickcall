use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::{MetadataExt, symlink};

use crate::common::{crate_root, unique_scratch, write_file};
use quickcall::config::{
    BootstrapMode, bootstrap_settings, config_paths, effective_shell, hard_refresh_sample_prompts,
    home_from_env, install_sample_prompts, load_config, resolve_bootstrap_home,
};

fn starter_toml(root: &std::path::Path) -> std::path::PathBuf {
    root.join("starter").join("config.toml")
}

fn write_bundled(root: &std::path::Path, config: &str, sample: &str) {
    write_file(&starter_toml(root), config);
    write_file(
        &root
            .join("starter")
            .join("prompts")
            .join("samples")
            .join("joke.md"),
        sample,
    );
}

#[test]
fn seeds_config_and_sample_prompts_only_when_the_sentinel_is_missing() {
    let scratch = unique_scratch("config-seed");
    let home = scratch.home();
    let cwd = scratch.cwd();
    fs::create_dir_all(cwd.join(".qc")).expect("project qc");
    write_bundled(
        &scratch.root,
        "[tool]\ndefault = \"pi\"\n[tool.pi]\ndefault_model = \"starter/model\"\n",
        "sample joke",
    );
    write_file(
        &scratch
            .root
            .join("starter")
            .join("prompts")
            .join("user-owned.md"),
        "not copied",
    );
    let config = config_paths(&home, &cwd, starter_toml(&scratch.root));
    bootstrap_settings(BootstrapMode::Repair, &config).expect("repair");
    let global = fs::read_to_string(&config.global).expect("global");
    assert!(global.contains("default = \"pi\""));
    assert_eq!(
        fs::read_to_string(config.sample_prompts.join("joke.md")).expect("sample"),
        "sample joke"
    );
    assert!(
        !home
            .join(".qc")
            .join("prompts")
            .join("user-owned.md")
            .exists()
    );
    assert_eq!(
        fs::read_to_string(&config.gitignore).expect("gitignore"),
        ".default-settings/\nsessions/\n"
    );
    let loaded = load_config(&config).expect("load");
    assert_eq!(loaded.tool.default.as_deref(), Some("pi"));
    assert_eq!(
        loaded.tool.tools["pi"].default_model.as_deref(),
        Some("starter/model")
    );
    assert_eq!(effective_shell(None, &loaded, &HashMap::new()), "/bin/sh");
    write_file(&config.global, "preserved");
    bootstrap_settings(BootstrapMode::Repair, &config).expect("repair again");
    assert_eq!(
        fs::read_to_string(&config.global).expect("kept"),
        "preserved"
    );
}

#[test]
fn preserves_user_prompts_and_skips_sample_refresh_when_config_exists() {
    let scratch = unique_scratch("config-sentinel");
    write_bundled(&scratch.root, "starter", "fresh sample");
    let config = config_paths(scratch.home(), scratch.cwd(), starter_toml(&scratch.root));
    let user_prompt = scratch.home().join(".qc").join("prompts").join("mine.md");
    let stale_sample = config.sample_prompts.join("joke.md");
    write_file(&config.global, "existing config");
    write_file(&user_prompt, "user prompt");
    write_file(&stale_sample, "stale sample");
    bootstrap_settings(BootstrapMode::Repair, &config).expect("repair");
    assert_eq!(
        fs::read_to_string(&config.global).expect("config"),
        "existing config"
    );
    assert_eq!(
        fs::read_to_string(&user_prompt).expect("user"),
        "user prompt"
    );
    assert_eq!(
        fs::read_to_string(&stale_sample).expect("sample"),
        "stale sample"
    );
}

#[test]
fn refresh_replaces_default_settings_and_gitignore_without_touching_live_config_or_samples() {
    let scratch = unique_scratch("config-refresh");
    write_bundled(&scratch.root, "starter v2", "starter v2 sample");
    let config = config_paths(scratch.home(), scratch.cwd(), starter_toml(&scratch.root));
    fs::create_dir_all(&config.global_root).expect("root");
    write_file(&config.global, "live config");
    write_file(&config.gitignore, "stale\n");
    write_file(&config.default_settings.join("stale.txt"), "old");
    write_file(&config.sample_prompts.join("joke.md"), "live sample");
    bootstrap_settings(BootstrapMode::Refresh, &config).expect("refresh");
    assert_eq!(
        fs::read_to_string(&config.global).expect("live"),
        "live config"
    );
    assert_eq!(
        fs::read_to_string(config.sample_prompts.join("joke.md")).expect("sample"),
        "live sample"
    );
    assert_eq!(
        fs::read_to_string(&config.gitignore).expect("gi"),
        ".default-settings/\nsessions/\n"
    );
    assert_eq!(
        fs::read_to_string(config.default_settings.join("config.toml")).expect("mirror"),
        "starter v2"
    );
}

#[test]
fn install_sample_prompts_hard_refreshes_samples_even_when_config_exists() {
    let scratch = unique_scratch("config-install-samples");
    write_bundled(&scratch.root, "starter", "packaged sample");
    let config = config_paths(scratch.home(), scratch.cwd(), starter_toml(&scratch.root));
    write_file(&config.global, "existing config");
    write_file(&config.sample_prompts.join("joke.md"), "stale sample");
    install_sample_prompts(&config).expect("install");
    assert_eq!(
        fs::read_to_string(&config.global).expect("config"),
        "existing config"
    );
    assert_eq!(
        fs::read_to_string(config.sample_prompts.join("joke.md")).expect("sample"),
        "packaged sample"
    );
}

#[test]
fn install_sample_prompts_does_not_run_ordinary_config_repair() {
    let scratch = unique_scratch("config-install-samples-no-repair");
    write_bundled(&scratch.root, "starter", "packaged sample");
    let config = config_paths(scratch.home(), scratch.cwd(), starter_toml(&scratch.root));
    install_sample_prompts(&config).expect("install");
    assert_eq!(
        fs::read_to_string(config.sample_prompts.join("joke.md")).expect("sample"),
        "packaged sample"
    );
    assert!(!config.global.exists());
    assert!(!config.gitignore.exists());
    assert!(!config.default_settings.join("config.toml").exists());
}

#[test]
fn writes_through_a_qc_symlink_without_replacing_the_link_inode() {
    let scratch = unique_scratch("config-symlink");
    write_bundled(&scratch.root, "starter", "sample");
    let target = scratch.root.join("target");
    fs::create_dir_all(&target).expect("target");
    let link = scratch.home().join(".qc");
    symlink(&target, &link).expect("symlink");
    let before = fs::symlink_metadata(&link).expect("lstat before");
    let config = config_paths(scratch.home(), scratch.cwd(), starter_toml(&scratch.root));
    bootstrap_settings(BootstrapMode::Repair, &config).expect("repair");
    let after = fs::symlink_metadata(&link).expect("lstat after");
    assert!(after.file_type().is_symlink());
    assert_eq!(before.ino(), after.ino());
    assert_eq!(
        fs::read_to_string(target.join("config.toml")).expect("seed"),
        "starter"
    );
    assert_eq!(
        fs::read_to_string(target.join("prompts").join("samples").join("joke.md")).expect("sample"),
        "sample"
    );
}

#[test]
fn hard_refresh_sample_prompts_leaves_sibling_prompt_files_intact() {
    let scratch = unique_scratch("config-sample-refresh");
    write_bundled(&scratch.root, "starter", "fresh");
    let config = config_paths(scratch.home(), scratch.cwd(), starter_toml(&scratch.root));
    let sibling = scratch.home().join(".qc").join("prompts").join("mine.md");
    let stale = config.sample_prompts.join("joke.md");
    write_file(&sibling, "keep me");
    write_file(&stale, "stale");
    hard_refresh_sample_prompts(&config).expect("refresh samples");
    assert_eq!(fs::read_to_string(&sibling).expect("sibling"), "keep me");
    assert_eq!(fs::read_to_string(&stale).expect("sample"), "fresh");
}

#[test]
fn ignores_leftover_command_permissions_and_still_validates_other_toml_shape() {
    let scratch = unique_scratch("config-order");
    let packaged = crate_root().join("share/settings/config.toml");
    let config = config_paths(scratch.home(), scratch.cwd(), &packaged);
    write_file(
        &config.global,
        "[command-permissions]\npwd = \"allow\"\n\"*\" = \"deny\"\n",
    );
    write_file(&config.project, "[command-permissions]\n\"*\" = \"ask\"\n");
    let loaded = load_config(&config).expect("load leftover");
    assert!(loaded.tool.default.is_none());
    assert!(loaded.tool.tools.is_empty());
    write_file(&config.project, "shell = \"\"\n");
    let empty = load_config(&config).expect_err("empty shell");
    assert!(empty.message.contains("non-empty"), "{}", empty.message);
    write_file(&config.project, "unknown = \"x\"\n");
    let unknown = load_config(&config).expect_err("unknown");
    assert!(
        unknown.message.contains("unknown config"),
        "{}",
        unknown.message
    );
    write_file(&config.project, "shell = [\n");
    let invalid = load_config(&config).expect_err("toml");
    assert!(
        invalid.message.contains("invalid TOML"),
        "{}",
        invalid.message
    );
}

#[test]
fn uses_cli_config_environment_then_bin_sh_for_the_shell() {
    let scratch = unique_scratch("config-shell");
    let packaged = crate_root().join("share/settings/config.toml");
    let config = config_paths(scratch.home(), scratch.cwd(), &packaged);
    write_file(&config.global, "shell = \"/bin/sh\"\n");
    write_file(&config.project, "shell = \"/bin/bash\"\n");
    let mut env = HashMap::new();
    env.insert("SHELL".into(), "/bin/false".into());
    assert_eq!(
        effective_shell(Some("/bin/zsh"), &load_config(&config).expect("load"), &env),
        "/bin/zsh"
    );
    assert_eq!(
        effective_shell(None, &load_config(&config).expect("load"), &HashMap::new()),
        "/bin/bash"
    );
    write_file(&config.project, "");
    env.insert("SHELL".into(), "/bin/zsh".into());
    assert_eq!(
        effective_shell(None, &load_config(&config).expect("load"), &env),
        "/bin/sh"
    );
}

#[test]
fn parses_nested_tool_tables_and_rejects_old_rename_keys() {
    let scratch = unique_scratch("config-tool");
    let packaged = crate_root().join("share/settings/config.toml");
    let config = config_paths(scratch.home(), scratch.cwd(), &packaged);
    write_file(
        &config.global,
        "[tool]\ndefault = \"pi\"\n[tool.pi]\ndefault_model = \"g/m\"\ndefault_thinking = \"medium\"\ncommand = \"pi\"\n",
    );
    write_file(
        &config.project,
        "[tool]\ndefault = \"cursor\"\n[tool.cursor]\ndefault_model = \"composer-2.5\"\n[tool.codex]\ndefault_model = \"gpt-5.6-luna\"\ndefault_thinking = \"medium\"\n",
    );
    let loaded = load_config(&config).expect("load tools");
    assert_eq!(loaded.tool.default.as_deref(), Some("cursor"));
    assert_eq!(
        loaded.tool.tools["pi"].default_model.as_deref(),
        Some("g/m")
    );
    assert_eq!(
        loaded.tool.tools["cursor"].default_model.as_deref(),
        Some("composer-2.5")
    );
    assert_eq!(
        loaded.tool.tools["codex"].default_model.as_deref(),
        Some("gpt-5.6-luna")
    );
    assert_eq!(
        loaded.tool.tools["codex"].default_thinking.as_deref(),
        Some("medium")
    );

    for (body, hint) in [
        ("default-cli = \"pi\"\n", "default-cli"),
        ("default-model = \"x\"\n", "default-model"),
        ("default-thinking = \"x\"\n", "default-thinking"),
    ] {
        write_file(&config.global, body);
        let err = load_config(&config).expect_err(hint);
        assert!(err.message.contains(hint), "{}", err.message);
    }
}

#[test]
fn loads_real_fixture_toml_and_accepts_whitespace_only_shell() {
    let scratch = unique_scratch("config-fixtures");
    let config = config_paths(
        scratch.home(),
        scratch.cwd(),
        crate_root().join("share/settings/config.toml"),
    );
    write_file(&config.global, "");
    fs::copy(
        crate_root().join("tests/fixtures/config/global.toml"),
        &config.global,
    )
    .expect("global fixture");
    fs::create_dir_all(config.project.parent().expect("project dir")).expect("project qc");
    fs::copy(
        crate_root().join("tests/fixtures/config/project.toml"),
        &config.project,
    )
    .expect("project fixture");
    let loaded = load_config(&config).expect("fixtures");
    assert_eq!(loaded.shell.as_deref(), Some("/bin/sh"));
    assert_eq!(loaded.tool.default.as_deref(), Some("pi"));
    assert_eq!(
        loaded.tool.tools["pi"].default_model.as_deref(),
        Some("global/model")
    );
    assert_eq!(
        loaded.tool.tools["pi"].default_thinking.as_deref(),
        Some("high")
    );

    write_file(&config.project, "shell = \" \"\n");
    let whitespace = load_config(&config).expect("whitespace shell");
    assert_eq!(whitespace.shell.as_deref(), Some(" "));

    fs::copy(
        crate_root().join("tests/fixtures/config/invalid.toml"),
        &config.global,
    )
    .expect("invalid fixture");
    write_file(&config.project, "");
    let renamed = load_config(&config).expect_err("default-model");
    assert!(
        renamed.message.contains("default-model"),
        "{}",
        renamed.message
    );
}

#[test]
fn bootstrap_home_uses_env_then_injected_fallback() {
    let mut env = HashMap::new();
    assert!(home_from_env(&env).is_none());
    let isolated = unique_scratch("bootstrap-home");
    let resolved = resolve_bootstrap_home(&env, || Some(isolated.home())).expect("fallback");
    assert_eq!(resolved, isolated.home());
    env.insert("HOME".into(), isolated.cwd().to_string_lossy().into_owned());
    let from_env = resolve_bootstrap_home(&env, || panic!("fallback must not run")).expect("env");
    assert_eq!(from_env, isolated.cwd());
}

#[test]
fn public_cli_home_rejects_empty_string() {
    let mut env = HashMap::new();
    env.insert("HOME".into(), String::new());
    assert!(home_from_env(&env).is_none());
}
