use std::collections::HashMap;
use std::time::Instant;

use crate::common::{Scratch, unique_scratch};
use quickcall::config::Config;
use quickcall::shell_output::{expand_shell, expressions};

fn empty_config() -> Config {
    Config::default()
}

fn shell_context() -> (Scratch, HashMap<String, String>) {
    let scratch = unique_scratch("shell-output");
    let mut environment = HashMap::new();
    environment.insert("HOME".into(), scratch.home().to_string_lossy().into_owned());
    environment.insert("PATH".into(), "/usr/bin:/bin".into());
    environment.insert("SHELL".into(), "/bin/sh".into());
    (scratch, environment)
}

#[test]
fn executes_substitutions_concurrently_and_replaces_in_source_order() {
    let (scratch, environment) = shell_context();
    let result = expand_shell(
        "!`sleep 0.05; printf first` !`printf second`",
        "/bin/sh",
        &empty_config(),
        &scratch.cwd(),
        &environment,
    )
    .expect("expand");
    assert_eq!(result, "first second");
}

#[test]
fn validates_the_selected_shell_even_when_no_substitutions_exist() {
    let (scratch, environment) = shell_context();
    let missing = expand_shell(
        "plain",
        "/definitely/missing-shell",
        &empty_config(),
        &scratch.cwd(),
        &environment,
    )
    .expect_err("missing");
    assert!(
        missing.message.contains("shell executable"),
        "{}",
        missing.message
    );
    let slash = expand_shell("plain", "/", &empty_config(), &scratch.cwd(), &environment)
        .expect_err("slash");
    assert!(
        slash.message.contains("shell executable"),
        "{}",
        slash.message
    );
}

#[test]
fn executes_arithmetic_expansion_when_permissions_are_unconfigured() {
    let (scratch, environment) = shell_context();
    let result = expand_shell(
        "!`echo $((1 + 2))`",
        "/bin/sh",
        &empty_config(),
        &scratch.cwd(),
        &environment,
    )
    .expect("arith");
    assert_eq!(result, "3\n");
}

#[test]
fn discovers_only_complete_exact_expressions_expands_concurrently_and_does_not_recurse() {
    let (scratch, environment) = shell_context();
    assert_eq!(expressions("x !`printf one` !`printf two` !`open").len(), 2);
    let two = expand_shell(
        "!`printf first` !`printf second`",
        "/bin/sh",
        &empty_config(),
        &scratch.cwd(),
        &environment,
    )
    .expect("two");
    assert_eq!(two, "first second");
    let nested = expand_shell(
        "!`printf '\\041\\140nested\\140'`",
        "/bin/sh",
        &empty_config(),
        &scratch.cwd(),
        &environment,
    )
    .expect("nested");
    assert_eq!(nested, "!`nested`");
}

#[test]
fn keeps_stdout_from_failed_commands_drains_stderr_and_runs_in_one_concurrent_batch() {
    let (scratch, environment) = shell_context();
    let started = Instant::now();
    let output = expand_shell(
        "!`sleep 0.2; printf a` !`sleep 0.2; printf b` !`printf kept; false` !`yes x | head -c 100000 >&2; false`",
        "/bin/sh",
        &empty_config(),
        &scratch.cwd(),
        &environment,
    )
    .expect("batch");
    assert!(
        started.elapsed().as_millis() < 350,
        "expected concurrent batch, took {:?}",
        started.elapsed()
    );
    assert_eq!(output, "a b kept ");
}
