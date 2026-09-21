use std::fs;

use chrono::{FixedOffset, TimeZone, Utc};
use serde_json::{Map, Value, json};

use crate::common::crate_root;
use quickcall::parsers::{
    json_to_string, parse_json, parse_toml, parse_yaml_mapping, parse_yaml_mapping_with_budget,
    pretty_clock_stamp, utc_millis,
};

fn fixture(rel: &str) -> String {
    fs::read_to_string(crate_root().join("tests/fixtures").join(rel)).expect(rel)
}

#[test]
fn json_preserves_insertion_order() {
    let mut map = Map::new();
    map.insert("tool".into(), json!("pi"));
    map.insert("session_id".into(), json!("id"));
    map.insert("native_id".into(), json!("n"));
    let encoded = json_to_string(&Value::Object(map)).expect("encode");
    assert_eq!(
        encoded,
        r#"{"tool":"pi","session_id":"id","native_id":"n"}"#
    );

    let mut reversed = Map::new();
    reversed.insert("z".into(), json!(1));
    reversed.insert("a".into(), json!(2));
    assert_eq!(
        json_to_string(&Value::Object(reversed)).expect("encode"),
        r#"{"z":1,"a":2}"#
    );
}

#[test]
fn json_round_trip_session_shape() {
    let source = r#"{
  "tool": "pi",
  "native_id": "provider-native-id",
  "cwd": "/absolute/example",
  "created": "2026-09-10T03:00:00.000Z",
  "updated": "2026-09-10T03:00:00.000Z",
  "warnings": []
}"#;
    let value = parse_json(source).expect("json");
    assert_eq!(value["tool"], "pi");
    assert_eq!(value["warnings"], json!([]));
}

#[test]
fn toml_parses_existing_global_and_project_fixtures() {
    let global = parse_toml(&fixture("config/global.toml")).expect("global");
    assert_eq!(global["shell"].as_str(), Some("/bin/sh"));
    assert_eq!(global["tool"]["default"].as_str(), Some("pi"));
    assert_eq!(
        global["tool"]["pi"]["default_model"].as_str(),
        Some("global/model")
    );

    let project = parse_toml(&fixture("config/project.toml")).expect("project");
    assert_eq!(
        project["tool"]["pi"]["default_thinking"].as_str(),
        Some("high")
    );
}

#[test]
fn toml_parses_schema_invalid_fixture_as_syntax_ok() {
    // invalid.toml is schema-invalid (`default-model`), not TOML-invalid.
    let parsed = parse_toml(&fixture("config/invalid.toml")).expect("syntax ok");
    assert_eq!(parsed["default-model"].as_integer(), Some(4));
}

#[test]
fn toml_rejects_syntax_errors_and_toml_1_1_only_forms() {
    let syntax = parse_toml("this is not = toml [").expect_err("syntax");
    assert!(syntax.message.contains("invalid TOML"));

    // @iarna/toml 2.2.5 also accepts trailing commas; keep that acceptance.
    let trailing = parse_toml("a = [1, 2,]\n").expect("trailing comma matches Node toml");
    assert_eq!(trailing["a"].as_array().map(Vec::len), Some(2));

    let inline_newline =
        parse_toml("a = { b = 1,\n c = 2 }\n").expect_err("newline in inline table is TOML 1.1");
    assert!(inline_newline.message.contains("invalid TOML"));

    let hex_escape = parse_toml("a = \"\\x41\"\n").expect_err("\\\\xHH is TOML 1.1");
    assert!(hex_escape.message.contains("invalid TOML"));
}

#[test]
fn yaml_parses_frontmatter_fixture_as_mapping() {
    let source = fixture("prompts/frontmatter.md");
    let yaml = source
        .strip_prefix("---\n")
        .and_then(|rest| rest.split_once("\n---\n"))
        .expect("frontmatter fences")
        .0;
    let map = parse_yaml_mapping(yaml).expect("yaml");
    assert_eq!(map["description"], "Test prompt");
    assert_eq!(map["qc_model"], "prompt/model");
    assert_eq!(map["qc_thinking"], "high");
    assert_eq!(map["qc_no_skills"], true);
    assert_eq!(map["qc_skill_path"], "~/.config/custom-skills");
}

#[test]
fn yaml_1_2_keeps_yes_as_string_and_true_as_bool() {
    let yes = parse_yaml_mapping("qc_no_skills: yes\n").expect("yes");
    assert_eq!(yes["qc_no_skills"], "yes");
    let flag = parse_yaml_mapping("qc_no_skills: true\n").expect("true");
    assert_eq!(flag["qc_no_skills"], true);
}

#[test]
fn yaml_duplicate_keys_error() {
    let err = parse_yaml_mapping("a: 1\na: 2\n").expect_err("duplicate");
    assert!(
        err.message.contains("invalid YAML") || err.message.to_lowercase().contains("duplicate")
    );
}

#[test]
fn yaml_aliases_expand_and_merge_keys_stay_ordinary() {
    let aliased = parse_yaml_mapping("a: &x hello\nb: *x\n").expect("alias");
    assert_eq!(aliased["a"], "hello");
    assert_eq!(aliased["b"], "hello");

    let merged = parse_yaml_mapping("defaults: &d\n  a: 1\nmerged:\n  <<: *d\n  b: 2\n")
        .expect("merge as ordinary");
    let merged_map = merged["merged"].as_object().expect("merged mapping");
    assert!(
        merged_map.contains_key("<<"),
        "npm yaml 2.8.1 defaults do not expand merge keys; got {merged_map:?}"
    );
    assert!(!merged_map.contains_key("a"));
    assert_eq!(merged_map["b"], 2);
}

#[test]
fn yaml_budget_rejects_alias_bomb() {
    let bomb = "a: &a [n, *a]\nb: *a\n";
    let err = parse_yaml_mapping_with_budget(bomb, 20, 8).expect_err("budget");
    assert!(
        err.message.contains("invalid YAML")
            || err.message.to_lowercase().contains("budget")
            || err.message.to_lowercase().contains("alias")
            || err.message.to_lowercase().contains("limit")
    );
}

#[test]
fn utc_millis_matches_js_to_iso_string() {
    let dt = Utc.with_ymd_and_hms(2026, 9, 10, 3, 0, 0).unwrap();
    assert_eq!(utc_millis(dt), "2026-09-10T03:00:00.000Z");
}

#[test]
fn pretty_clock_uses_local_offset() {
    let utc = Utc.with_ymd_and_hms(2026, 9, 10, 3, 0, 0).unwrap();
    let seoul = FixedOffset::east_opt(9 * 3600).expect("kst");
    assert_eq!(pretty_clock_stamp(utc.with_timezone(&seoul)), "260910-1200");
    let edt = FixedOffset::west_opt(4 * 3600).expect("edt");
    assert_eq!(pretty_clock_stamp(utc.with_timezone(&edt)), "260909-2300");
    let est = FixedOffset::west_opt(5 * 3600).expect("est");
    assert_eq!(pretty_clock_stamp(utc.with_timezone(&est)), "260909-2200");
}
