use std::fs;
use std::process::{Command, Stdio};

use crate::common::{crate_root, install_fixture_names, stage_package, unique_scratch, write_file};

#[test]
fn sample_like_prompt_matches_core_fixture_assertions() {
    let scratch = unique_scratch("smoke-sample");
    install_fixture_names(&scratch.bin());
    let qc = stage_package(&scratch, "0.0.0");
    write_file(
        &scratch.cwd().join(".qc/config.toml"),
        r#"[tool]
default = "pi"
[tool.pi]
default_model = "opencode-go/muse-spark-1.3-contributor"
default_thinking = "high"
"#,
    );
    write_file(
        &scratch.cwd().join(".qc/prompts/context/report.md"),
        r#"---
description: Report the sample project's invocation context.
qc_no_skills: true
---

# Sample Project Context

Use this verified invocation context:

- Working directory: !`pwd`
- Sample marker: !`printf sample-project`
"#,
    );
    let record = scratch.root.join("record.json");
    let output = Command::new(&qc)
        .args([
            "context/report",
            "--append",
            "Runner context: !`printf runner-append`",
        ])
        .current_dir(scratch.cwd())
        .env_clear()
        .env("HOME", scratch.home())
        .env("PATH", format!("{}:/usr/bin:/bin", scratch.bin().display()))
        .env("SHELL", "/bin/sh")
        .env("QC_RECORD", &record)
        .env("QC_AGENT_TEXT", "fixture Pi: success")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("sample qc");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("fixture Pi: success"));
    let recorded: serde_json::Value =
        serde_json::from_slice(&fs::read(&record).expect("record")).expect("json");
    let argv = recorded["argv"].as_array().expect("argv");
    assert_eq!(argv[0], "--mode");
    assert_eq!(argv[1], "json");
    assert_eq!(argv[2], "-a");
    assert!(
        argv.iter()
            .any(|v| v == "opencode-go/muse-spark-1.3-contributor")
    );
    assert!(argv.iter().any(|v| v == "high"));
    assert!(argv.iter().any(|v| v == "--no-skills"));
    let stdin = recorded["stdin"].as_str().expect("stdin");
    assert!(stdin.contains(&scratch.cwd().to_string_lossy().into_owned()));
    assert!(stdin.contains("sample-project"));
    assert!(stdin.contains("runner-append"));
    let expected_config =
        fs::read_to_string(crate_root().join("share/settings/config.toml")).expect("starter");
    assert_eq!(
        fs::read_to_string(scratch.home().join(".qc/config.toml")).expect("global"),
        expected_config
    );
}
