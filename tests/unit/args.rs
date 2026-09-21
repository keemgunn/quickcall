use quickcall::QcError;
use quickcall::args::{OpenArg, OutputMode, parse_args};

fn argv(tokens: &[&str]) -> Vec<String> {
    tokens.iter().map(|token| (*token).to_string()).collect()
}

fn parse(tokens: &[&str]) -> quickcall::args::Args {
    parse_args(&argv(tokens)).unwrap_or_else(|cause| panic!("parse {tokens:?}: {cause}"))
}

fn parse_err(tokens: &[&str]) -> QcError {
    parse_args(&argv(tokens)).expect_err("expected parse error")
}

#[test]
fn accepts_prompt_forms() {
    let cases: &[&[&str]] = &[
        &["daily/report"],
        &["daily/report", "--shell", "/bin/sh"],
        &["daily/report", "--append", "hello"],
        &[
            "daily/report",
            "--tool",
            "cursor",
            "--model",
            "composer-2.5",
        ],
        &["daily/report", "--thinking", "high", "--workdir", "/tmp/w"],
        &["daily/report", "--output", "json"],
        &["daily/report", "--debug"],
        &["daily/report", "-q"],
        &["daily/report", "--quiet"],
        &["daily/report", "-c", "260901-1200--pi--a1b2c3"],
        &["daily/report", "--skill", "/skills", "--no-skills"],
    ];
    for tokens in cases {
        assert_eq!(
            parse(tokens).prompt.as_deref(),
            Some("daily/report"),
            "{tokens:?}"
        );
    }
}

#[test]
fn accepts_continue_plus_append_without_a_prompt() {
    let parsed = parse(&["-c", "260901-1200--pi--a1b2c3", "-a", "also add tests"]);
    assert_eq!(
        parsed.continue_id.as_deref(),
        Some("260901-1200--pi--a1b2c3")
    );
    assert_eq!(parsed.append.as_deref(), Some("also add tests"));
    assert_eq!(parsed.prompt, None);
}

#[test]
fn accepts_append_only_without_a_prompt() {
    let parsed = parse(&["-a", "only"]);
    assert_eq!(parsed.append.as_deref(), Some("only"));
    assert_eq!(parsed.prompt, None);
}

#[test]
fn accepts_append_only_with_tool_and_model() {
    let parsed = parse(&[
        "--tool",
        "cursor",
        "--model",
        "composer-2.5",
        "--append",
        "inspect this repo",
    ]);
    assert_eq!(parsed.tool.as_deref(), Some("cursor"));
    assert_eq!(parsed.model.as_deref(), Some("composer-2.5"));
    assert_eq!(parsed.append.as_deref(), Some("inspect this repo"));
    assert_eq!(parsed.prompt, None);
}

#[test]
fn accepts_bare_open_without_a_prompt() {
    let parsed = parse(&["-o"]);
    assert_eq!(parsed.open, Some(OpenArg::Bare));
    assert_eq!(parsed.prompt, None);
    assert_eq!(parse(&["--open"]), parsed);
}

#[test]
fn consumes_the_next_token_as_an_open_id_when_it_is_not_an_option() {
    let id = "260901-1200--pi--a1b2c3";
    let parsed = parse(&["-o", id]);
    assert_eq!(parsed.open, Some(OpenArg::Id(id.to_string())));
    assert_eq!(parsed.prompt, None);
    assert_eq!(parse(&["--open", id]), parsed);
    let with_tool = parse(&["-o", id, "--tool", "pi"]);
    assert_eq!(with_tool.open, Some(OpenArg::Id(id.to_string())));
    assert_eq!(with_tool.tool.as_deref(), Some("pi"));
    let lookahead = parse(&["-o", "--tool", "cursor"]);
    assert_eq!(lookahead.open, Some(OpenArg::Bare));
    assert_eq!(lookahead.tool.as_deref(), Some("cursor"));
}

#[test]
fn rejects_attached_open_forms_as_unknown_options() {
    assert_eq!(
        parse_err(&["--open=260901-1200--pi--a1b2c3"]).message,
        "unknown option '--open=260901-1200--pi--a1b2c3'"
    );
    assert_eq!(
        parse_err(&["-o260901-1200--pi--a1b2c3"]).message,
        "unknown option '-o260901-1200--pi--a1b2c3'"
    );
}

#[test]
fn parses_quiet_as_one_identity() {
    let short = parse(&["daily/report", "-q"]);
    assert_eq!(short.prompt.as_deref(), Some("daily/report"));
    assert!(short.quiet);
    assert_eq!(parse(&["daily/report", "--quiet"]), short);
    let append = parse(&["-q", "--append", "inspect this repo"]);
    assert!(append.quiet);
    assert_eq!(append.append.as_deref(), Some("inspect this repo"));
    let mixed = parse(&[
        "review",
        "-q",
        "--tool",
        "cursor",
        "--model",
        "composer-2.5",
        "--debug",
        "--output",
        "json",
    ]);
    assert_eq!(mixed.prompt.as_deref(), Some("review"));
    assert!(mixed.quiet);
    assert_eq!(mixed.tool.as_deref(), Some("cursor"));
    assert_eq!(mixed.model.as_deref(), Some("composer-2.5"));
    assert!(mixed.debug);
    assert_eq!(mixed.output, Some(OutputMode::Json));
}

