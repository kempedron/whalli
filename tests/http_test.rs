mod common;
use common::run_code;
use whalli::value::Value;
use std::sync::Arc;

#[test]
fn test_http_status_text() {
    let code = r#"
    import http

    let ok = http.status_text(200)
    let created = http.status_text(201)
    let not_found = http.status_text(404)
    let server_error = http.status_text(500)
    "#;
    let vm = run_code(code);
    assert_eq!(vm.globals.get("ok"), Some(&Value::Str(Arc::new("OK".to_string()))));
    assert_eq!(vm.globals.get("created"), Some(&Value::Str(Arc::new("Created".to_string()))));
    assert_eq!(vm.globals.get("not_found"), Some(&Value::Str(Arc::new("Not Found".to_string()))));
    assert_eq!(vm.globals.get("server_error"), Some(&Value::Str(Arc::new("Internal Server Error".to_string()))));
}

#[test]
fn test_http_response_builders() {
    let code = r#"
    import http

    // Text response
    let text_res = http.text_response(200, "hello world")

    // JSON response
    let json_res = http.json_response(201, {"status": "ok", "items": [1, 2, 3]})

    // HTML response
    let html_res = http.html_response(200, "<h1>Hello</h1>", {"X-Custom": "test"})
    "#;
    let vm = run_code(code);

    let text_res = vm.globals.get("text_res").unwrap().to_string();
    assert!(text_res.starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(text_res.contains("Content-Type: text/plain; charset=utf-8\r\n"));
    assert!(text_res.contains("Content-Length: 11\r\n"));
    assert!(text_res.ends_with("\r\n\r\nhello world"));

    let json_res = vm.globals.get("json_res").unwrap().to_string();
    assert!(json_res.starts_with("HTTP/1.1 201 Created\r\n"));
    assert!(json_res.contains("Content-Type: application/json\r\n"));
    assert!(json_res.contains("\"status\":\"ok\""));

    let html_res = vm.globals.get("html_res").unwrap().to_string();
    assert!(html_res.starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(html_res.contains("Content-Type: text/html; charset=utf-8\r\n"));
    assert!(html_res.contains("X-Custom: test\r\n"));
    assert!(html_res.ends_with("\r\n\r\n<h1>Hello</h1>"));
}

#[test]
fn test_http_parse_request() {
    let code = r#"
    import http

    let raw = "POST /api/v1/users?role=admin&active=true HTTP/1.1\r\nHost: localhost:8080\r\nContent-Type: application/json\r\nContent-Length: 26\r\n\r\n{\"name\": \"Alice\", \"age\": 30}"

    let (req, err) = http.parse_request(raw)
    let parse_ok = err == nil
    let method = req["method"]
    let path = req["path"]
    let query_role = req["query"]["role"]
    let query_active = req["query"]["active"]
    let header_host = req["headers"]["host"]
    let header_ct = req["headers"]["content-type"]
    let body = req["body"]

    // Test req.json() method
    let parsed_json = req.json()
    let user_name = parsed_json["name"]
    let user_age = parsed_json["age"]
    "#;
    let vm = run_code(code);
    assert_eq!(vm.globals.get("parse_ok"), Some(&Value::Bool(true)));
    assert_eq!(vm.globals.get("method"), Some(&Value::Str(Arc::new("POST".to_string()))));
    assert_eq!(vm.globals.get("path"), Some(&Value::Str(Arc::new("/api/v1/users".to_string()))));
    assert_eq!(vm.globals.get("query_role"), Some(&Value::Str(Arc::new("admin".to_string()))));
    assert_eq!(vm.globals.get("query_active"), Some(&Value::Str(Arc::new("true".to_string()))));
    assert_eq!(vm.globals.get("header_host"), Some(&Value::Str(Arc::new("localhost:8080".to_string()))));
    assert_eq!(vm.globals.get("header_ct"), Some(&Value::Str(Arc::new("application/json".to_string()))));
    assert_eq!(vm.globals.get("user_name"), Some(&Value::Str(Arc::new("Alice".to_string()))));
    assert_eq!(vm.globals.get("user_age"), Some(&Value::Int(30)));
}

#[test]
fn test_http_router_matching() {
    let code = r#"
    import http

    let router = http.router()

    func get_users(req) {
        return http.json_response(200, [{"id": 1, "name": "Alice"}])
    }

    func get_user_by_id(req) {
        let user_id = req["params"]["id"]
        return http.json_response(200, {"id": user_id, "name": "Found User"})
    }

    func create_user(req) {
        let body = req.json()
        return http.json_response(201, {"created": true, "name": body["name"]})
    }

    router.get("/users", get_users)
    router.get("/users/:id", get_user_by_id)
    router.post("/users", create_user)

    // Test 1: exact GET /users
    let (h1, p1) = http.match_route(router, "GET", "/users")
    let has_h1 = h1 != nil
    let r1 = h1(new(map))

    // Test 2: parameterized GET /users/42
    let (h2, p2) = http.match_route(router, "GET", "/users/42")
    let has_h2 = h2 != nil
    let param_id = p2["id"]
    let req2 = new(map)
    req2["params"] = p2
    let r2 = h2(req2)

    // Test 3: non-existent route
    let (h3, p3) = http.match_route(router, "GET", "/not-found")
    let has_h3 = h3 != nil
    "#;
    let vm = run_code(code);
    assert_eq!(vm.globals.get("has_h1"), Some(&Value::Bool(true)));
    let r1 = vm.globals.get("r1").unwrap().to_string();
    assert!(r1.contains("\"name\":\"Alice\""));

    assert_eq!(vm.globals.get("has_h2"), Some(&Value::Bool(true)));
    assert_eq!(vm.globals.get("param_id"), Some(&Value::Str(Arc::new("42".to_string()))));
    let r2 = vm.globals.get("r2").unwrap().to_string();
    assert!(r2.contains("\"id\":\"42\""));

    assert_eq!(vm.globals.get("has_h3"), Some(&Value::Bool(false)));
}

#[test]
fn test_http_status_constants_and_helpers() {
    let code = r#"
    import http

    let code_ok = http.STATUS_OK
    let code_created = http.STATUS_CREATED
    let code_not_found = http.STATUS_NOT_FOUND
    let code_method_not_allowed = http.STATUS_METHOD_NOT_ALLOWED
    let code_internal_err = http.STATUS_INTERNAL_SERVER_ERROR

    let redir = http.redirect("https://example.com/login", 303)
    let err_txt = http.error(400, "Invalid payload")
    let err_json = http.json_error(422, "Validation failed")
    let cookie_val = http.set_cookie("session_id", "secret123", {
        "path": "/api",
        "http_only": true,
        "secure": true,
        "max_age": 3600,
        "same_site": "Lax"
    })
    "#;
    let vm = run_code(code);
    assert_eq!(vm.globals.get("code_ok"), Some(&Value::Int(200)));
    assert_eq!(vm.globals.get("code_created"), Some(&Value::Int(201)));
    assert_eq!(vm.globals.get("code_not_found"), Some(&Value::Int(404)));
    assert_eq!(vm.globals.get("code_method_not_allowed"), Some(&Value::Int(405)));
    assert_eq!(vm.globals.get("code_internal_err"), Some(&Value::Int(500)));

    let redir = vm.globals.get("redir").unwrap().to_string();
    assert!(redir.starts_with("HTTP/1.1 303 See Other\r\n"));
    assert!(redir.contains("Location: https://example.com/login\r\n"));

    let err_txt = vm.globals.get("err_txt").unwrap().to_string();
    assert!(err_txt.starts_with("HTTP/1.1 400 Bad Request\r\n"));
    assert!(err_txt.ends_with("\r\n\r\nInvalid payload"));

    let err_json = vm.globals.get("err_json").unwrap().to_string();
    assert!(err_json.starts_with("HTTP/1.1 422 Unprocessable Entity\r\n"));
    assert!(err_json.contains("Content-Type: application/json\r\n"));
    assert!(err_json.contains("\"error\":\"Validation failed\""));
    assert!(err_json.contains("\"code\":422"));

    let cookie_val = vm.globals.get("cookie_val").unwrap().to_string();
    assert_eq!(
        cookie_val,
        "session_id=secret123; Path=/api; Max-Age=3600; HttpOnly; Secure; SameSite=Lax"
    );
}

#[test]
fn test_http_request_advanced_helpers() {
    let code = r#"
    import http

    let raw = "POST /search?q=whalli%20lang&page=2 HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/x-www-form-urlencoded\r\nCookie: session=xyz890; theme=dark\r\nContent-Length: 27\r\n\r\nuser=alice&action=subscribe"

    let (req, err) = http.parse_request(raw)
    let ok = err == nil

    // 1. Header lookup
    let h_ct = req.header("Content-Type")
    let h_missing = req.header("X-Custom", "default-custom")

    // 2. Query lookup
    let q_search = req.query("q")
    let q_page = req.query("page")
    let q_missing = req.query("limit", "25")

    // 3. Cookie lookup
    let c_sess = req.cookie("session")
    let c_theme = req.cookie("theme")
    let c_missing = req.cookie("lang", "en")

    // 4. Form urlencoded lookup
    let f_user = req.form_value("user")
    let f_action = req.form_value("action")
    let f_missing = req.form_value("token", "none")

    // 5. Remote addr exists
    let remote = req["remote_addr"]
    "#;
    let vm = run_code(code);
    assert_eq!(vm.globals.get("ok"), Some(&Value::Bool(true)));
    assert_eq!(
        vm.globals.get("h_ct"),
        Some(&Value::Str(Arc::new("application/x-www-form-urlencoded".to_string())))
    );
    assert_eq!(
        vm.globals.get("h_missing"),
        Some(&Value::Str(Arc::new("default-custom".to_string())))
    );

    assert_eq!(
        vm.globals.get("q_search"),
        Some(&Value::Str(Arc::new("whalli lang".to_string())))
    );
    assert_eq!(
        vm.globals.get("q_page"),
        Some(&Value::Str(Arc::new("2".to_string())))
    );
    assert_eq!(
        vm.globals.get("q_missing"),
        Some(&Value::Str(Arc::new("25".to_string())))
    );

    assert_eq!(
        vm.globals.get("c_sess"),
        Some(&Value::Str(Arc::new("xyz890".to_string())))
    );
    assert_eq!(
        vm.globals.get("c_theme"),
        Some(&Value::Str(Arc::new("dark".to_string())))
    );
    assert_eq!(
        vm.globals.get("c_missing"),
        Some(&Value::Str(Arc::new("en".to_string())))
    );

    assert_eq!(
        vm.globals.get("f_user"),
        Some(&Value::Str(Arc::new("alice".to_string())))
    );
    assert_eq!(
        vm.globals.get("f_action"),
        Some(&Value::Str(Arc::new("subscribe".to_string())))
    );
    assert_eq!(
        vm.globals.get("f_missing"),
        Some(&Value::Str(Arc::new("none".to_string())))
    );
}

#[test]
fn test_http_router_wildcards_groups_and_allowed_methods() {
    let code = r#"
    import http

    let router = http.router()

    func handle_static(req) {
        return http.text_response(200, "STATIC FILE")
    }

    func handle_users(req) {
        return http.text_response(200, "USERS LIST")
    }

    func handle_create_user(req) {
        return http.text_response(201, "USER CREATED")
    }

    // 1. Wildcard routes
    router.get("/static/*filepath", handle_static)

    // 2. Grouping
    let api = router.group("/api")
    let v1 = api.group("/v1")
    v1.get("/users", handle_users)
    v1.post("/users", handle_create_user)

    // Test wildcard match
    let (h_wild, p_wild) = http.match_route(router, "GET", "/static/images/logo.png")
    let wild_matched = h_wild != nil
    let wild_path = p_wild["filepath"]

    // Test group route
    let (h_grp, p_grp) = http.match_route(router, "GET", "/api/v1/users")
    let grp_matched = h_grp != nil

    // Test allowed methods for /api/v1/users (GET, POST, HEAD)
    let allowed = http.allowed_methods(router, "/api/v1/users")
    let has_get = allowed.contains("GET")
    let has_post = allowed.contains("POST")
    let has_head = allowed.contains("HEAD")
    let has_delete = allowed.contains("DELETE")
    "#;
    let vm = run_code(code);
    assert_eq!(vm.globals.get("wild_matched"), Some(&Value::Bool(true)));
    assert_eq!(
        vm.globals.get("wild_path"),
        Some(&Value::Str(Arc::new("images/logo.png".to_string())))
    );
    assert_eq!(vm.globals.get("grp_matched"), Some(&Value::Bool(true)));

    assert_eq!(vm.globals.get("has_get"), Some(&Value::Bool(true)));
    assert_eq!(vm.globals.get("has_post"), Some(&Value::Bool(true)));
    assert_eq!(vm.globals.get("has_head"), Some(&Value::Bool(true)));
    assert_eq!(vm.globals.get("has_delete"), Some(&Value::Bool(false)));
}

#[test]
fn test_http_static_file_response_and_security() {
    let code = r#"
    import http

    // 1. Valid existing file
    let resp_valid = http.file_response("Cargo.toml")

    // 2. Path traversal attack attempt with '..'
    let resp_traversal = http.file_response("tests", "../Cargo.toml")

    // 3. Non-existent file
    let resp_not_found = http.file_response("non_existent_dir_12345.txt")
    "#;
    let vm = run_code(code);

    let resp_valid = vm.globals.get("resp_valid").unwrap().to_string();
    assert!(resp_valid.starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(resp_valid.contains("Content-Type: text/plain; charset=utf-8\r\n"));
    assert!(resp_valid.contains("[package]"));

    let resp_traversal = vm.globals.get("resp_traversal").unwrap().to_string();
    assert!(resp_traversal.starts_with("HTTP/1.1 403 Forbidden\r\n"));

    let resp_not_found = vm.globals.get("resp_not_found").unwrap().to_string();
    assert!(resp_not_found.starts_with("HTTP/1.1 404 Not Found\r\n"));
}

#[test]
fn test_http_builtin_middlewares_options_and_headers() {
    let code = r#"
    import http

    // 1. CORS middleware
    let cors_mw = http.cors({
        "origin": "https://frontend.example.com",
        "methods": "GET, POST",
        "credentials": true
    })

    // Test Preflight OPTIONS call
    let req_opt = new(map)
    req_opt["method"] = "OPTIONS"
    let opt_resp = http._run_mw_step(cors_mw, req_opt)

    // Test normal GET request with CORS
    let req_get = new(map)
    req_get["method"] = "GET"
    req_get["res_headers"] = new(map)
    let get_resp = http._run_mw_step(cors_mw, req_get)
    let cors_origin = req_get["res_headers"]["Access-Control-Allow-Origin"]
    let cors_cred = req_get["res_headers"]["Access-Control-Allow-Credentials"]

    // 2. Secure headers middleware
    let sec_mw = http.secure_headers()
    let req_sec = new(map)
    req_sec["res_headers"] = new(map)
    let sec_resp = http._run_mw_step(sec_mw, req_sec)
    let x_frame = req_sec["res_headers"]["X-Frame-Options"]
    let x_type = req_sec["res_headers"]["X-Content-Type-Options"]
    "#;

    let vm = run_code(code);

    let opt_resp = vm.globals.get("opt_resp").unwrap().to_string();
    assert!(opt_resp.starts_with("HTTP/1.1 204 No Content\r\n"));
    assert!(opt_resp.contains("Access-Control-Allow-Origin: https://frontend.example.com\r\n"));
    assert!(opt_resp.contains("Access-Control-Allow-Methods: GET, POST\r\n"));
    assert!(opt_resp.contains("Access-Control-Allow-Credentials: true\r\n"));

    assert_eq!(vm.globals.get("get_resp"), Some(&Value::Nil));
    assert_eq!(
        vm.globals.get("cors_origin"),
        Some(&Value::Str(Arc::new("https://frontend.example.com".to_string())))
    );
    assert_eq!(
        vm.globals.get("cors_cred"),
        Some(&Value::Str(Arc::new("true".to_string())))
    );

    assert_eq!(
        vm.globals.get("x_frame"),
        Some(&Value::Str(Arc::new("DENY".to_string())))
    );
    assert_eq!(
        vm.globals.get("x_type"),
        Some(&Value::Str(Arc::new("nosniff".to_string())))
    );
}
