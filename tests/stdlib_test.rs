mod common;
use common::run_code;
use whalli::value::Value;
use std::sync::Arc;

#[test]
fn test_json_encode_decode() {
    let code = r#"
    import json

    let user = new(map)
    user["name"] = "Alice"
    user["age"] = 30
    let encoded = json.encode(user)

    let (decoded, err) = json.decode(encoded)
    let name_val = decoded["name"]
    let age_val = decoded["age"]

    let (invalid_data, parse_err) = json.decode("invalid json {{{")
    "#;
    let vm = run_code(code);
    let name_val = vm.globals.get("name_val").expect("name_val should exist");
    assert_eq!(name_val, &Value::Str(Arc::new("Alice".to_string())));

    let age_val = vm.globals.get("age_val").expect("age_val should exist");
    assert_eq!(age_val, &Value::Int(30));

    let err_val = vm.globals.get("parse_err").expect("parse_err should exist");
    assert!(matches!(err_val, Value::Str(_)), "parse_err should be string error");
}

#[test]
fn test_fs_error_handling() {
    let code = r#"
    import fs

    let (ok, write_err) = fs.write("/tmp/whalli_test_err_file.txt", "hello error handling")
    let (content, read_err) = fs.read("/tmp/whalli_test_err_file.txt")

    let (missing_content, missing_err) = fs.read("/tmp/definitely_not_existing_file_99999.txt")
    "#;
    let vm = run_code(code);
    let content_val = vm.globals.get("content").expect("content should exist");
    assert_eq!(content_val, &Value::Str(Arc::new("hello error handling".to_string())));

    let read_err = vm.globals.get("read_err").expect("read_err should exist");
    assert_eq!(read_err, &Value::Nil);

    let missing_content = vm.globals.get("missing_content").expect("missing_content should exist");
    assert_eq!(missing_content, &Value::Nil);

    let missing_err = vm.globals.get("missing_err").expect("missing_err should exist");
    assert!(matches!(missing_err, Value::Str(_)));

    std::fs::remove_file("/tmp/whalli_test_err_file.txt").ok();
}

#[test]
fn test_os_module() {
    let code = r#"
    import os

    let dir = os.cwd()
    os.set_env("WHALLI_TEST_KEY", "whalli_123")
    let read_back = os.env("WHALLI_TEST_KEY")
    let non_existent = os.env("DEFINITELY_NOT_SET_12345")
    "#;
    let vm = run_code(code);
    let read_back = vm.globals.get("read_back").expect("read_back should exist");
    assert_eq!(read_back, &Value::Str(Arc::new("whalli_123".to_string())));

    let non_existent = vm.globals.get("non_existent").expect("non_existent should exist");
    assert_eq!(non_existent, &Value::Nil);
}

#[test]
fn test_string_and_list_methods() {
    let code = r#"
    let raw = "  apple,banana,orange  "
    let trimmed = raw.trim()
    let parts = trimmed.split(",")
    let joined = parts.join(" - ")
    let has_banana = parts.contains("banana")

    let text = "hello world"
    let replaced = text.replace("world", "whalli")
    let upper = text.to_upper()
    let starts = text.starts_with("hello")
    let ends = text.ends_with("world")
    "#;
    let vm = run_code(code);

    let trimmed = vm.globals.get("trimmed").expect("trimmed should exist");
    assert_eq!(trimmed, &Value::Str(Arc::new("apple,banana,orange".to_string())));

    let joined = vm.globals.get("joined").expect("joined should exist");
    assert_eq!(joined, &Value::Str(Arc::new("apple - banana - orange".to_string())));

    let has_banana = vm.globals.get("has_banana").expect("has_banana should exist");
    assert_eq!(has_banana, &Value::Bool(true));

    let replaced = vm.globals.get("replaced").expect("replaced should exist");
    assert_eq!(replaced, &Value::Str(Arc::new("hello whalli".to_string())));

    let upper = vm.globals.get("upper").expect("upper should exist");
    assert_eq!(upper, &Value::Str(Arc::new("HELLO WORLD".to_string())));

    let starts = vm.globals.get("starts").expect("starts should exist");
    assert_eq!(starts, &Value::Bool(true));

    let ends = vm.globals.get("ends").expect("ends should exist");
    assert_eq!(ends, &Value::Bool(true));
}
