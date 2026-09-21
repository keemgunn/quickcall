use std::path::{Component, Path, PathBuf};

use serde_json::Value;

use crate::errors::QcError;
use crate::fsutil::qc_io;
use crate::messages::renamed_frontmatter_key;
use crate::parsers::parse_yaml_mapping;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PromptMeta {
    pub tool: Option<String>,
    pub model: Option<String>,
    pub thinking: Option<String>,
    pub workdir: Option<String>,
    pub no_skills: Option<bool>,
    pub skill_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prompt {
    pub path: PathBuf,
    pub body: String,
    pub meta: PromptMeta,
}

fn is_direct(reference: &str) -> bool {
    Path::new(reference).is_absolute()
        || reference.starts_with("./")
        || reference.starts_with("../")
        || reference.ends_with(".md")
}

/// Node `path.resolve` lexical form: absolute, `.` / `..` collapsed, no I/O.
fn lexical_normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => match out.components().next_back() {
                Some(Component::Normal(_)) => {
                    out.pop();
                }
                Some(Component::RootDir) | Some(Component::Prefix(_)) => {}
                Some(Component::ParentDir) | Some(Component::CurDir) | None => {
                    if !path.has_root() {
                        out.push(Component::ParentDir);
                    }
                }
            },
            other => out.push(other),
        }
    }
    if out.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        out
    }
}

fn node_resolve(cwd: &Path, reference: &str) -> PathBuf {
    lexical_normalize(&cwd.join(reference))
}

fn posix_relative(from: &Path, to: &Path) -> PathBuf {
    let from_c: Vec<_> = from.components().collect();
    let to_c: Vec<_> = to.components().collect();
    let mut i = 0;
    while i < from_c.len() && i < to_c.len() && from_c[i] == to_c[i] {
        i += 1;
    }
    let mut rel = PathBuf::new();
    for _ in i..from_c.len() {
        rel.push("..");
    }
    for comp in &to_c[i..] {
        rel.push(*comp);
    }
    rel
}

/// Node `stat().isFile()`, including symlink-to-file. Any error is "not a file".
fn regular_file(path: &Path) -> bool {
    std::fs::metadata(path).is_ok_and(|meta| meta.is_file())
}

/// realpath both sides, then Node `relative` + `startsWith("..")` string check.
fn contained(candidate: &Path, root: &Path) -> Result<bool, QcError> {
    let file =
        std::fs::canonicalize(candidate).map_err(|cause| qc_io("realpath", candidate, cause))?;
    let base = std::fs::canonicalize(root).map_err(|cause| qc_io("realpath", root, cause))?;
    let diff = posix_relative(&base, &file);
    let diff_str = diff.to_string_lossy();
    Ok(!diff_str.is_empty() && !diff_str.starts_with("..") && !diff.is_absolute())
}

pub fn resolve_prompt(reference: &str, cwd: &Path, home: &Path) -> Result<PathBuf, QcError> {
    if is_direct(reference) {
        let direct = node_resolve(cwd, reference);
        if !regular_file(&direct) {
            return Err(QcError::new(format!(
                "prompt file not found: {}",
                direct.display()
            )));
        }
        return Ok(direct);
    }
    if reference.split('/').any(|part| part == "..") || reference.starts_with('/') {
        return Err(QcError::new(format!(
            "prompt alias must stay below its prompts root: {reference}"
        )));
    }
    let name = format!("{reference}.md");
    let roots = [
        cwd.join(".qc").join("prompts"),
        home.join(".qc").join("prompts"),
    ];
    let candidates: Vec<PathBuf> = roots.iter().map(|root| root.join(&name)).collect();
    for (index, candidate) in candidates.iter().enumerate() {
        if !regular_file(candidate) {
            continue;
        }
        if !contained(candidate, &roots[index])? {
            return Err(QcError::new(format!(
                "prompt alias resolves outside its prompts root: {}",
                candidate.display()
            )));
        }
        return Ok(candidate.clone());
    }
    let listed = candidates
        .iter()
        .map(|path| format!("  {}", path.display()))
        .collect::<Vec<_>>()
        .join("\n");
    Err(QcError::new(format!(
        "prompt alias '{reference}' was not found. Checked:\n{listed}"
    )))
}

fn split_frontmatter(source: &str) -> Result<Option<(&str, &str)>, ()> {
    let after_open = if let Some(rest) = source.strip_prefix("---\n") {
        rest
    } else if let Some(rest) = source.strip_prefix("---\r\n") {
        rest
    } else {
        return Ok(None);
    };
    // `/^---\r?\n([\s\S]*?)\r?\n---\r?\n?/` — first newline-then-`---` closes.
    let crlf = after_open.find("\r\n---").map(|pos| (pos, pos + 5));
    let lf = after_open.find("\n---").map(|pos| (pos, pos + 4));
    let (yaml_end, after_dashes) = match (crlf, lf) {
        (Some((a, ae)), Some((b, _))) if a <= b => (a, ae),
        (Some(_), Some((b, be))) => (b, be),
        (Some((a, ae)), None) => (a, ae),
        (None, Some((b, be))) => (b, be),
        (None, None) => return Err(()),
    };
    let yaml = &after_open[..yaml_end];
    let rest = &after_open[after_dashes..];
    let body = rest
        .strip_prefix("\r\n")
        .or_else(|| rest.strip_prefix('\n'))
        .unwrap_or(rest);
    Ok(Some((yaml, body)))
}

