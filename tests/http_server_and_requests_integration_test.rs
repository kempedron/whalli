mod common;
use common::run_code;
use whalli::value::Value;
use std::sync::Arc;

#[test]
fn test_http_listen_and_serve_with_router_and_requests() {
    let code = r#"
    import http
    import requests
    import time

    let port = 19123
    let router = http.router()

    func handle_health(req) {
        return http.text_response(200, "SYSTEM HEALTHY")
    }

    func handle_user(req) {
        let user_id = req["params"]["id"]
        return http.json_response(200, {
            "id": user_id,
            "username": f"user_{user_id}",
            "active": true
        })
    }

    func handle_echo(req) {
        let body = req.json()
        let agent = req["headers"]["user-agent"]
        return http.json_response(201, {
            "received": body,
            "agent": agent
        })
    }

    router.get("/api/health", handle_health)
    router.get("/api/users/:id", handle_user)
    router.post("/api/echo", handle_echo)

    // Launch server in background woroutine
    wo http.listen_and_serve(f"127.0.0.1:{port}", router)

    time.sleep(0.08)

    // 1. GET /api/health
    let r_health = requests.get(f"http://127.0.0.1:{port}/api/health", {"timeout": 3})
    let health_ok = r_health.ok
    let health_status = r_health.status_code
    let health_text = r_health.text

    // 2. GET /api/users/99 (with route parameter extraction)
    let r_user = requests.get(f"http://127.0.0.1:{port}/api/users/99", {"timeout": 3})
    let user_ok = r_user.ok
    let user_status = r_user.status_code
    let user_data = r_user.json()
    let user_id = user_data["id"]
    let username = user_data["username"]

    // 3. POST /api/echo (with JSON request and response)
    let r_echo = requests.post(f"http://127.0.0.1:{port}/api/echo", {
        "headers": {"User-Agent": "whalli-custom-agent"},
        "json": {"msg": "hello backend", "num": 123},
        "timeout": 3
    })
    let echo_ok = r_echo.ok
    let echo_status = r_echo.status_code
    let echo_data = r_echo.json()
    let echo_agent = echo_data["agent"]
    let echo_msg = echo_data["received"]["msg"]
    let echo_num = echo_data["received"]["num"]

    // 4. GET /nonexistent (404 Not Found)
    let r_404 = requests.get(f"http://127.0.0.1:{port}/nonexistent", {"timeout": 3})
    let is_404 = r_404.status_code == 404

    // Gracefully shutdown the server listener
    http.close(router.server_id)
    "#;

    let vm = run_code(code);

    // 1. Health check
    assert_eq!(vm.globals.get("health_ok"), Some(&Value::Bool(true)));
    assert_eq!(vm.globals.get("health_status"), Some(&Value::Int(200)));
    assert_eq!(
        vm.globals.get("health_text"),
        Some(&Value::Str(Arc::new("SYSTEM HEALTHY".to_string())))
    );

    // 2. Param matching
    assert_eq!(vm.globals.get("user_ok"), Some(&Value::Bool(true)));
    assert_eq!(vm.globals.get("user_status"), Some(&Value::Int(200)));
    assert_eq!(
        vm.globals.get("user_id"),
        Some(&Value::Str(Arc::new("99".to_string())))
    );
    assert_eq!(
        vm.globals.get("username"),
        Some(&Value::Str(Arc::new("user_99".to_string())))
    );

    // 3. POST JSON echo
    assert_eq!(vm.globals.get("echo_ok"), Some(&Value::Bool(true)));
    assert_eq!(vm.globals.get("echo_status"), Some(&Value::Int(201)));
    assert_eq!(
        vm.globals.get("echo_agent"),
        Some(&Value::Str(Arc::new("whalli-custom-agent".to_string())))
    );
    assert_eq!(
        vm.globals.get("echo_msg"),
        Some(&Value::Str(Arc::new("hello backend".to_string())))
    );
    assert_eq!(vm.globals.get("echo_num"), Some(&Value::Int(123)));

    // 4. 404
    assert_eq!(vm.globals.get("is_404"), Some(&Value::Bool(true)));
}
