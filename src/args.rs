use crate::errors::QcError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutputMode {
    Text,
    Json,
}

/// `true` in TypeScript is [`OpenArg::Bare`]; a string is [`OpenArg::Id`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenArg {
    Bare,
    Id(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Args {
    pub prompt: Option<String>,
    pub shell: Option<String>,
    pub append: Option<String>,
    pub tool: Option<String>,
    pub model: Option<String>,
    pub thinking: Option<String>,
    pub workdir: Option<String>,
    pub output: Option<OutputMode>,
    pub debug: bool,
    pub quiet: bool,
    pub continue_id: Option<String>,
    pub open: Option<OpenArg>,
    pub skill: Option<String>,
    pub no_skills: bool,
    pub help: bool,
    pub version: bool,
    pub install_sample_prompts: bool,
    pub install_agent_harness: bool,
}

fn canonical(token: &str) -> &str {
    match token {
        "-h" => "--help",
        "-v" => "--version",
        "-s" => "--shell",
        "-a" => "--append",
        "-c" => "--continue",
        "-o" => "--open",
        "-q" => "--quiet",
        other => other,
    }
}

fn is_option_token(token: &str) -> bool {
    matches!(
        token,
        "-h" | "--help"
            | "-v"
            | "--version"
            | "-s"
            | "--shell"
            | "-a"
            | "--append"
            | "-c"
            | "--continue"
            | "-o"
            | "--open"
            | "-q"
            | "--quiet"
            | "--tool"
            | "--model"
            | "--thinking"
            | "--workdir"
            | "--output"
            | "--debug"
            | "--skill"
            | "--no-skills"
            | "--install-sample-prompts"
            | "--install-agent-harness"
    )
}

fn assert_exclusive(result: &Args, flag: &str) -> Result<(), QcError> {
    if result.prompt.is_some()
        || result.shell.is_some()
        || result.append.is_some()
        || result.tool.is_some()
        || result.model.is_some()
        || result.thinking.is_some()
        || result.workdir.is_some()
        || result.output.is_some()
        || result.debug
        || result.continue_id.is_some()
        || result.skill.is_some()
        || result.no_skills
    {
        return Err(QcError::new(format!(
            "{flag} cannot be combined with a prompt reference"
        )));
    }
    if result.help
        || result.version
        || result.install_sample_prompts
        || result.install_agent_harness
    {
        return Err(QcError::new(format!("{flag} must be used alone")));
    }
    Ok(())
}

fn assert_open_exclusive(result: &Args) -> Result<(), QcError> {
    if result.prompt.is_some() {
        return Err(QcError::new(
            "--open cannot be combined with a prompt reference",
        ));
    }
    if result.continue_id.is_some() {
        return Err(QcError::new("--open cannot be combined with --continue"));
    }
    if result.append.is_some() {
        return Err(QcError::new("--open cannot be combined with --append"));
    }
    if result.output.is_some() {
        return Err(QcError::new("--open cannot be combined with --output"));
    }
    if result.debug {
        return Err(QcError::new("--open cannot be combined with --debug"));
    }
    if result.quiet {
        return Err(QcError::new("--open cannot be combined with --quiet"));
    }
    if result.model.is_some() {
        return Err(QcError::new("--open cannot be combined with --model"));
    }
    if result.thinking.is_some() {
        return Err(QcError::new("--open cannot be combined with --thinking"));
    }
    if result.workdir.is_some() {
        return Err(QcError::new("--open cannot be combined with --workdir"));
    }
    if result.skill.is_some() {
        return Err(QcError::new("--open cannot be combined with --skill"));
    }
    if result.no_skills {
        return Err(QcError::new("--open cannot be combined with --no-skills"));
    }
    if result.shell.is_some() {
        return Err(QcError::new("--open cannot be combined with --shell"));
    }
    Ok(())
}

fn take_optional_open_id(argv: &[String], index: usize) -> (Option<&str>, usize) {
    match argv.get(index + 1) {
        None => (None, index),
        Some(next) if next.starts_with('-') || is_option_token(next) => (None, index),
        Some(next) => (Some(next.as_str()), index + 1),
    }
}

fn take_value<'a>(
    argv: &'a [String],
    index: usize,
    flag: &str,
) -> Result<(&'a str, usize), QcError> {
    let next = argv.get(index + 1);
    let missing = match next {
        None => true,
        Some(value) => {
            value.starts_with("--")
                || is_option_token(value)
                || (flag == "--shell" && value.is_empty())
        }
    };
    if missing {
        return Err(QcError::new(format!("{flag} requires a value")));
    }
    Ok((next.expect("value present").as_str(), index + 1))
}