fn yaml_non_empty_string<'a>(value: &'a Value, key: &str, path: &str) -> Result<&'a str, QcError> {
    match value {
        Value::String(text) if !text.is_empty() => Ok(text),
        _ => Err(QcError::new(format!(
            "{key} in {path} must be a non-empty string"
        ))),
    }
}

fn yaml_bool(value: &Value, key: &str, path: &str) -> Result<bool, QcError> {
    match value {
        Value::Bool(flag) => Ok(*flag),
        _ => Err(QcError::new(format!(
            "{key} in {path} must be true or false"
        ))),
    }
}

pub fn parse_prompt(source: &str, path: &str, home: &str) -> Result<Prompt, QcError> {
    let Some((yaml_text, body)) = split_frontmatter(source)
        .map_err(|()| QcError::new(format!("invalid YAML frontmatter in {path}")))?
    else {
        return Ok(Prompt {
            path: PathBuf::from(path),
            body: source.to_string(),
            meta: PromptMeta::default(),
        });
    };

    // TS treats YAML parse errors the same as a non-mapping document.
    let values = parse_yaml_mapping(yaml_text)
        .map_err(|_| QcError::new(format!("YAML frontmatter in {path} must be a mapping")))?;

    let known = [
        "description",
        "qc_tool",
        "qc_model",
        "qc_thinking",
        "qc_workdir",
        "qc_no_skills",
        "qc_skill_path",
    ];
    for key in values.keys() {
        if key == "qc_cli" {
            return Err(QcError::new(renamed_frontmatter_key("qc_cli", "qc_tool")));
        }
        if key == "qc_approve" {
            return Err(QcError::new(renamed_frontmatter_key(
                "qc_approve",
                "nothing (agents always force-allow)",
            )));
        }
        if key.starts_with("pqi_") || key.starts_with("pi_") || key.starts_with("qpi_") {
            return Err(QcError::new(format!(
                "frontmatter key '{key}' in {path} is not supported; use qc_* keys"
            )));
        }
        if key.starts_with("qc_") && !known.contains(&key.as_str()) {
            return Err(QcError::new(format!(
                "unknown qc frontmatter key '{key}' in {path}"
            )));
        }
    }

    if let Some(description) = values.get("description")
        && !description.is_string()
    {
        return Err(QcError::new(format!(
            "description in {path} must be a string"
        )));
    }

    let string_field = |key: &str| -> Result<Option<String>, QcError> {
        match values.get(key) {
            None => Ok(None),
            Some(value) => Ok(Some(yaml_non_empty_string(value, key, path)?.to_string())),
        }
    };
    let bool_field = |key: &str| -> Result<Option<bool>, QcError> {
        match values.get(key) {
            None => Ok(None),
            Some(value) => Ok(Some(yaml_bool(value, key, path)?)),
        }
    };

    let mut meta = PromptMeta {
        tool: string_field("qc_tool")?,
        model: string_field("qc_model")?,
        thinking: string_field("qc_thinking")?,
        workdir: string_field("qc_workdir")?,
        no_skills: bool_field("qc_no_skills")?,
        skill_path: None,
    };
    if let Some(skill) = string_field("qc_skill_path")? {
        meta.skill_path = Some(if let Some(rest) = skill.strip_prefix("~/") {
            Path::new(home).join(rest).to_string_lossy().into_owned()
        } else {
            skill
        });
    }

    Ok(Prompt {
        path: PathBuf::from(path),
        body: body.to_string(),
        meta,
    })
}

pub fn read_prompt(reference: &str, cwd: &Path, home: &Path) -> Result<Prompt, QcError> {
    let path = resolve_prompt(reference, cwd, home)?;
    let bytes = std::fs::read(&path).map_err(|cause| qc_io("read", &path, cause))?;
    let source = String::from_utf8_lossy(&bytes);
    parse_prompt(&source, &path.to_string_lossy(), &home.to_string_lossy())
}

/// Normal append wraps user text. No prompt file → raw append is the whole turn.
pub fn append_prompt(body: &str, append: Option<&str>, raw_append: bool) -> String {
    let Some(append) = append else {
        return body.to_string();
    };
    if raw_append {
        return append.to_string();
    }
    format!("{body}\n\n---\n\nAdditional Message from the user:\n\n{append}")
}
