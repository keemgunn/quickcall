use std::os::unix::fs::symlink;
use std::path::Path;

use crate::common::{unique_scratch, write_file};
use quickcall::prompt::{append_prompt, parse_prompt, resolve_prompt};

#[test]
fn removes_canonical_frontmatter_and_expands_a_skill_home() {
    let parsed = parse_prompt(
        "---\nqc_skill_path: ~/skills\nqc_tool: pi\n---\nhello",
        "p.md",
        "/home/test",
    )
    .expect("parse");
    assert_eq!(parsed.body, "hello");
    assert_eq!(parsed.meta.skill_path.as_deref(), Some("/home/test/skills"));
    assert_eq!(parsed.meta.tool.as_deref(), Some("pi"));
}

#[test]
fn formats_the_appended_message_exactly() {
    assert_eq!(
        append_prompt("body", Some("more"), false),
        "body\n\n---\n\nAdditional Message from the user:\n\nmore"
    );
}

#[test]
fn uses_raw_append_text_when_no_prompt_file() {
    assert_eq!(
        append_prompt("", Some("also add tests"), true),
        "also add tests"
    );
}

#[test]
fn rejects_removed_qc_cli_and_qc_approve_with_rename_errors() {
    let cli = parse_prompt("---\nqc_cli: pi\n---\nbody", "p.md", "/home").expect_err("qc_cli");
    assert!(cli.message.contains("qc_tool"), "{}", cli.message);
    assert_eq!(
        cli.message,
        "frontmatter key 'qc_cli' was removed; use qc_tool"
    );
    let approve =
        parse_prompt("---\nqc_approve: true\n---\nbody", "p.md", "/home").expect_err("qc_approve");
    assert!(
        approve.message.contains("qc_approve"),
        "{}",
        approve.message
    );
    assert_eq!(
        approve.message,
        "frontmatter key 'qc_approve' was removed; use nothing (agents always force-allow)"
    );
}

#[test]
fn rejects_legacy_frontmatter_keys_with_qc_guidance() {
    for source in [
        "---\npqi_model: x\n---\nbody",
        "---\nqpi_model: x\n---\nbody",
        "---\npi_model: x\n---\nbody",
    ] {
        let err = parse_prompt(source, "p.md", "/home").expect_err(source);
        assert!(err.message.contains("qc_*"), "{}: {}", source, err.message);
    }
}

#[test]
fn requires_a_yaml_mapping() {
    for source in [
        "---\n- item\n---\nbody",
        "---\nvalue\n---\nbody",
        "---\ntrue\n---\nbody",
    ] {
        let err = parse_prompt(source, "p.md", "/home").expect_err(source);
        assert!(
            err.message.contains("mapping"),
            "{}: {}",
            source,
            err.message
        );
    }
}

#[test]
fn validates_description_as_a_string() {
    let err = parse_prompt("---\ndescription: 4\n---\nbody", "p.md", "/home").expect_err("n");
    assert!(err.message.contains("description"), "{}", err.message);
}

#[test]
fn strips_a_folded_comment_key_with_the_rest_of_the_frontmatter() {
    let source = [
        "---",
        "description: documented",
        "comment: >",
        "  Example of driving an external CLI from a qc prompt.",
        "---",
        "body",
    ]
    .join("\n");
    let parsed = parse_prompt(&source, "p.md", "/home").expect("parse");
    assert_eq!(parsed.body, "body");
    assert_eq!(parsed.meta, quickcall::prompt::PromptMeta::default());
}

#[test]
fn validates_every_canonical_qc_field_and_leaves_unknown_non_qc_metadata_alone() {
    let source = "---\ndescription: documented\nowner: team\ncomment: human note\nqc_tool: cursor\nqc_model: m\nqc_thinking: high\nqc_workdir: /w\nqc_no_skills: true\nqc_skill_path: relative\n---\nbody";
    let parsed = parse_prompt(source, "p.md", "/home").expect("parse");
    assert_eq!(parsed.body, "body");
    assert_eq!(parsed.meta.tool.as_deref(), Some("cursor"));
    assert_eq!(parsed.meta.model.as_deref(), Some("m"));
    assert_eq!(parsed.meta.thinking.as_deref(), Some("high"));
    assert_eq!(parsed.meta.workdir.as_deref(), Some("/w"));
    assert_eq!(parsed.meta.no_skills, Some(true));
    assert_eq!(parsed.meta.skill_path.as_deref(), Some("relative"));
    assert!(!parsed.body.contains("human note"));

    for field in [
        "qc_model: 1",
        "qc_thinking: ''",
        "qc_no_skills: yes",
        "qc_skill_path: ''",
        "qc_workdir: ''",
        "qc_unknown: x",
        "pi_model: x",
    ] {
        parse_prompt(&format!("---\n{field}\n---\nbody"), "p.md", "/home").expect_err(field);
    }
    assert_eq!(append_prompt("body", None, false), "body");
    assert_eq!(
        append_prompt("body", Some(""), false),
        "body\n\n---\n\nAdditional Message from the user:\n\n"
    );
}