pub fn parse_args(argv: &[String]) -> Result<Args, QcError> {
    let mut result = Args::default();
    let mut index = 0;
    while index < argv.len() {
        let value = canonical(&argv[index]);

        if value == "--help" {
            if result.help {
                return Err(QcError::new("--help was provided more than once"));
            }
            assert_exclusive(&result, value)?;
            result.help = true;
            index += 1;
            continue;
        }

        if value == "--version" {
            if result.version {
                return Err(QcError::new("--version was provided more than once"));
            }
            assert_exclusive(&result, value)?;
            result.version = true;
            index += 1;
            continue;
        }

        if value == "--install-sample-prompts" {
            if result.install_sample_prompts {
                return Err(QcError::new(
                    "--install-sample-prompts was provided more than once",
                ));
            }
            assert_exclusive(&result, value)?;
            result.install_sample_prompts = true;
            index += 1;
            continue;
        }

        if value == "--install-agent-harness" {
            if result.install_agent_harness {
                return Err(QcError::new(
                    "--install-agent-harness was provided more than once",
                ));
            }
            assert_exclusive(&result, value)?;
            result.install_agent_harness = true;
            index += 1;
            continue;
        }

        if value == "--no-skills" {
            if result.no_skills {
                return Err(QcError::new("--no-skills was provided more than once"));
            }
            result.no_skills = true;
            index += 1;
            continue;
        }

        if value == "--debug" {
            if result.debug {
                return Err(QcError::new("--debug was provided more than once"));
            }
            result.debug = true;
            index += 1;
            continue;
        }

        if value == "--quiet" {
            if result.quiet {
                return Err(QcError::new("--quiet was provided more than once"));
            }
            result.quiet = true;
            index += 1;
            continue;
        }

        if value == "--open" {
            if result.open.is_some() {
                return Err(QcError::new("--open was provided more than once"));
            }
            let (id, next) = take_optional_open_id(argv, index);
            result.open = Some(match id {
                Some(value) => OpenArg::Id(value.to_string()),
                None => OpenArg::Bare,
            });
            index = next + 1;
            continue;
        }

        if matches!(
            value,
            "--shell"
                | "--append"
                | "--tool"
                | "--model"
                | "--thinking"
                | "--workdir"
                | "--output"
                | "--continue"
                | "--skill"
        ) {
            let (taken, next) = take_value(argv, index, value)?;
            match value {
                "--shell" => {
                    if result.shell.is_some() {
                        return Err(QcError::new("--shell was provided more than once"));
                    }
                    result.shell = Some(taken.to_string());
                }
                "--append" => {
                    if result.append.is_some() {
                        return Err(QcError::new("--append was provided more than once"));
                    }
                    result.append = Some(taken.to_string());
                }
                "--tool" => {
                    if result.tool.is_some() {
                        return Err(QcError::new("--tool was provided more than once"));
                    }
                    result.tool = Some(taken.to_string());
                }
                "--model" => {
                    if result.model.is_some() {
                        return Err(QcError::new("--model was provided more than once"));
                    }
                    result.model = Some(taken.to_string());
                }
                "--thinking" => {
                    if result.thinking.is_some() {
                        return Err(QcError::new("--thinking was provided more than once"));
                    }
                    result.thinking = Some(taken.to_string());
                }
                "--workdir" => {
                    if result.workdir.is_some() {
                        return Err(QcError::new("--workdir was provided more than once"));
                    }
                    result.workdir = Some(taken.to_string());
                }
                "--output" => {
                    if result.output.is_some() {
                        return Err(QcError::new("--output was provided more than once"));
                    }
                    result.output = Some(match taken {
                        "text" => OutputMode::Text,
                        "json" => OutputMode::Json,
                        _ => return Err(QcError::new("--output must be 'text' or 'json'")),
                    });
                }
                "--continue" => {
                    if result.continue_id.is_some() {
                        return Err(QcError::new("--continue was provided more than once"));
                    }
                    result.continue_id = Some(taken.to_string());
                }
                "--skill" => {
                    if result.skill.is_some() {
                        return Err(QcError::new("--skill was provided more than once"));
                    }
                    result.skill = Some(taken.to_string());
                }
                _ => unreachable!("matched option list"),
            }
            index = next + 1;
            continue;
        }

        if value.starts_with('-') {
            return Err(QcError::new(format!("unknown option '{value}'")));
        }
        if result.prompt.is_some() {
            return Err(QcError::new("only one prompt reference is allowed"));
        }
        result.prompt = Some(value.to_string());
        index += 1;
    }

    let informational = result.help
        || result.version
        || result.install_sample_prompts
        || result.install_agent_harness;

    if result.open.is_some() {
        assert_open_exclusive(&result)?;
    }

    if !informational && result.open.is_none() && result.prompt.is_none() && result.append.is_none()
    {
        return Err(QcError::new("a prompt reference or --append is required"));
    }

    if (result.help || result.version) && result.prompt.is_some() {
        return Err(QcError::new(
            "--help and --version cannot be combined with a prompt reference",
        ));
    }

    if result.help && result.version {
        return Err(QcError::new("--help and --version cannot be combined"));
    }

    if (result.help || result.version) && argv.len() != 1 {
        return Err(QcError::new("--help and --version must be used alone"));
    }

    if result.install_sample_prompts && argv.len() != 1 {
        return Err(QcError::new("--install-sample-prompts must be used alone"));
    }

    if result.install_agent_harness && argv.len() != 1 {
        return Err(QcError::new("--install-agent-harness must be used alone"));
    }

    Ok(result)
}
