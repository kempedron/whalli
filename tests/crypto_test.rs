mod common;
use common::run_code;
use whalli::value::Value;
use std::sync::Arc;

#[test]
fn test_crypto_hashing() {
    let code = r#"
    import crypto

    let h_sha256 = crypto.sha256("hello world")
    let h_sha1 = crypto.sha1("hello world")
    let h_md5 = crypto.md5("hello world")
    "#;
    let vm = run_code(code);
    assert_eq!(
        vm.globals.get("h_sha256"),
        Some(&Value::Str(Arc::new(
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9".to_string()
        )))
    );
    assert_eq!(
        vm.globals.get("h_sha1"),
        Some(&Value::Str(Arc::new(
            "2aae6c35c94fcfb415dbe95f408b9ce91ee846ed".to_string()
        )))
    );
    assert_eq!(
        vm.globals.get("h_md5"),
        Some(&Value::Str(Arc::new(
            "5eb63bbbe01eeed093cb22bb8f5acdc3".to_string()
        )))
    );
}

#[test]
fn test_crypto_hmac() {
    let code = r#"
    import crypto

    let mac = crypto.hmac_sha256("secret_key", "important payload")
    "#;
    let vm = run_code(code);
    let mac = vm.globals.get("mac").unwrap().to_string();
    assert_eq!(mac.len(), 64);
}

#[test]
fn test_crypto_base64_and_hex() {
    let code = r#"
    import crypto

    let b64 = crypto.base64_encode("Whalli Lang 2026")
    let (decoded_bytes, _) = crypto.base64_decode(b64)
    let (decoded_str, _) = decoded_bytes.decode()

    let hx = crypto.hex_encode(b"test123")
    let (hx_bytes, _) = crypto.hex_decode(hx)
    let (hx_str, _) = hx_bytes.decode()
    "#;
    let vm = run_code(code);
    assert_eq!(
        vm.globals.get("decoded_str"),
        Some(&Value::Str(Arc::new("Whalli Lang 2026".to_string())))
    );
    assert_eq!(
        vm.globals.get("hx_str"),
        Some(&Value::Str(Arc::new("test123".to_string())))
    );
}

#[test]
fn test_crypto_random_and_uuid() {
    let code = r#"
    import crypto

    let r_bytes = crypto.random_bytes(16)
    let r_hex = crypto.random_hex(16)
    let u = crypto.uuid4()
    let is_bytes = r_bytes is bytes
    let hex_len = r_hex.len()
    let u_len = u.len()
    "#;
    let vm = run_code(code);
    assert_eq!(vm.globals.get("is_bytes"), Some(&Value::Bool(true)));
    assert_eq!(vm.globals.get("hex_len"), Some(&Value::Int(32)));
    assert_eq!(vm.globals.get("u_len"), Some(&Value::Int(36)));
    let uuid_str = vm.globals.get("u").unwrap().to_string();
    assert_eq!(uuid_str.chars().filter(|c| *c == '-').count(), 4);
}
