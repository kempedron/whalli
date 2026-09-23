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
