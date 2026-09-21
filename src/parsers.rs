use chrono::{DateTime, SecondsFormat, TimeZone, Utc};
use serde_json::{Map, Value};

use crate::errors::QcError;

/// Parse JSON with insertion-order maps (`serde_json` `preserve_order`).
pub fn parse_json(source: &str) -> Result<Value, QcError> {
    serde_json::from_str(source).map_err(|cause| QcError::new(format!("invalid JSON: {cause}")))
}

pub fn json_to_string(value: &Value) -> Result<String, QcError> {
    serde_json::to_string(value)
        .map_err(|cause| QcError::new(format!("cannot encode JSON: {cause}")))
}

/// TOML 1.0 table. The `toml` 0.8 parser rejects TOML 1.1-only syntax.
pub fn parse_toml(source: &str) -> Result<toml::Table, QcError> {
    source
        .parse::<toml::Table>()
        .map_err(|cause| QcError::new(format!("invalid TOML: {cause}")))
}

fn yaml_options() -> serde_saphyr::Options {
    // Match npm `yaml` 2.8.1 defaults used by qc frontmatter: YAML 1.2 core
    // booleans, unique keys, no YAML 1.1 merge-key expansion.
    serde_saphyr::options! {
        merge_keys: serde_saphyr::MergeKeyPolicy::AsOrdinary,
        strict_booleans: true,
        duplicate_keys: serde_saphyr::DuplicateKeyPolicy::Error,
    }
}

/// Parse a YAML 1.2 mapping with qc-owned parser options.
pub fn parse_yaml_mapping(source: &str) -> Result<Map<String, Value>, QcError> {
    let value: Value = serde_saphyr::from_str_with_options(source, yaml_options())
        .map_err(|cause| QcError::new(format!("invalid YAML: {cause}")))?;
    match value {
        Value::Object(map) => Ok(map),
        _ => Err(QcError::new("YAML frontmatter must be a mapping")),
    }
}

/// Same options plus a tight alias/event budget for adversarial YAML.
pub fn parse_yaml_mapping_with_budget(
    source: &str,
    max_events: usize,
    max_replayed_events: usize,
) -> Result<Map<String, Value>, QcError> {
    let options = serde_saphyr::options! {
        merge_keys: serde_saphyr::MergeKeyPolicy::AsOrdinary,
        strict_booleans: true,
        duplicate_keys: serde_saphyr::DuplicateKeyPolicy::Error,
        budget: serde_saphyr::budget! {
            max_events: max_events,
        },
        alias_limits: serde_saphyr::alias_limits! {
            max_total_replayed_events: max_replayed_events,
        },
    };
    let value: Value = serde_saphyr::from_str_with_options(source, options)
        .map_err(|cause| QcError::new(format!("invalid YAML: {cause}")))?;
    match value {
        Value::Object(map) => Ok(map),
        _ => Err(QcError::new("YAML frontmatter must be a mapping")),
    }
}

/// JS `Date#toISOString()` shape: UTC, milliseconds, `Z`.
pub fn utc_millis(datetime: DateTime<Utc>) -> String {
    datetime.to_rfc3339_opts(SecondsFormat::Millis, true)
}

/// Local pretty-ID clock fragment `yymmdd-hhmm`.
pub fn pretty_clock_stamp<Tz: TimeZone>(datetime: DateTime<Tz>) -> String
where
    Tz::Offset: std::fmt::Display,
{
    datetime.format("%y%m%d-%H%M").to_string()
}
