mod common;
use common::run_code;
use whalli::value::Value;
use std::sync::Arc;

#[test]
fn test_net_tcp_echo_server_and_client() {
    let code = r#"
    import net
    import time
    import sync

    let wg = sync.WaitGroup()
    let server_port = 19876

    let (server_id, err) = net.listen(server_port)
    let server_ok = err == nil

    let server_received = ""

    func server_worker(srv_id, wg) {
        let client_conn = net.accept(srv_id)
        if client_conn != nil {
            let (data, r_err) = net.read_until(client_conn, "\n")
            server_received = data
            net.write_all(client_conn, f"ECHO: {data}")
            net.close(client_conn)
        }
        net.close(srv_id)
        wg.done()
    }

    wg.add(1)
    wo server_worker(server_id, wg)

    // Give server worker brief time to start listen/accept
    time.sleep(0.05)

    // Client connection
    let (client, c_err) = net.connect("127.0.0.1", server_port)
    let client_connected = c_err == nil

    let remote_ip = net.peer_addr(client)
    let has_remote_ip = remote_ip != nil

    let nodelay_set = net.set_nodelay(client, true)

    net.write_all(client, "hello whalli\n")
    let (response, resp_err) = net.read_until(client, "\n")
    net.close(client)

    wg.wait()
    "#;
    let vm = run_code(code);
    assert_eq!(vm.globals.get("server_ok"), Some(&Value::Bool(true)));
    assert_eq!(vm.globals.get("client_connected"), Some(&Value::Bool(true)));
    assert_eq!(vm.globals.get("has_remote_ip"), Some(&Value::Bool(true)));
    assert_eq!(vm.globals.get("nodelay_set"), Some(&Value::Bool(true)));
    assert_eq!(
        vm.globals.get("server_received"),
        Some(&Value::Str(Arc::new("hello whalli\n".to_string())))
    );
    assert_eq!(
        vm.globals.get("response"),
        Some(&Value::Str(Arc::new("ECHO: hello whalli\n".to_string())))
    );
}
