mod common;
use common::run_code;
use whalli::value::Value;

#[test]
fn test_websocket_upgrade_handshake_and_error() {
    let port = 9388;
    let code = format!(r#"
    import http
    import time
    import requests

    let router = http.router()

    func ws_handler(req) {{
        let (ws, err) = http.upgrade(req)
        if err != nil {{
            return http.text_response(400, err)
        }}
        return nil
    }}

    router.get("/ws", ws_handler)

    wo http.listen_and_serve("127.0.0.1:{}", router)

    time.sleep(0.08)

    // 1. Missing Sec-WebSocket-Key should be rejected with 400
    let bad_req = requests.get("http://127.0.0.1:{}/ws", {{"timeout": 3}})
    let bad_status = bad_req.status_code
    let bad_text = bad_req.text

    http.close(router.server_id)
    "#, port, port);

    let vm = run_code(&code);
    assert_eq!(vm.globals.get("bad_status"), Some(&Value::Int(400)));
    let bad_text = vm.globals.get("bad_text").unwrap().to_string();
    assert!(bad_text.contains("Missing Sec-WebSocket-Key header"));
}
