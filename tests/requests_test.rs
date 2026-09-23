mod common;
use common::run_code;
use whalli::value::Value;
use std::sync::Arc;

#[test]
fn test_requests_error_handling() {
    let code = r#"
    import requests

    // Non-existent port/host should not panic, but return resp with ok == false and error != nil
    let resp = requests.get("http://127.0.0.1:19999", {
        "timeout": 0.5
    })

    let is_ok = resp.ok
    let status = resp.status_code
    let has_error = resp.error != nil
    "#;
    let vm = run_code(code);
    assert_eq!(vm.globals.get("is_ok"), Some(&Value::Bool(false)));
    assert_eq!(vm.globals.get("status"), Some(&Value::Int(0)));
    assert_eq!(vm.globals.get("has_error"), Some(&Value::Bool(true)));
}

#[test]
fn test_requests_with_local_tcp_server() {
    let code = r#"
    import requests
    import net
    import time
    import sync

    let port = 18765
    let (server_id, err) = net.listen(port)

    func server_task(srv) {
        let client = net.accept(srv)
        if client != nil {
            let (req, r_err) = net.read_until(client, "\r\n\r\n")
            let body = "{\"message\": \"hello from mock server\", \"status\": \"success\"}"
            let response = f"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {body.len()}\r\nConnection: close\r\n\r\n{body}"
            net.write_all(client, response)
            net.close(client)
        }
        net.close(srv)
    }

    wo server_task(server_id)
    time.sleep(0.05)

    // Send request via requests module
    let resp = requests.get(f"http://127.0.0.1:{port}/api/test", {
        "headers": {"X-Custom-Header": "whalli-client"},
        "params": {"page": 1, "query": "search"},
        "timeout": 2
    })

    let resp_ok = resp.ok
    let resp_code = resp.status_code
    let resp_text = resp.text
    let data = resp.json()
    let data_msg = data["message"]
    let data_status = data["status"]
    let header_type = resp.headers["content-type"]
    "#;

    let vm = run_code(code);
    assert_eq!(vm.globals.get("resp_ok"), Some(&Value::Bool(true)));
    assert_eq!(vm.globals.get("resp_code"), Some(&Value::Int(200)));
    assert_eq!(
        vm.globals.get("data_msg"),
        Some(&Value::Str(Arc::new("hello from mock server".to_string())))
    );
    assert_eq!(
        vm.globals.get("data_status"),
        Some(&Value::Str(Arc::new("success".to_string())))
    );
    assert_eq!(
        vm.globals.get("header_type"),
        Some(&Value::Str(Arc::new("application/json".to_string())))
    );
}

#[test]
fn test_requests_post_json() {
    let code = r#"
    import requests
    import http
    import net
    import time

    let port = 18766
    let (server_id, err) = net.listen(port)
    let received_body = ""

    func server_task(srv) {
        let client = net.accept(srv)
        if client != nil {
            let (req, r_err) = http.read_request(client)
            if req != nil {
                received_body = req["body"]
            }
            let resp_body = "{\"id\": 42, \"created\": true}"
            let response = http.json_response(201, {"id": 42, "created": true})
            net.write_all(client, response)
            net.close(client)
        }
        net.close(srv)
    }

    wo server_task(server_id)
    time.sleep(0.05)

    let created = requests.post(f"http://127.0.0.1:{port}/api/items", {
        "json": {"title": "Test Item", "count": 10},
        "timeout": 5
    })

    let post_ok = created.ok
    let post_code = created.status_code
    let post_json = created.json()
    let new_id = post_json["id"]
    let is_created = post_json["created"]
    "#;

    let vm = run_code(code);
    assert_eq!(vm.globals.get("post_ok"), Some(&Value::Bool(true)));
    assert_eq!(vm.globals.get("post_code"), Some(&Value::Int(201)));
    let received = vm.globals.get("received_body").unwrap().to_string();
    assert!(received.contains("\"title\":\"Test Item\""));
    assert_eq!(vm.globals.get("new_id"), Some(&Value::Int(42)));
    assert_eq!(vm.globals.get("is_created"), Some(&Value::Bool(true)));
}
