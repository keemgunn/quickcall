use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use chrono::{DateTime, Local};
use serde_json::{Map, Value, json};

use crate::errors::QcError;
use crate::fsutil::qc_io;
use crate::parsers::{parse_json, pretty_clock_stamp};
use crate::tools::ToolName;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionMapping {
    pub tool: ToolName,
    pub native_id: String,
    pub cwd: String,
    pub created: String,
    pub updated: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedSessionId {
    pub tool: ToolName,
    pub stamp: String,
    pub suffix: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundSession {
    pub id: String,
    pub mapping: SessionMapping,
}

const SUFFIX_FILL: &str = "a1b2c3";

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

fn os_random_4() -> [u8; 4] {
    let mut buf = [0u8; 4];
    // Node `crypto.randomBytes(4)` — OS entropy, not a PRNG.
    // musl libc crate does not export getentropy; /dev/urandom is the same OS entropy.
    #[cfg(not(target_env = "musl"))]
    {
        let rc = unsafe { libc::getentropy(buf.as_mut_ptr().cast(), buf.len()) };
        if rc == 0 {
            return buf;
        }
    }
    let mut file = fs::File::open("/dev/urandom").expect("OS randomness");
    file.read_exact(&mut buf).expect("OS randomness");
    buf
}

fn base64url_4(bytes: [u8; 4]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let x = (u32::from(bytes[0]) << 16) | (u32::from(bytes[1]) << 8) | u32::from(bytes[2]);
    let y = u32::from(bytes[3]) << 16;
    let mut out = String::with_capacity(6);
    out.push(TABLE[((x >> 18) & 63) as usize] as char);
    out.push(TABLE[((x >> 12) & 63) as usize] as char);
    out.push(TABLE[((x >> 6) & 63) as usize] as char);
    out.push(TABLE[(x & 63) as usize] as char);
    out.push(TABLE[((y >> 18) & 63) as usize] as char);
    out.push(TABLE[((y >> 12) & 63) as usize] as char);
    out
}

fn pretty_suffix() -> String {
    let encoded = base64url_4(os_random_4()).to_ascii_lowercase();
    let stripped: String = encoded
        .chars()
        .filter(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit())
        .take(6)
        .collect();
    format!("{stripped}{SUFFIX_FILL}").chars().take(6).collect()
}

/// Local-time pretty id: `yymmdd-hhmm--<tool>--<6 lowercase alnum>`.
pub fn mint_session_id(tool: ToolName, now: Option<DateTime<Local>>) -> String {
    let now = now.unwrap_or_else(Local::now);
    let stamp = pretty_clock_stamp(now);
    let suffix = pretty_suffix();
    format!("{stamp}--{tool}--{suffix}")
}

fn stamp_ok(stamp: &str) -> bool {
    let bytes = stamp.as_bytes();
    bytes.len() == 11
        && bytes[6] == b'-'
        && bytes.iter().enumerate().all(|(i, b)| {
            if i == 6 {
                *b == b'-'
            } else {
                b.is_ascii_digit()
            }
        })
}

fn suffix_ok(suffix: &str) -> bool {
    suffix.len() == 6
        && suffix
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
}

pub fn parse_session_id(id: &str) -> Result<ParsedSessionId, QcError> {
    let mut parts = id.splitn(3, "--");
    let stamp = parts.next().unwrap_or("");
    let tool_name = parts.next().unwrap_or("");
    let suffix = parts.next().unwrap_or("");
    let form_ok = stamp_ok(stamp)
        && !tool_name.is_empty()
        && tool_name.bytes().all(|b| b.is_ascii_lowercase())
        && suffix_ok(suffix);
    if !form_ok {
        return Err(QcError::new(format!("invalid session id '{id}'")));
    }
    let tool = ToolName::parse(tool_name)
        .ok_or_else(|| QcError::new(format!("invalid session id tool '{tool_name}'")))?;
    Ok(ParsedSessionId {
        tool,
        stamp: stamp.to_string(),
        suffix: suffix.to_string(),
    })
}

pub fn sessions_dir(home: impl AsRef<Path>) -> PathBuf {
    home.as_ref().join(".qc").join("sessions")
}

pub fn session_path(home: impl AsRef<Path>, id: &str) -> PathBuf {
    sessions_dir(home).join(format!("{id}.json"))
}

fn js_string(value: &Value) -> String {
    match value {
        Value::Null => "null".into(),
        Value::Bool(true) => "true".into(),
        Value::Bool(false) => "false".into(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.clone(),
        Value::Array(items) => items.iter().map(js_string).collect::<Vec<_>>().join(","),
        Value::Object(_) => "[object Object]".into(),
    }
}

fn mapping_from_json(id: &str, parsed: Value) -> Result<SessionMapping, QcError> {
    let Value::Object(map) = parsed else {
        return Err(QcError::new(format!("corrupt session mapping: {id}")));
    };
    let tool = map
        .get("tool")
        .and_then(Value::as_str)
        .and_then(ToolName::parse);
    let native_id = map.get("native_id").and_then(Value::as_str);
    let cwd = map.get("cwd").and_then(Value::as_str);
    let Some(tool) = tool else {
        return Err(QcError::new(format!("corrupt session mapping: {id}")));
    };
    let Some(native_id) = native_id.filter(|text| !text.is_empty()) else {
        return Err(QcError::new(format!("corrupt session mapping: {id}")));
    };
    let Some(cwd) = cwd.filter(|text| !text.is_empty()) else {
        return Err(QcError::new(format!("corrupt session mapping: {id}")));
    };
    let created = match map.get("created") {
        Some(Value::String(text)) => text.clone(),
        _ => String::new(),
    };
    let updated = match map.get("updated") {
        Some(Value::String(text)) => text.clone(),
        _ => String::new(),
    };
    let warnings = match map.get("warnings") {
        Some(Value::Array(items)) => items.iter().map(js_string).collect(),
        _ => Vec::new(),
    };
    Ok(SessionMapping {
        tool,
        native_id: native_id.to_string(),
        cwd: cwd.to_string(),
        created,
        updated,
        warnings,
    })
}

pub fn load_session(home: impl AsRef<Path>, id: &str) -> Result<SessionMapping, QcError> {
    parse_session_id(id)?;
    let path = session_path(home, id);
    let raw = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Err(QcError::new(format!("session not found: {id}")));
        }
        Err(err) => return Err(qc_io("read", &path, err)),
    };
    let parsed =
        parse_json(&raw).map_err(|_| QcError::new(format!("corrupt session mapping: {id}")))?;
    mapping_from_json(id, parsed)
}

pub fn save_session(
    home: impl AsRef<Path>,
    id: &str,
    mapping: &SessionMapping,
) -> Result<(), QcError> {
    parse_session_id(id)?;
    let dir = sessions_dir(&home);
    fs::create_dir_all(&dir).map_err(|cause| qc_io("mkdir", &dir, cause))?;
    let mut map = Map::new();
    map.insert("tool".into(), json!(mapping.tool.as_str()));
    map.insert("native_id".into(), json!(mapping.native_id));
    map.insert("cwd".into(), json!(mapping.cwd));
    map.insert("created".into(), json!(mapping.created));
    map.insert("updated".into(), json!(mapping.updated));
    map.insert("warnings".into(), json!(mapping.warnings));
    let body = serde_json::to_string_pretty(&Value::Object(map))
        .map_err(|cause| QcError::new(format!("cannot encode JSON: {cause}")))?;
    let path = session_path(home, id);
    fs::write(&path, format!("{body}\n")).map_err(|cause| qc_io("write", &path, cause))
}

/// Canonical cwd for session matching: path.resolve, then realpath when the path exists.
/// Relative stored cwd resolves against the current process cwd. No ancestor walk.
pub fn cwd_key(cwd: &str) -> String {
    let input = Path::new(cwd);
    let resolved = if input.is_absolute() {
        lexical_normalize(input)
    } else {
        match std::env::current_dir() {
            Ok(base) => lexical_normalize(&base.join(input)),
            Err(_) => lexical_normalize(input),
        }
    };
    match fs::metadata(&resolved) {
        Ok(_) => fs::canonicalize(&resolved)
            .unwrap_or(resolved)
            .to_string_lossy()
            .into_owned(),
        Err(_) => resolved.to_string_lossy().into_owned(),
    }
}

fn newer_session(a: FoundSession, b: FoundSession) -> FoundSession {
    if a.mapping.updated != b.mapping.updated {
        return if a.mapping.updated > b.mapping.updated {
            a
        } else {
            b
        };
    }
    if a.mapping.created != b.mapping.created {
        return if a.mapping.created > b.mapping.created {
            a
        } else {
            b
        };
    }
    if a.id >= b.id { a } else { b }
}

/// Scan ~/.qc/sessions/*.json; skip unreadable/corrupt files; pick newest updated then created then id.
pub fn find_latest_session(
    home: impl AsRef<Path>,
    cwd: &str,
    tool: Option<ToolName>,
) -> Result<FoundSession, QcError> {
    let key = cwd_key(cwd);
    let dir = sessions_dir(&home);
    let names = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Err(QcError::new(match tool {
                Some(name) => format!("no {name} session found for this directory"),
                None => "no session found for this directory".into(),
            }));
        }
        Err(err) => return Err(qc_io("readdir", &dir, err)),
    };

    let mut best: Option<FoundSession> = None;
    for entry in names {
        let Ok(entry) = entry else { continue };
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if !name.ends_with(".json") {
            continue;
        }
        let id = &name[..name.len() - ".json".len()];
        let Ok(mapping) = load_session(&home, id) else {
            continue;
        };
        if cwd_key(&mapping.cwd) != key {
            continue;
        }
        if let Some(wanted) = tool
            && mapping.tool != wanted
        {
            continue;
        }
        let row = FoundSession {
            id: id.to_string(),
            mapping,
        };
        best = Some(match best.take() {
            Some(current) => newer_session(current, row),
            None => row,
        });
    }

    best.ok_or_else(|| {
        QcError::new(match tool {
            Some(name) => format!("no {name} session found for this directory"),
            None => "no session found for this directory".into(),
        })
    })
}