#[test]
fn parses_new_option_values() {
    let parsed = parse(&[
        "p",
        "--tool",
        "claude",
        "--model",
        "sonnet",
        "--thinking",
        "high",
        "--workdir",
        "/w",
        "--output",
        "json",
        "--debug",
        "--continue",
        "260901-1200--claude--abcdef",
        "--skill",
        "/s",
        "--no-skills",
    ]);
    assert_eq!(parsed.prompt.as_deref(), Some("p"));
    assert_eq!(parsed.tool.as_deref(), Some("claude"));
    assert_eq!(parsed.model.as_deref(), Some("sonnet"));
    assert_eq!(parsed.thinking.as_deref(), Some("high"));
    assert_eq!(parsed.workdir.as_deref(), Some("/w"));
    assert_eq!(parsed.output, Some(OutputMode::Json));
    assert!(parsed.debug);
    assert_eq!(
        parsed.continue_id.as_deref(),
        Some("260901-1200--claude--abcdef")
    );
    assert_eq!(parsed.skill.as_deref(), Some("/s"));
    assert!(parsed.no_skills);
}

#[test]
fn preserves_prototype_key_prompt_references() {
    for prompt in ["constructor", "toString", "__proto__"] {
        assert_eq!(parse(&[prompt]).prompt.as_deref(), Some(prompt));
    }
}

#[test]
fn accepts_unrecognized_single_dash_values() {
    assert_eq!(
        parse(&["daily/report", "--shell", "-custom-shell"])
            .shell
            .as_deref(),
        Some("-custom-shell")
    );
    assert_eq!(
        parse(&["daily/report", "--append", "-message"])
            .append
            .as_deref(),
        Some("-message")
    );
}

#[test]
fn accepts_standalone_help_version_and_install() {
    assert!(parse(&["--help"]).help);
    assert!(parse(&["-h"]).help);
    assert!(parse(&["--version"]).version);
    assert!(parse(&["-v"]).version);
    assert!(parse(&["--install-sample-prompts"]).install_sample_prompts);
}

#[test]
fn accepts_standalone_harness_install() {
    let parsed = parse(&["--install-agent-harness"]);
    assert!(!parsed.help);
    assert!(!parsed.version);
    assert!(!parsed.install_sample_prompts);
    assert!(parsed.install_agent_harness);
}

#[test]
fn normalizes_short_aliases() {
    let pairs: &[(&[&str], &[&str])] = &[
        (
            &["daily/report", "-s", "/bin/sh"],
            &["daily/report", "--shell", "/bin/sh"],
        ),
        (
            &["daily/report", "-a", "hello"],
            &["daily/report", "--append", "hello"],
        ),
        (
            &["daily/report", "-c", "id"],
            &["daily/report", "--continue", "id"],
        ),
        (
            &["-o", "260901-1200--pi--a1b2c3"],
            &["--open", "260901-1200--pi--a1b2c3"],
        ),
        (&["-o"], &["--open"]),
        (&["-h"], &["--help"]),
        (&["-v"], &["--version"]),
        (&["daily/report", "-q"], &["daily/report", "--quiet"]),
    ];
    for (short, long) in pairs {
        assert_eq!(parse(short), parse(long), "{short:?} vs {long:?}");
    }
}

