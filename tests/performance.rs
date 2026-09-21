//! Release-artifact timings. Not part of `scripts/test.sh all`.
//! Measures staged native `qc` and the npm `bin/qc` launcher, never `cargo run`.

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::Instant;

use common::{Scratch, host_triple, install_fixture_names, unique_scratch, write_file};
use serde_json::{Value, json};

const WARMUP: usize = 3;
const SAMPLES: usize = 20;

struct Entry {
    name: &'static str,
    path: PathBuf,
}

struct Row {
    scenario: String,
    entry: String,
    samples: usize,
    median_ms: f64,
    p95_ms: f64,
    min_ms: f64,
    max_ms: f64,
    notes: String,
}

fn required_path(var: &str) -> Option<PathBuf> {
    std::env::var_os(var).map(PathBuf::from)
}

fn release_native() -> PathBuf {
    if let Some(path) = required_path("QC_BENCH_NATIVE") {
        return path;
    }
    common::crate_root()
        .join("dist")
        .join(host_triple())
        .join("qc")
}

fn run_qc(
    exe: &Path,
    scratch: &Scratch,
    cwd: &Path,
    argv: &[&str],
    extra: &[(&str, &str)],
) -> Output {
    let mut cmd = Command::new(exe);
    let path = match which_node() {
        Some(dir) => format!(
            "{}:{}:/usr/bin:/bin",
            scratch.bin().display(),
            dir.display()
        ),
        None => format!("{}:/usr/bin:/bin", scratch.bin().display()),
    };
    cmd.args(argv)
        .current_dir(cwd)
        .env_clear()
        .env("HOME", scratch.home())
        .env("PATH", path)
        .env("SHELL", "/bin/sh")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, value) in extra {
        cmd.env(key, value);
    }
    cmd.output().expect("qc invoke")
}

fn which_node() -> Option<PathBuf> {
    common::command_on_path("node")?
        .parent()
        .map(Path::to_path_buf)
}

fn timed_ok(
    exe: &Path,
    scratch: &Scratch,
    cwd: &Path,
    argv: &[&str],
    extra: &[(&str, &str)],
    expect_success: bool,
) -> f64 {
    let start = Instant::now();
    let output = run_qc(exe, scratch, cwd, argv, extra);
    let ms = start.elapsed().as_secs_f64() * 1000.0;
    let code = output.status.code();
    if expect_success {
        assert_eq!(
            code,
            Some(0),
            "failed {} {:?}\nstdout={}\nstderr={}",
            exe.display(),
            argv,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    } else {
        assert!(
            code.is_some(),
            "no exit status for {} {:?}",
            exe.display(),
            argv
        );
    }
    ms
}

fn stats(mut values: Vec<f64>) -> (f64, f64, f64, f64) {
    values.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let min = values[0];
    let max = *values.last().unwrap();
    let median = percentile(&values, 50.0);
    let p95 = percentile(&values, 95.0);
    (median, p95, min, max)
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let rank = (p / 100.0) * (sorted.len() - 1) as f64;
    let lo = rank.floor() as usize;
    let hi = rank.ceil() as usize;
    if lo == hi {
        sorted[lo]
    } else {
        let w = rank - lo as f64;
        sorted[lo] * (1.0 - w) + sorted[hi] * w
    }
}

fn sample_scenario(
    entry: &Entry,
    scratch: &Scratch,
    cwd: &Path,
    scenario: &str,
    argv: &[&str],
    extra: &[(&str, &str)],
    notes: &str,
) -> Row {
    for _ in 0..WARMUP {
        let _ = timed_ok(&entry.path, scratch, cwd, argv, extra, true);
    }
    let mut samples = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        samples.push(timed_ok(&entry.path, scratch, cwd, argv, extra, true));
    }
    let (median_ms, p95_ms, min_ms, max_ms) = stats(samples);
    Row {
        scenario: scenario.to_string(),
        entry: entry.name.to_string(),
        samples: SAMPLES,
        median_ms,
        p95_ms,
        min_ms,
        max_ms,
        notes: notes.to_string(),
    }
}

fn row_json(row: &Row) -> Value {
    json!({
        "scenario": row.scenario,
        "entry": row.entry,
        "samples": row.samples,
        "median_ms": (row.median_ms * 100.0).round() / 100.0,
        "p95_ms": (row.p95_ms * 100.0).round() / 100.0,
        "min_ms": (row.min_ms * 100.0).round() / 100.0,
        "max_ms": (row.max_ms * 100.0).round() / 100.0,
        "notes": row.notes,
    })
}

fn baseline_median(baseline: &Value, scenario: &str, entry: &str) -> Option<f64> {
    baseline["rows"].as_array()?.iter().find_map(|row| {
        if row["scenario"] == scenario && row["entry"] == entry {
            row["median_ms"].as_f64()
        } else {
            None
        }
    })
}

