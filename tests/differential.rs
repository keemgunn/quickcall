//! Opt-in TypeScript oracle vs native `qc`. Not part of `all`.
//!
//! Invoke via `scripts/verification/test-differential.sh --legacy-root <path>`
//! or `codebase-cli/scripts/test.sh differential --legacy-root <path>`.

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use common::{
    Scratch, crate_root, host_triple, install_fixture_names, scratch_root, unique_scratch,
    write_file,
};

fn legacy_root() -> PathBuf {
    let from_env = std::env::var("QC_LEGACY_ROOT").unwrap_or_default();
    if from_env.is_empty() {
        panic!(
            "differential tests require QC_LEGACY_ROOT (pass --legacy-root to scripts/test.sh differential or scripts/verification/test-differential.sh)"
        );
    }
    let path = PathBuf::from(&from_env);
    let cli = path.join("dist/cli.js");
    if !cli.is_file() {
        panic!(
            "legacy oracle missing at {} (need dist/cli.js). Default proof path is .tmp/rust-rewrite/baseline/",
            cli.display()
        );
    }
    path
}

fn node_bin() -> PathBuf {
    let path = std::env::var("PATH").unwrap_or_default();
    for part in path.split(':') {
        let candidate = Path::new(part).join("node");
        if candidate.is_file() {
            return candidate;
        }
    }
    panic!("node is required on PATH to run the TypeScript oracle");
}

struct Pair {
    legacy: Scratch,
    native: Scratch,
    qc: PathBuf,
    node: PathBuf,
    cli: PathBuf,
}

fn pair(label: &str) -> Pair {
    let root = legacy_root();
    let node = node_bin();
    let legacy = unique_scratch(&format!("diff-legacy-{label}"));
    let native = unique_scratch(&format!("diff-native-{label}"));
    install_fixture_names(&legacy.bin());
    install_fixture_names(&native.bin());
    fs::create_dir_all(legacy.cwd().join(".qc/prompts")).expect("legacy prompts");
    fs::create_dir_all(native.cwd().join(".qc/prompts")).expect("native prompts");
    let qc_src = PathBuf::from(env!("CARGO_BIN_EXE_qc"));
    let dist = native.root.join("dist").join(host_triple());
    fs::create_dir_all(&dist).expect("dist");
    fs::copy(&qc_src, dist.join("qc")).expect("copy qc");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let dest = dist.join("qc");
        let mut perms = fs::metadata(&dest).expect("qc meta").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&dest, perms).expect("chmod qc");
    }
    common::copy_dir_all(&crate_root().join("share"), &native.root.join("share"));
    fs::write(
        native.root.join("package.json"),
        r#"{ "name": "@keemgunn/quickcall", "version": "0.0.0" }"#,
    )
    .expect("manifest");
    Pair {
        legacy,
        native,
        qc: dist.join("qc"),
        node,
        cli: root.join("dist/cli.js"),
    }
}

fn extra_path(scratch: &Scratch, _node: &Path) -> String {
    format!("{}:/usr/bin:/bin", scratch.bin().display())
}

