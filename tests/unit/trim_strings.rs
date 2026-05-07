use kolas::app::http::middleware::trim_strings::{trim_form_pairs, trim_query_string, trim_value};
use serde_json::json;

#[test]
fn trims_top_level_strings() {
    let mut v = json!({ "name": "  john  " });
    trim_value(&mut v, None);
    assert_eq!(v["name"], "john");
}

#[test]
fn trims_nested_objects_and_arrays() {
    let mut v = json!({
        "user": { "nick": "  bob  " },
        "tags": ["  a  ", "  b  "]
    });
    trim_value(&mut v, None);
    assert_eq!(v["user"]["nick"], "bob");
    assert_eq!(v["tags"][0], "a");
    assert_eq!(v["tags"][1], "b");
}

#[test]
fn skips_password_field() {
    let mut v = json!({ "password": "  pwd  " });
    trim_value(&mut v, None);
    assert_eq!(v["password"], "  pwd  ");
}

#[test]
fn skips_password_confirmation_field() {
    let mut v = json!({ "password_confirmation": "  x  " });
    trim_value(&mut v, None);
    assert_eq!(v["password_confirmation"], "  x  ");
}

#[test]
fn non_string_values_unchanged() {
    let mut v = json!({
        "n": 42,
        "b": true,
        "z": null
    });
    trim_value(&mut v, None);
    assert_eq!(v["n"], 42);
    assert_eq!(v["b"], true);
    assert_eq!(v["z"], serde_json::Value::Null);
}

#[test]
fn trims_form_pairs() {
    let mut pairs = vec![("name".into(), "  john  ".into())];
    trim_form_pairs(&mut pairs);
    assert_eq!(pairs[0].1, "john");
}

#[test]
fn skips_password_pair() {
    let mut pairs = vec![("password".into(), "  s  ".into())];
    trim_form_pairs(&mut pairs);
    assert_eq!(pairs[0].1, "  s  ");
}

#[test]
fn trims_query_values() {
    let out = trim_query_string("q=%20%20hello%20%20&lang=en");
    assert!(out.contains("q=hello"));
    assert!(out.contains("lang=en"));
}

#[test]
fn preserves_password_in_query() {
    let raw = "password=%20%20s%20%20";
    let out = trim_query_string(raw);
    let pairs: Vec<(String, String)> =
        serde_urlencoded::from_str(&out).expect("parse trimmed query");
    assert_eq!(pairs.len(), 1);
    assert_eq!(pairs[0].0, "password");
    assert_eq!(pairs[0].1, "  s  ");
}

#[test]
fn empty_query_returns_empty_string() {
    assert_eq!(trim_query_string(""), "");
}