#[test]
fn records_release_artifact_timings() {
    let native_path = release_native();
    assert!(
        native_path.is_file(),
        "release qc missing at {} — run scripts/verification/benchmark.sh",
        native_path.display()
    );

    let mut entries = vec![Entry {
        name: "native qc",
        path: native_path.clone(),
    }];
    if let Some(path) = required_path("QC_BENCH_NPM") {
        assert!(path.is_file(), "npm launcher missing at {}", path.display());
        entries.push(Entry {
            name: "npm bin/qc",
            path,
        });
    }

    let scratch = unique_scratch("bench");
    install_fixture_names(&scratch.bin());
    write_file(
        &scratch.cwd().join(".qc/config.toml"),
        "[tool]\ndefault = \"pi\"\n",
    );
    write_file(
        &scratch.cwd().join(".qc/prompts/front.md"),
        "---\ndescription: Bench prompt\nqc_no_skills: true\n---\n\nHello from frontmatter.\n",
    );
    write_file(
        &scratch.cwd().join(".qc/prompts/shell.md"),
        "Status:\n\n!`printf hi`\n",
    );

    let extra_ok: &[(&str, &str)] = &[("QC_AGENT_TEXT", "ok")];
    let mut rows = Vec::new();

    // Fresh HOME bootstrap is one sample, not mixed into warm medians.
    let cold = unique_scratch("bench-cold");
    install_fixture_names(&cold.bin());
    let cold_ms = timed_ok(&native_path, &cold, &cold.cwd(), &["--version"], &[], true);
    rows.push(Row {
        scenario: "cold-HOME --version".into(),
        entry: "native qc".into(),
        samples: 1,
        median_ms: cold_ms,
        p95_ms: cold_ms,
        min_ms: cold_ms,
        max_ms: cold_ms,
        notes: "first repair on empty HOME".into(),
    });

    // Warm the shared HOME once so later samples skip first-run copy.
    let _ = timed_ok(
        &native_path,
        &scratch,
        &scratch.cwd(),
        &["--version"],
        &[],
        true,
    );

    for entry in &entries {
        rows.push(sample_scenario(
            entry,
            &scratch,
            &scratch.cwd(),
            "warm-HOME --version",
            &["--version"],
            &[],
            "steady-state after bootstrap",
        ));
        rows.push(sample_scenario(
            entry,
            &scratch,
            &scratch.cwd(),
            "warm-HOME --help",
            &["--help"],
            &[],
            "steady-state after bootstrap",
        ));
        rows.push(sample_scenario(
            entry,
            &scratch,
            &scratch.cwd(),
            "append-only fixture turn",
            &["-q", "--append", "inspect this repo"],
            extra_ok,
            "fixture pi; no LLM",
        ));
        rows.push(sample_scenario(
            entry,
            &scratch,
            &scratch.cwd(),
            "prompt/frontmatter/config fixture turn",
            &["-q", "front"],
            extra_ok,
            "fixture pi; no LLM",
        ));
        rows.push(sample_scenario(
            entry,
            &scratch,
            &scratch.cwd(),
            "shell-expansion fixture turn",
            &["-q", "shell"],
            extra_ok,
            "fixture pi plus !`printf hi`; no LLM",
        ));
    }

    let mut notes = vec![
        "Timings are qc overhead with fixture agents. They are not LLM inference.".to_string(),
        format!("native={}", native_path.display()),
    ];

    let baseline_path = required_path("QC_BENCH_BASELINE").or_else(|| {
        let path = common::crate_root()
            .join("..")
            .join(".tmp/rust-rewrite/startup-measurements.json");
        path.is_file().then_some(path)
    });
    let mut comparison = Vec::new();
    if let Some(path) = baseline_path.as_ref() {
        if let Ok(body) = fs::read_to_string(path)
            && let Ok(baseline) = serde_json::from_str::<Value>(&body)
        {
            for row in &rows {
                if row.entry != "npm bin/qc" {
                    continue;
                }
                if let Some(old) = baseline_median(&baseline, &row.scenario, "node dist/cli.js") {
                    let delta = row.median_ms - old;
                    let ratio = if old > 0.0 { row.median_ms / old } else { 0.0 };
                    let regression = delta > 15.0 && ratio > 1.15;
                    comparison.push(json!({
                        "scenario": row.scenario,
                        "m0_node_dist_cli_js_median_ms": (old * 100.0).round() / 100.0,
                        "m8_npm_bin_qc_median_ms": (row.median_ms * 100.0).round() / 100.0,
                        "delta_ms": (delta * 100.0).round() / 100.0,
                        "regression": regression,
                    }));
                    if regression {
                        notes.push(format!(
                            "npm-path slower than M0 node dist/cli.js on {}: {:.1}ms vs {:.1}ms. Node native.mjs selector plus exec, not the Rust turn.",
                            row.scenario, row.median_ms, old
                        ));
                    }
                }
            }
        }
        notes.push(format!("baseline={}", path.display()));
    } else {
        notes.push("M0 startup-measurements.json not found; no numeric comparison".into());
    }

    let report = json!({
        "generated": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        "warmup": WARMUP,
        "samples": SAMPLES,
        "native_path": native_path.to_string_lossy(),
        "npm_path": required_path("QC_BENCH_NPM").map(|p| p.to_string_lossy().into_owned()),
        "rows": rows.iter().map(row_json).collect::<Vec<_>>(),
        "comparison": comparison,
        "notes": notes,
    });

    let report_path = required_path("QC_BENCH_REPORT").unwrap_or_else(|| {
        common::crate_root()
            .join("..")
            .join(".tmp/rust-rewrite/benchmark.json")
    });
    if let Some(parent) = report_path.parent() {
        fs::create_dir_all(parent).expect("report dir");
    }
    fs::write(
        &report_path,
        serde_json::to_string_pretty(&report).expect("report json") + "\n",
    )
    .expect("write report");

    eprintln!("benchmark report {}", report_path.display());
    eprintln!(
        "{:<42} {:<14} {:>8} {:>8}",
        "scenario", "entry", "median", "p95"
    );
    for row in &rows {
        eprintln!(
            "{:<42} {:<14} {:>7.1}ms {:>7.1}ms",
            row.scenario, row.entry, row.median_ms, row.p95_ms
        );
    }
}