fn run_legacy(pair: &Pair, argv: &[&str], extra: &[(&str, &str)]) -> (i32, String, String) {
    let mut cmd = Command::new(&pair.node);
    cmd.arg(&pair.cli)
        .args(argv)
        .current_dir(pair.legacy.cwd())
        .env_clear()
        .env("HOME", pair.legacy.home())
        .env("PATH", extra_path(&pair.legacy, &pair.node))
        .env("SHELL", "/bin/sh")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, value) in extra {
        cmd.env(key, value);
    }
    let output = cmd.output().expect("legacy qc");
    (
        output.status.code().unwrap_or(1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

fn run_native(pair: &Pair, argv: &[&str], extra: &[(&str, &str)]) -> (i32, String, String) {
    let mut cmd = Command::new(&pair.qc);
    cmd.args(argv)
        .current_dir(pair.native.cwd())
        .env_clear()
        .env("HOME", pair.native.home())
        .env("PATH", extra_path(&pair.native, &pair.node))
        .env("SHELL", "/bin/sh")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, value) in extra {
        cmd.env(key, value);
    }
    let output = cmd.output().expect("native qc");
    (
        output.status.code().unwrap_or(1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

fn looks_like_pretty_id(text: &str) -> bool {
    let Some((stamp, rest)) = text.split_once("--") else {
        return false;
    };
    if stamp.len() != 11 || stamp.as_bytes()[6] != b'-' {
        return false;
    }
    let stamp_ok = stamp.bytes().enumerate().all(|(i, b)| {
        if i == 6 {
            b == b'-'
        } else {
            b.is_ascii_digit()
        }
    });
    let mut parts = rest.splitn(2, "--");
    let tool = parts.next().unwrap_or("");
    let suffix = parts.next().unwrap_or("");
    stamp_ok
        && !tool.is_empty()
        && tool.bytes().all(|b| b.is_ascii_lowercase())
        && suffix.len() == 6
        && suffix
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
}

fn looks_like_iso(text: &str) -> bool {
    text.len() >= 20 && text.ends_with('Z') && text.contains('T')
}

fn normalize_slot(text: &str, legacy_root: &Path, native_root: &Path) -> String {
    let mut out = text.to_string();
    for root in [legacy_root, native_root] {
        let display = root.to_string_lossy();
        out = out.replace(display.as_ref(), "$ROOT");
    }
    out = out.replace(&scratch_root().to_string_lossy().into_owned(), "$TMP");
    let mut rebuilt = String::new();
    for token in out.split_inclusive(|ch: char| {
        ch.is_whitespace() || matches!(ch, '"' | ',' | ']' | '[' | '{' | '}' | ':' | '\\')
    }) {
        let trimmed = token.trim_matches(|ch: char| {
            matches!(ch, '"' | ',' | ']' | '[' | '{' | '}' | ':' | '\\' | '\n')
                || ch.is_whitespace()
        });
        if looks_like_pretty_id(trimmed) {
            rebuilt.push_str(&token.replace(trimmed, "$SESSION"));
        } else if looks_like_iso(trimmed) {
            rebuilt.push_str(&token.replace(trimmed, "$TIME"));
        } else {
            rebuilt.push_str(token);
        }
    }
    regex_duration(&rebuilt)
}

fn regex_duration(text: &str) -> String {
    let mut out = String::new();
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b' ' && i + 1 < bytes.len() && bytes[i + 1].is_ascii_digit() {
            let mut j = i + 1;
            while j < bytes.len() && bytes[j].is_ascii_digit() {
                j += 1;
            }
            if j < bytes.len() && bytes[j] == b's' {
                let prev = &text[..i];
                if prev.ends_with('⋅') || prev.ends_with(" ⋅") {
                    out.push_str(" $DURATIONs");
                    i = j + 1;
                    continue;
                }
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    let marker = "\"duration_s\": ";
    let mut replaced = String::new();
    let mut rest = out.as_str();
    while let Some(idx) = rest.find(marker) {
        replaced.push_str(&rest[..idx]);
        replaced.push_str(marker);
        replaced.push_str("$DURATION");
        rest = &rest[idx + marker.len()..];
        while rest.starts_with('-') || rest.chars().next().is_some_and(|ch| ch.is_ascii_digit()) {
            rest = &rest[1..];
        }
    }
    replaced.push_str(rest);
    replaced
}

fn compare_streams(
    label: &str,
    legacy: (i32, String, String),
    native: (i32, String, String),
    legacy_root: &Path,
    native_root: &Path,
) {
    let n_out = normalize_slot(&legacy.1, legacy_root, native_root);
    let r_out = normalize_slot(&native.1, legacy_root, native_root);
    let n_err = normalize_slot(&legacy.2, legacy_root, native_root);
    let r_err = normalize_slot(&native.2, legacy_root, native_root);
    assert_eq!(legacy.0, native.0, "{label} exit");
    assert_eq!(
        n_out, r_out,
        "{label} stdout\nlegacy={n_out:?}\nnative={r_out:?}"
    );
    assert_eq!(
        n_err, r_err,
        "{label} stderr\nlegacy={n_err:?}\nnative={r_err:?}"
    );
}

fn session_names(home: &Path) -> Vec<String> {
    let dir = home.join(".qc/sessions");
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".json"))
        .collect();
    names.sort();
    names
}

#[test]
fn help_version_and_parse_fail() {
    let pair = pair("info");
    compare_streams(
        "parse-fail",
        run_legacy(&pair, &["--wat"], &[]),
        run_native(&pair, &["--wat"], &[]),
        &pair.legacy.root,
        &pair.native.root,
    );
    assert!(!pair.legacy.home().join(".qc").exists());
    assert!(!pair.native.home().join(".qc").exists());
    compare_streams(
        "help",
        run_legacy(&pair, &["--help"], &[]),
        run_native(&pair, &["--help"], &[]),
        &pair.legacy.root,
        &pair.native.root,
    );
    compare_streams(
        "version",
        run_legacy(&pair, &["--version"], &[]),
        run_native(&pair, &["--version"], &[]),
        &pair.legacy.root,
        &pair.native.root,
    );
}

#[test]
fn append_prompt_quiet_json_fail_and_missing_binary() {
    let pair = pair("turns");
    write_file(&pair.legacy.cwd().join(".qc/prompts/plain.md"), "Plain");
    write_file(&pair.native.cwd().join(".qc/prompts/plain.md"), "Plain");
    write_file(&pair.legacy.home().join(".qc/config.toml"), "");
    write_file(&pair.native.home().join(".qc/config.toml"), "");

    compare_streams(
        "append-only",
        run_legacy(&pair, &["--append", "inspect"], &[("QC_AGENT_TEXT", "ok")]),
        run_native(&pair, &["--append", "inspect"], &[("QC_AGENT_TEXT", "ok")]),
        &pair.legacy.root,
        &pair.native.root,
    );
    compare_streams(
        "prompt",
        run_legacy(&pair, &["plain"], &[("QC_AGENT_TEXT", "joke")]),
        run_native(&pair, &["plain"], &[("QC_AGENT_TEXT", "joke")]),
        &pair.legacy.root,
        &pair.native.root,
    );
    compare_streams(
        "quiet",
        run_legacy(&pair, &["-q", "plain"], &[("QC_AGENT_TEXT", "joke")]),
        run_native(&pair, &["-q", "plain"], &[("QC_AGENT_TEXT", "joke")]),
        &pair.legacy.root,
        &pair.native.root,
    );
    compare_streams(
        "json",
        run_legacy(
            &pair,
            &["plain", "--output", "json"],
            &[("QC_AGENT_TEXT", "joke")],
        ),
        run_native(
            &pair,
            &["plain", "--output", "json"],
            &[("QC_AGENT_TEXT", "joke")],
        ),
        &pair.legacy.root,
        &pair.native.root,
    );
    compare_streams(
        "fail-empty",
        run_legacy(&pair, &["plain"], &[("QC_AGENT_TEXT", "")]),
        run_native(&pair, &["plain"], &[("QC_AGENT_TEXT", "")]),
        &pair.legacy.root,
        &pair.native.root,
    );
    assert_eq!(
        session_names(&pair.legacy.home()).len(),
        session_names(&pair.native.home()).len()
    );

    let missing_legacy = unique_scratch("diff-miss-l");
    let missing_native = unique_scratch("diff-miss-n");
    fs::create_dir_all(missing_legacy.cwd().join(".qc/prompts")).expect("p");
    fs::create_dir_all(missing_native.cwd().join(".qc/prompts")).expect("p");
    write_file(&missing_legacy.cwd().join(".qc/prompts/plain.md"), "Plain");
    write_file(&missing_native.cwd().join(".qc/prompts/plain.md"), "Plain");
    let l = Command::new(&pair.node)
        .arg(&pair.cli)
        .args(["plain"])
        .current_dir(missing_legacy.cwd())
        .env_clear()
        .env("HOME", missing_legacy.home())
        .env("PATH", "/usr/bin:/bin")
        .env("SHELL", "/bin/sh")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("legacy missing");
    let n = Command::new(&pair.qc)
        .args(["plain"])
        .current_dir(missing_native.cwd())
        .env_clear()
        .env("HOME", missing_native.home())
        .env("PATH", "/usr/bin:/bin")
        .env("SHELL", "/bin/sh")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("native missing");
    assert_eq!(l.status.code(), n.status.code());
    let lerr = String::from_utf8_lossy(&l.stderr);
    let nerr = String::from_utf8_lossy(&n.stderr);
    assert!(lerr.contains("pi executable 'pi' was not found"));
    assert!(nerr.contains("pi executable 'pi' was not found"));
    assert!(!missing_legacy.home().join(".qc/sessions").exists());
    assert!(!missing_native.home().join(".qc/sessions").exists());
}

#[test]
fn valid_text_nonzero_exit_matches_oracle() {
    let pair = pair("nonzero");
    write_file(&pair.legacy.cwd().join(".qc/prompts/plain.md"), "Plain");
    write_file(&pair.native.cwd().join(".qc/prompts/plain.md"), "Plain");
    write_file(&pair.legacy.home().join(".qc/config.toml"), "");
    write_file(&pair.native.home().join(".qc/config.toml"), "");
    compare_streams(
        "text-plus-7",
        run_legacy(
            &pair,
            &["plain"],
            &[("QC_AGENT_TEXT", "kept-text"), ("QC_PI_EXIT", "7")],
        ),
        run_native(
            &pair,
            &["plain"],
            &[("QC_AGENT_TEXT", "kept-text"), ("QC_PI_EXIT", "7")],
        ),
        &pair.legacy.root,
        &pair.native.root,
    );
}

#[test]
fn open_bare_and_by_id() {
    let pair = pair("open");
    write_file(&pair.legacy.home().join(".qc/config.toml"), "");
    write_file(&pair.native.home().join(".qc/config.toml"), "");
    let id = "260901-1000--pi--bbbbbb";
    for side in [&pair.legacy, &pair.native] {
        let cwd = side.cwd().to_string_lossy().into_owned();
        write_file(
            &side.home().join(".qc/sessions").join(format!("{id}.json")),
            &format!(
                "{}\n",
                serde_json::json!({
                    "tool": "pi",
                    "native_id": "new-native",
                    "cwd": cwd,
                    "created": "2026-09-01T00:00:00.000Z",
                    "updated": "2026-09-01T02:00:00.000Z",
                    "warnings": []
                })
            ),
        );
    }
    let l_record = pair.legacy.root.join("record.json");
    let n_record = pair.native.root.join("record.json");
    let l = run_legacy(
        &pair,
        &["-o", id],
        &[("QC_RECORD", l_record.to_str().unwrap())],
    );
    let n = run_native(
        &pair,
        &["-o", id],
        &[("QC_RECORD", n_record.to_str().unwrap())],
    );
    assert_eq!(l.0, n.0);
    let lrec: serde_json::Value =
        serde_json::from_slice(&fs::read(&l_record).expect("lrec")).expect("json");
    let nrec: serde_json::Value =
        serde_json::from_slice(&fs::read(&n_record).expect("nrec")).expect("json");
    assert_eq!(lrec["argv"], nrec["argv"]);
}
