mod common;
use common::run_code;
use whalli::value::Value;
use std::sync::Arc;

#[test]
fn test_bytes_literal_and_operations() {
    let code = r#"
    let b = b"hello\x20world\x00"
    let l = b.len()
    let first = b[0]
    let hex_val = b.hex()
    let (decoded, err) = b.decode()
    "#;
    let vm = run_code(code);

    assert_eq!(vm.globals.get("l"), Some(&Value::Int(12)));
    assert_eq!(vm.globals.get("first"), Some(&Value::Int(104))); // 'h' = 104
    assert_eq!(vm.globals.get("hex_val"), Some(&Value::Str(Arc::new("68656c6c6f20776f726c6400".to_string()))));
    assert_eq!(vm.globals.get("decoded"), Some(&Value::Str(Arc::new("hello world\0".to_string()))));
    assert_eq!(vm.globals.get("err"), Some(&Value::Nil));
}

#[test]
fn test_bytes_concatenation_and_slice() {
    let code = r#"
    let b1 = b"foo"
    let b2 = b"bar"
    let b3 = b1 + b2
    let sub = b3.slice(2, 5)
    let (s, _) = sub.decode()
    let is_bytes = b3 is bytes
    "#;
    let vm = run_code(code);

    assert_eq!(vm.globals.get("b3"), Some(&Value::Bytes(Arc::new(b"foobar".to_vec()))));
    assert_eq!(vm.globals.get("sub"), Some(&Value::Bytes(Arc::new(b"oba".to_vec()))));
    assert_eq!(vm.globals.get("s"), Some(&Value::Str(Arc::new("oba".to_string()))));
    assert_eq!(vm.globals.get("is_bytes"), Some(&Value::Bool(true)));
}

#[test]
fn test_bytes_pattern_matching() {
    let code = r#"
    let packet = b"PING"
    let res = match packet {
        b"PING" => "PONG",
        b"PONG" => "PING",
        _ => "UNKNOWN",
    }
    "#;
    let vm = run_code(code);

    assert_eq!(vm.globals.get("res"), Some(&Value::Str(Arc::new("PONG".to_string()))));
}
