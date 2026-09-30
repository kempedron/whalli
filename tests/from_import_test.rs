mod common;
use common::run_code;
use whalli::value::Value;
use std::fs;

#[test]
fn test_from_import_file() {
    let mod_code = r#"
    pub let pi_val = 3.14
    let private_secret = 42

    pub func add(a, b) {
        return a + b
    }

    func private_fn() {
        return 999
    }
    "#;
    let file_path = "/tmp/whalli_test_math_lib.wh";
    fs::write(file_path, mod_code).unwrap();

    let code = r#"
    from "/tmp/whalli_test_math_lib.wh" import pi_val, add as my_add

    let sum = my_add(10, 20)
    "#;

    let vm = run_code(code);
    assert_eq!(vm.globals.get("pi_val"), Some(&Value::Float(3.14)));
    assert_eq!(vm.globals.get("sum"), Some(&Value::Int(30)));
    assert_eq!(vm.globals.get("my_add").is_some(), true);

    let _ = fs::remove_file(file_path);
}

#[test]
fn test_from_import_stdlib() {
    let code = r#"
    from math import sqrt, pi
    let root = sqrt(16)
    "#;
    let vm = run_code(code);
    assert_eq!(vm.globals.get("root"), Some(&Value::Float(4.0)));
    assert_eq!(vm.globals.get("pi"), Some(&Value::Float(std::f64::consts::PI)));
}
