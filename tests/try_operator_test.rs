mod common;
use common::run_code;
use whalli::value::Value;
use std::sync::Arc;

#[test]
fn test_try_operator_unwrap_success() {
    let test_file = "/tmp/whalli_try_test_ok.txt";
    std::fs::write(test_file, "  hello world  ").unwrap();

    let code = r#"
    import fs

    func read_trimmed(path: str) {
        let content = fs.read(path)?
        return content.trim()
    }

    let result = read_trimmed("/tmp/whalli_try_test_ok.txt")
    "#;

    let vm = run_code(code);
    let result_val = vm.globals.get("result").expect("result should exist");
    assert_eq!(result_val, &Value::Str(Arc::new("hello world".to_string())));

    std::fs::remove_file(test_file).ok();
}

#[test]
fn test_try_operator_propagate_error() {
    let code = r#"
    import fs

    func load_missing() {
        let content = fs.read("/tmp/definitely_missing_file_xyz_9999.txt")?
        // Following lines must NOT execute
        return "unreachable"
    }

    let (val, err) = load_missing()
    "#;

    let vm = run_code(code);
    let val = vm.globals.get("val").expect("val should exist");
    assert_eq!(val, &Value::Nil);

    let err = vm.globals.get("err").expect("err should exist");
    assert!(matches!(err, Value::Str(_)), "err should contain error message");
}

#[test]
fn test_try_operator_chained() {
    let test_file = "/tmp/whalli_try_test_json.txt";
    std::fs::write(test_file, "{\"name\": \"Whalli\", \"version\": 1}").unwrap();

    let code = r#"
    import fs
    import json

    func load_config(path: str) {
        let raw = fs.read(path)?
        let (config, err) = json.decode(raw)
        return config["name"]
    }

    let app_name = load_config("/tmp/whalli_try_test_json.txt")
    "#;

    let vm = run_code(code);
    let name_val = vm.globals.get("app_name").expect("app_name should exist");
    assert_eq!(name_val, &Value::Str(Arc::new("Whalli".to_string())));

    std::fs::remove_file(test_file).ok();
}