#[test]
fn uses_canonical_diagnostics() {
    let cases: &[(&[&str], &str)] = &[
        (&["daily/report", "-s"], "--shell requires a value"),
        (&["daily/report", "-a"], "--append requires a value"),
        (&["daily/report", "-c"], "--continue requires a value"),
        (
            &["daily/report", "--output", "xml"],
            "--output must be 'text' or 'json'",
        ),
        (
            &["daily/report", "-s", "-a", "hello"],
            "--shell requires a value",
        ),
        (
            &["daily/report", "--tool", "--wat"],
            "--tool requires a value",
        ),
        (
            &["daily/report", "-s", "/bin/sh", "--shell", "/bin/zsh"],
            "--shell was provided more than once",
        ),
        (&["-h", "--help"], "--help was provided more than once"),
        (
            &["--install-agent-harness", "cursor"],
            "--install-agent-harness must be used alone",
        ),
        (
            &["review", "-o"],
            "--open cannot be combined with a prompt reference",
        ),
        (
            &["-o", "-c", "260901-1200--pi--a1b2c3"],
            "--open cannot be combined with --continue",
        ),
        (
            &["-o", "-a", "x"],
            "--open cannot be combined with --append",
        ),
        (
            &["-o", "--output", "json"],
            "--open cannot be combined with --output",
        ),
        (&["-o", "--debug"], "--open cannot be combined with --debug"),
        (&["-o", "-q"], "--open cannot be combined with --quiet"),
        (
            &["--quiet", "-o", "260901-1200--pi--a1b2c3"],
            "--open cannot be combined with --quiet",
        ),
        (
            &["review", "-q", "--quiet"],
            "--quiet was provided more than once",
        ),
        (
            &["review", "-q", "-q"],
            "--quiet was provided more than once",
        ),
        (&["-q", "-h"], "--help and --version must be used alone"),
        (
            &["--quiet", "--version"],
            "--help and --version must be used alone",
        ),
        (
            &["-q", "--install-agent-harness"],
            "--install-agent-harness must be used alone",
        ),
        (&["review", "-qv"], "unknown option '-qv'"),
        (&["-qc", "review"], "unknown option '-qc'"),
        (
            &["-o", "--model", "m"],
            "--open cannot be combined with --model",
        ),
        (
            &["-o", "--thinking", "high"],
            "--open cannot be combined with --thinking",
        ),
        (
            &["-o", "--workdir", "/w"],
            "--open cannot be combined with --workdir",
        ),
        (
            &["-o", "--skill", "/s"],
            "--open cannot be combined with --skill",
        ),
        (
            &["-o", "--no-skills"],
            "--open cannot be combined with --no-skills",
        ),
        (
            &["-o", "-s", "/bin/sh"],
            "--open cannot be combined with --shell",
        ),
        (&["-o", "--help"], "--help and --version must be used alone"),
    ];
    for (tokens, message) in cases {
        assert_eq!(parse_err(tokens).message, *message, "{tokens:?}");
    }
}

#[test]
fn rejects_invalid_grammar() {
    let cases: &[&[&str]] = &[
        &[],
        &["a", "b"],
        &["a", "--wat"],
        &["--shell"],
        &["--append"],
        &["-c", "id"],
        &["--tool", "cursor"],
        &["review", "-o"],
        &["-o", "-c", "id"],
        &["-o", "-a", "x"],
        &["-o", "--debug"],
        &["-o", "-q"],
        &["review", "-q", "--quiet"],
        &["review", "-qv"],
        &["-qc", "review"],
        &["-q", "-h"],
        &["--open=id"],
        &["-o260901"],
        &["--help", "--version"],
        &["--help", "a"],
        &["--version", "a"],
        &["-hv"],
        &["a", "-s/bin/sh"],
        &["--install-sample-prompts", "a"],
        &["--install-agent-harness", "cursor"],
        &["daily/report", "--install-sample-prompts"],
    ];
    for tokens in cases {
        parse_args(&argv(tokens)).expect_err(&format!("expected rejection for {tokens:?}"));
    }
}

#[test]
fn requires_prompt_or_append() {
    let cases: &[(&[&str], &str)] = &[
        (&[], "a prompt reference or --append is required"),
        (
            &["--tool", "cursor"],
            "a prompt reference or --append is required",
        ),
        (&["-c", "id"], "a prompt reference or --append is required"),
    ];
    for (tokens, message) in cases {
        assert_eq!(parse_err(tokens).message, *message, "{tokens:?}");
    }
}

#[test]
fn accepts_standalone_help_and_version_aliases() {
    let help = parse(&["--help"]);
    assert!(help.help);
    assert!(!help.version);
    assert_eq!(help, parse(&["-h"]));

    let version = parse(&["--version"]);
    assert!(version.version);
    assert!(!version.help);
    assert_eq!(version, parse(&["-v"]));
}

#[test]
fn rejects_help_and_version_combinations_with_qc_diagnostics() {
    assert_eq!(
        parse_err(&["--help", "--version"]).message,
        "--version must be used alone"
    );
    assert_eq!(
        parse_err(&["--help", "a"]).message,
        "--help and --version cannot be combined with a prompt reference"
    );
    assert_eq!(
        parse_err(&["--version", "a"]).message,
        "--help and --version cannot be combined with a prompt reference"
    );
    assert_eq!(
        parse_err(&["-h", "--help"]).message,
        "--help was provided more than once"
    );
    assert_eq!(
        parse_err(&["-q", "-h"]).message,
        "--help and --version must be used alone"
    );
    assert_eq!(
        parse_err(&["--quiet", "--version"]).message,
        "--help and --version must be used alone"
    );
}

#[test]
fn rejects_unknown_options_and_empty_argv() {
    assert_eq!(parse_err(&["--wat"]).message, "unknown option '--wat'");
    assert_eq!(
        parse_err(&[]).message,
        "a prompt reference or --append is required"
    );
    assert_eq!(
        parse_err(&["review", "-qv"]).message,
        "unknown option '-qv'"
    );
}
