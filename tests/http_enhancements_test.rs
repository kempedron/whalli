mod common;
use common::run_code;
use whalli::value::Value;
use std::sync::Arc;

#[test]
fn test_req_context_set_get() {
    let code = r#"
    import http

    let raw = "GET /profile HTTP/1.1\r\nHost: localhost\r\n\r\n"
    let (req, _) = http.parse_request(raw)

    // Store state in context
    req.set("user_id", 42)
    req.set("role", "admin")

    let u_id = req.get("user_id")
    let r = req.get("role")
    let def = req.get("missing", "guest")
    let state_dict = req["state"]
    let state_user = state_dict["user_id"]
    "#;
    let vm = run_code(code);
    assert_eq!(vm.globals.get("u_id"), Some(&Value::Int(42)));
    assert_eq!(vm.globals.get("r"), Some(&Value::Str(Arc::new("admin".to_string()))));
    assert_eq!(vm.globals.get("def"), Some(&Value::Str(Arc::new("guest".to_string()))));
    assert_eq!(vm.globals.get("state_user"), Some(&Value::Int(42)));
}

#[test]
fn test_payload_too_large_rejection() {
    let code = r#"
    import http

    // Large Content-Length header exceeding 10MB
    let raw = "POST /upload HTTP/1.1\r\nContent-Length: 20000000\r\n\r\n"
    let (req, err) = http.parse_request(raw)
    let is_rejected = err != nil
    "#;
    let vm = run_code(code);
    assert_eq!(vm.globals.get("is_rejected"), Some(&Value::Bool(true)));
}

#[test]
fn test_router_stop_method() {
    let code = r#"
    import http

    let router = http.router()
    router["server_id"] = 123
    router["running"] = true

    let stopped = router.stop()
    let is_running = router["running"]
    "#;
    let vm = run_code(code);
    assert_eq!(vm.globals.get("stopped"), Some(&Value::Bool(true)));
    assert_eq!(vm.globals.get("is_running"), Some(&Value::Bool(false)));
}
