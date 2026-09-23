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

#[test]
fn test_http_backend_full_features() {
    let code = r#"
    import http
    import requests
    import time

    let port = 19124
    let router = http.router()

    // 1. Global middleware for auth check on /protected
    func auth_middleware(req) {
        if req["path"].starts_with("/api/v1/protected") {
            let token = req.header("x-auth-token", "")
            if token != "secret-token-123" {
                return http.json_error(401, "Unauthorized: missing valid auth token")
            }
        }
        return nil
    }
    router.use(auth_middleware)

    // 2. Custom not_found handler returning JSON
    func custom_404(req) {
        return http.json_error(404, f"API endpoint {req[\"path\"]} not found")
    }
    router.not_found(custom_404)

    // 3. Handlers
    func handle_protected(req) {
        return http.json_response(200, {"secret": "classified-data"})
    }

    func handle_cookie_echo(req) {
        let sess = req.cookie("session_id", "anonymous")
        return http.json_response(200, {"session": sess})
    }

    // 4. Grouped routes
    let api = router.group("/api")
    let v1 = api.group("/v1")
    v1.get("/protected", handle_protected)
    v1.get("/cookie-check", handle_cookie_echo)

    // 5. Static file serving
    router.static("/public", ".")

    // Start server
    wo http.listen_and_serve(f"127.0.0.1:{port}", router)
    time.sleep(0.08)

    // A. Test unauthorized access to protected route -> 401
    let r_unauth = requests.get(f"http://127.0.0.1:{port}/api/v1/protected", {"timeout": 3})
    let status_unauth = r_unauth.status_code
    let data_unauth = r_unauth.json()
    let unauth_err = data_unauth["error"]

    // B. Test authorized access with header -> 200
    let r_auth = requests.get(f"http://127.0.0.1:{port}/api/v1/protected", {
        "headers": {"X-Auth-Token": "secret-token-123"},
        "timeout": 3
    })
    let status_auth = r_auth.status_code
    let auth_data = r_auth.json()
    let secret = auth_data["secret"]

    // C. Test cookie parsing on server
    let r_cookie = requests.get(f"http://127.0.0.1:{port}/api/v1/cookie-check", {
        "headers": {"Cookie": "session_id=user_sess_777; other=1"},
        "timeout": 3
    })
    let cookie_sess = r_cookie.json()["session"]

    // D. Test static file serving (/public/Cargo.toml)
    let r_static = requests.get(f"http://127.0.0.1:{port}/public/Cargo.toml", {"timeout": 3})
    let static_ok = r_static.ok
    let static_has_pkg = r_static.text.contains("[package]")

    // E. Test 405 Method Not Allowed: POST to /api/v1/protected (only GET registered)
    let r_405 = requests.post(f"http://127.0.0.1:{port}/api/v1/protected", {
        "headers": {"X-Auth-Token": "secret-token-123"},
        "timeout": 3
    })
    let is_405 = r_405.status_code == 405
    let allow_header = r_405.headers["allow"]

    // F. Test custom JSON 404
    let r_custom_404 = requests.get(f"http://127.0.0.1:{port}/missing/path", {"timeout": 3})
    let is_404_code = r_custom_404.status_code == 404
    let not_found_msg = r_custom_404.json()["error"]

    // Graceful shutdown
    http.close(router.server_id)
    "#;

    let vm = run_code(code);

    assert_eq!(vm.globals.get("status_unauth"), Some(&Value::Int(401)));
    assert_eq!(
        vm.globals.get("unauth_err"),
        Some(&Value::Str(Arc::new("Unauthorized: missing valid auth token".to_string())))
    );

    assert_eq!(vm.globals.get("status_auth"), Some(&Value::Int(200)));
    assert_eq!(
        vm.globals.get("secret"),
        Some(&Value::Str(Arc::new("classified-data".to_string())))
    );

    assert_eq!(
        vm.globals.get("cookie_sess"),
        Some(&Value::Str(Arc::new("user_sess_777".to_string())))
    );

    assert_eq!(vm.globals.get("static_ok"), Some(&Value::Bool(true)));
    assert_eq!(vm.globals.get("static_has_pkg"), Some(&Value::Bool(true)));

    assert_eq!(vm.globals.get("is_405"), Some(&Value::Bool(true)));
    let allow_hdr = vm.globals.get("allow_header").unwrap().to_string();
    assert!(allow_hdr.contains("GET"));

    assert_eq!(vm.globals.get("is_404_code"), Some(&Value::Bool(true)));
    assert_eq!(
        vm.globals.get("not_found_msg"),
        Some(&Value::Str(Arc::new("API endpoint /missing/path not found".to_string())))
    );
}
