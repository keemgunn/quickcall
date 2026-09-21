use std::fs;

use crate::common::{crate_root, unique_scratch, write_file};
use quickcall::harness::{install_agent_harness, refresh_known, skill_destinations};

fn skill_file(
    home: &std::path::Path,
    dest_index: usize,
    skill: &str,
    relative: &str,
) -> std::path::PathBuf {
    skill_destinations(home)[dest_index]
        .join(skill)
        .join(relative)
}

#[test]
fn replace_installs_packaged_skills_into_both_skill_destinations() {
    let scratch = unique_scratch("harness-install");
    let home = scratch.home();
    let bundled = crate_root().join("share/skills");
    let result = install_agent_harness(&home, &bundled).expect("install");
    assert!(result.copied.iter().any(|path| {
        path == &home
            .join(".agents")
            .join("skills")
            .join("qc")
            .to_string_lossy()
    }));
    assert!(result.copied.iter().any(|path| {
        path == &home
            .join(".claude")
            .join("skills")
            .join("qc-create-prompt")
            .to_string_lossy()
    }));
    let qc_agents = fs::read_to_string(skill_file(&home, 0, "qc", "SKILL.md")).expect("agents qc");
    assert!(qc_agents.contains("name: qc"));
    let qc_claude = fs::read_to_string(skill_file(&home, 1, "qc", "SKILL.md")).expect("claude qc");
    assert!(qc_claude.contains("name: qc"));
    let command = fs::read_to_string(skill_file(&home, 0, "qc-create-prompt", "SKILL.md"))
        .expect("command skill");
    assert!(command.contains("disable-model-invocation: true"));
    let openai = fs::read_to_string(skill_file(&home, 1, "qc-create-prompt", "openai.yaml"))
        .expect("openai yaml");
    assert!(openai.contains("allow_implicit_invocation: false"));
    assert!(
        !home
            .join(".cursor")
            .join("commands")
            .join("qc")
            .join("qc-create-prompt.md")
            .exists()
    );
}

#[test]
fn deletes_retired_qc_dirs_and_leaves_unrelated_skills() {
    let scratch = unique_scratch("harness-replace");
    let home = scratch.home();
    let agents = home.join(".agents").join("skills");
    write_file(&agents.join("qc").join("SKILL.md"), "stale");
    write_file(&agents.join("qc-retired").join("SKILL.md"), "retired");
    write_file(&agents.join("other").join("SKILL.md"), "keep");
    let bundled = scratch.root.join("bundled");
    write_file(&bundled.join("qc").join("SKILL.md"), "fresh qc");
    write_file(
        &bundled.join("qc-create-prompt").join("SKILL.md"),
        "fresh command",
    );
    install_agent_harness(&home, &bundled).expect("install");
    assert_eq!(
        fs::read_to_string(agents.join("qc").join("SKILL.md")).expect("qc"),
        "fresh qc"
    );
    assert_eq!(
        fs::read_to_string(agents.join("qc-create-prompt").join("SKILL.md")).expect("cmd"),
        "fresh command"
    );
    assert_eq!(
        fs::read_to_string(agents.join("other").join("SKILL.md")).expect("other"),
        "keep"
    );
    assert!(!agents.join("qc-retired").join("SKILL.md").exists());
}

#[test]
fn refresh_known_updates_only_dests_that_already_have_product_skills() {
    let scratch = unique_scratch("harness-refresh-known");
    let home = scratch.home();
    let bundled = scratch.root.join("bundled");
    write_file(&bundled.join("qc").join("SKILL.md"), "fresh skill");
    write_file(
        &home
            .join(".agents")
            .join("skills")
            .join("qc")
            .join("SKILL.md"),
        "stale",
    );
    let result = refresh_known(&home, &bundled).expect("refresh");
    assert!(result.known);
    assert_eq!(
        fs::read_to_string(
            home.join(".agents")
                .join("skills")
                .join("qc")
                .join("SKILL.md")
        )
        .expect("agents"),
        "fresh skill"
    );
    assert!(
        !home
            .join(".claude")
            .join("skills")
            .join("qc")
            .join("SKILL.md")
            .exists()
    );
}

#[test]
fn refresh_known_wipe_reinstalls_every_packaged_skill_on_a_dest_that_already_has_qc_or_qc_star() {
    let scratch = unique_scratch("harness-refresh-clean-slate");
    let home = scratch.home();
    let agents = home.join(".agents").join("skills");
    let claude = home.join(".claude").join("skills");
    let bundled = scratch.root.join("bundled");
    write_file(&bundled.join("qc").join("SKILL.md"), "fresh qc");
    write_file(
        &bundled.join("qc-create-prompt").join("SKILL.md"),
        "fresh command",
    );
    write_file(&agents.join("qc").join("SKILL.md"), "stale");
    write_file(&agents.join("qc-retired").join("SKILL.md"), "retired");
    write_file(&agents.join("other").join("SKILL.md"), "keep-agents");
    write_file(
        &agents.join("qcode").join("SKILL.md"),
        "not-a-product-prefix",
    );
    write_file(&claude.join("unrelated").join("SKILL.md"), "keep-claude");
    let result = refresh_known(&home, &bundled).expect("refresh");
    assert!(result.known);
    assert_eq!(
        fs::read_to_string(agents.join("qc").join("SKILL.md")).expect("qc"),
        "fresh qc"
    );
    assert_eq!(
        fs::read_to_string(agents.join("qc-create-prompt").join("SKILL.md")).expect("cmd"),
        "fresh command"
    );
    assert_eq!(
        fs::read_to_string(agents.join("other").join("SKILL.md")).expect("other"),
        "keep-agents"
    );
    assert_eq!(
        fs::read_to_string(agents.join("qcode").join("SKILL.md")).expect("qcode"),
        "not-a-product-prefix"
    );
    assert!(!agents.join("qc-retired").join("SKILL.md").exists());
    assert!(!claude.join("qc").join("SKILL.md").exists());
    assert_eq!(
        fs::read_to_string(claude.join("unrelated").join("SKILL.md")).expect("unrelated"),
        "keep-claude"
    );
}

#[test]
fn refresh_known_is_a_no_op_when_nothing_was_installed_before() {
    let scratch = unique_scratch("harness-refresh-empty");
    let result = refresh_known(scratch.home(), crate_root().join("share/skills")).expect("refresh");
    assert!(result.copied.is_empty());
    assert!(result.skipped.is_empty());
    assert!(!result.known);
}