#[test]
fn classifies_direct_paths_and_resolves_project_global_aliases_nested_aliases_and_safe_symlinks() {
    let scratch = unique_scratch("prompt-paths");
    let cwd = scratch.cwd();
    let home = scratch.home();
    let project = cwd.join(".qc").join("prompts");
    let global = home.join(".qc").join("prompts");
    std::fs::create_dir_all(project.join("nested")).expect("project nested");
    std::fs::create_dir_all(global.join("nested")).expect("global nested");
    write_file(&project.join("same.md"), "project");
    write_file(&global.join("same.md"), "global");
    write_file(&global.join("nested").join("global.md"), "nested");
    write_file(&cwd.join("direct.md"), "direct");
    write_file(&project.join("inside.md"), "inside");
    symlink("inside.md", project.join("link.md")).expect("link");

    assert_eq!(
        resolve_prompt("same", &cwd, &home).expect("same"),
        project.join("same.md")
    );
    assert_eq!(
        resolve_prompt("nested/global", &cwd, &home).expect("nested"),
        global.join("nested").join("global.md")
    );
    assert_eq!(
        resolve_prompt("direct.md", &cwd, &home).expect("direct"),
        cwd.join("direct.md")
    );
    assert_eq!(
        resolve_prompt("link", &cwd, &home).expect("link"),
        project.join("link.md")
    );
    let traversal = resolve_prompt("nested/../escape", &cwd, &home).expect_err("traversal");
    assert!(
        traversal.message.contains("must stay"),
        "{}",
        traversal.message
    );
    let missing = resolve_prompt("none", &cwd, &home).expect_err("none");
    assert!(
        missing
            .message
            .contains(&project.join("none.md").to_string_lossy().into_owned()),
        "{}",
        missing.message
    );
}

#[test]
fn rejects_an_alias_symlink_that_escapes_its_selected_prompt_root() {
    let scratch = unique_scratch("prompt-escape");
    let cwd = scratch.cwd();
    let home = scratch.home();
    let prompts = cwd.join(".qc").join("prompts");
    std::fs::create_dir_all(&prompts).expect("prompts");
    let outside = scratch.root.join("outside.md");
    write_file(&outside, "outside");
    symlink(&outside, prompts.join("escape.md")).expect("escape link");
    let err = resolve_prompt("escape", &cwd, &home).expect_err("escape");
    assert!(err.message.contains("outside"), "{}", err.message);
}

#[test]
fn malformed_opening_fence_without_close_is_invalid_frontmatter() {
    let err = parse_prompt("---\nqc_tool: pi\nbody", "p.md", "/home").expect_err("open");
    assert_eq!(err.message, "invalid YAML frontmatter in p.md");
}

#[test]
fn crlf_fences_strip_like_lf() {
    let parsed = parse_prompt("---\r\nqc_tool: pi\r\n---\r\nhello", "p.md", "/home").expect("crlf");
    assert_eq!(parsed.body, "hello");
    assert_eq!(parsed.meta.tool.as_deref(), Some("pi"));
}

#[test]
fn read_prompt_loads_a_real_alias_file() {
    let scratch = unique_scratch("prompt-read");
    let cwd = scratch.cwd();
    let home = scratch.home();
    let path = cwd.join(".qc").join("prompts").join("joke.md");
    write_file(&path, "---\nqc_tool: pi\n---\nhello body");
    let prompt = quickcall::prompt::read_prompt("joke", &cwd, &home).expect("read");
    assert_eq!(prompt.path, path);
    assert_eq!(prompt.body, "hello body");
    assert_eq!(prompt.meta.tool.as_deref(), Some("pi"));
}

#[test]
fn resolves_absolute_direct_path() {
    let scratch = unique_scratch("prompt-abs");
    let file = scratch.cwd().join("abs.md");
    write_file(&file, "abs");
    assert_eq!(
        resolve_prompt(
            file.to_str().expect("utf8"),
            Path::new("/unused"),
            Path::new("/home")
        )
        .expect("abs"),
        file
    );
}
