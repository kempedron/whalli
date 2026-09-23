use crate::compiler::Compiler;
use crate::heap::Obj;
use crate::lexer::Lexer;
use crate::opcode::OpCode;
use crate::parser::Parser;
use crate::value::{NativeResult, Value};
use crate::vm::VM;
use mio::net::TcpListener;
use mio::{Interest, Token};
use std::collections::HashMap;
use std::io::Read;
use std::net::SocketAddr;
use std::sync::atomic::Ordering;
use std::sync::Arc;

pub fn parse_addr_str(addr_val: &Value) -> Result<(String, u16), String> {
    match addr_val {
        Value::Int(p) => Ok(("0.0.0.0".to_string(), *p as u16)),
        Value::Str(s) => {
            let s = s.trim();
            if let Ok(p) = s.parse::<u16>() {
                return Ok(("0.0.0.0".to_string(), p));
            }
            if s.starts_with(':') {
                let port_str = &s[1..];
                let port = port_str
                    .parse::<u16>()
                    .map_err(|e| format!("Invalid port in '{}': {}", s, e))?;
                return Ok(("0.0.0.0".to_string(), port));
            }
            if let Some((host_part, port_part)) = s.rsplit_once(':') {
                let host = if host_part.is_empty() || host_part == "localhost" {
                    "127.0.0.1".to_string()
                } else {
                    host_part.to_string()
                };
                let port = port_part
                    .parse::<u16>()
                    .map_err(|e| format!("Invalid port in '{}': {}", s, e))?;
                return Ok((host, port));
            }
            Err(format!(
                "Invalid address format '{}'. Expected 'host:port', ':port', or port integer",
                s
            ))
        }
        _ => Err("Address must be a string like ':8080' or '127.0.0.1:8080', or port integer".to_string()),
    }
}

pub fn status_text_for_code(code: i64) -> &'static str {
    match code {
        100 => "Continue",
        101 => "Switching Protocols",
        200 => "OK",
        201 => "Created",
        202 => "Accepted",
        204 => "No Content",
        206 => "Partial Content",
        301 => "Moved Permanently",
        302 => "Found",
        304 => "Not Modified",
        307 => "Temporary Redirect",
        308 => "Permanent Redirect",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        408 => "Request Timeout",
        409 => "Conflict",
        410 => "Gone",
        413 => "Payload Too Large",
        415 => "Unsupported Media Type",
        422 => "Unprocessable Entity",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        501 => "Not Implemented",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        504 => "Gateway Timeout",
        _ => "Unknown",
    }
}

fn build_http_response(
    status_code: i64,
    content_type: Option<&str>,
    custom_headers: Option<&HashMap<String, Value>>,
    body: &str,
    heap: &crate::heap::Heap,
) -> String {
    let reason = status_text_for_code(status_code);
    let body_bytes = body.as_bytes();
    let body_len = body_bytes.len();

    // Pre-allocate buffer for high performance
    let mut resp = String::with_capacity(128 + body_len);
    resp.push_str("HTTP/1.1 ");
    resp.push_str(&status_code.to_string());
    resp.push(' ');
    resp.push_str(reason);
    resp.push_str("\r\nContent-Length: ");
    resp.push_str(&body_len.to_string());
    resp.push_str("\r\nConnection: close\r\n");

    let mut has_content_type = false;
    if let Some(headers) = custom_headers {
        for (k, v) in headers {
            let key_lower = k.to_lowercase();
            if key_lower == "content-type" {
                has_content_type = true;
            }
            resp.push_str(k);
            resp.push_str(": ");
            resp.push_str(&v.stringify(heap));
            resp.push_str("\r\n");
        }
    }

    if !has_content_type {
        if let Some(ct) = content_type {
            resp.push_str("Content-Type: ");
            resp.push_str(ct);
            resp.push_str("\r\n");
        }
    }

    resp.push_str("\r\n");
    resp.push_str(body);
    resp
}

fn url_decode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(val) = u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""), 16) {
                result.push(val as char);
                i += 3;
                continue;
            }
        } else if bytes[i] == b'+' {
            result.push(' ');
            i += 1;
            continue;
        }
        result.push(bytes[i] as char);
        i += 1;
    }
    result
}

pub fn parse_http_request_bytes(raw_bytes: &[u8], vm: &mut VM) -> Result<Value, String> {
    let mut headers_buf = [httparse::Header { name: "", value: &[] }; 64];
    let mut parsed_req = httparse::Request::new(&mut headers_buf);

    let status = parsed_req
        .parse(raw_bytes)
        .map_err(|e| format!("HTTP parse error: {:?}", e))?;

    let header_len = match status {
        httparse::Status::Complete(len) => len,
        httparse::Status::Partial => return Err("Incomplete HTTP request".to_string()),
    };

    let method = parsed_req.method.unwrap_or("GET").to_string();
    let raw_path = parsed_req.path.unwrap_or("/").to_string();

    let (path_str, query_str) = match raw_path.split_once('?') {
        Some((p, q)) => (p.to_string(), q.to_string()),
        None => (raw_path.clone(), String::new()),
    };

    // Parse query string into Map
    let mut query_map = HashMap::new();
    if !query_str.is_empty() {
        for pair in query_str.split('&') {
            if let Some((k, v)) = pair.split_once('=') {
                query_map.insert(url_decode(k), Value::Str(Arc::new(url_decode(v))));
            } else if !pair.is_empty() {
                query_map.insert(url_decode(pair), Value::Str(Arc::new(String::new())));
            }
        }
    }
    let query_id = vm.heap.alloc(Obj::Map(query_map));

    // Parse headers into Map (lowercase keys)
    let mut headers_map = HashMap::new();
    for h in parsed_req.headers {
        if !h.name.is_empty() {
            let key_lower = h.name.to_lowercase();
            let val = String::from_utf8_lossy(h.value).to_string();
            headers_map.insert(key_lower, Value::Str(Arc::new(val)));
        }
    }
    let headers_id = vm.heap.alloc(Obj::Map(headers_map));

    let body_bytes = &raw_bytes[header_len..];
    let body_str = String::from_utf8_lossy(body_bytes).to_string();

    let mut req_map = HashMap::new();
    req_map.insert("method".to_string(), Value::Str(Arc::new(method)));
    req_map.insert("url".to_string(), Value::Str(Arc::new(raw_path)));
    req_map.insert("path".to_string(), Value::Str(Arc::new(path_str)));
    req_map.insert("query".to_string(), Value::ObjRef(query_id));
    req_map.insert("headers".to_string(), Value::ObjRef(headers_id));
    req_map.insert("body".to_string(), Value::Str(Arc::new(body_str)));
    req_map.insert("proto".to_string(), Value::Str(Arc::new("HTTP/1.1".to_string())));

    let req_id = vm.heap.alloc(Obj::Map(req_map));
    Ok(Value::ObjRef(req_id))
}

pub fn register(vm: &mut VM) -> Value {
    let mut http_module = HashMap::new();

    // 1. http.listen(addr: str | int) -> (server_id: int, err: str | nil)
    http_module.insert(
        "listen".to_string(),
        Value::Native(|args, vm| {
            if let Some(addr_val) = args.first() {
                let (host, port) = match parse_addr_str(addr_val) {
                    Ok(hp) => hp,
                    Err(e) => {
                        let res = Value::Tuple(Arc::new(vec![Value::Nil, Value::Str(Arc::new(e))]));
                        return NativeResult::Return(res);
                    }
                };

                let addr_str = format!("{}:{}", host, port);
                let addr: SocketAddr = match addr_str.parse() {
                    Ok(a) => a,
                    Err(e) => {
                        let res = Value::Tuple(Arc::new(vec![Value::Nil, Value::Str(Arc::new(e.to_string()))]));
                        return NativeResult::Return(res);
                    }
                };

                let mut listener = match TcpListener::bind(addr) {
                    Ok(l) => l,
                    Err(e) => {
                        let res = Value::Tuple(Arc::new(vec![Value::Nil, Value::Str(Arc::new(e.to_string()))]));
                        return NativeResult::Return(res);
                    }
                };

                let token_id = vm.net.next_token.fetch_add(1, Ordering::Relaxed);
                let token = Token(token_id);

                let poller = vm.net.poll.lock();
                if let Err(e) = poller.registry().register(&mut listener, token, Interest::READABLE) {
                    let res = Value::Tuple(Arc::new(vec![Value::Nil, Value::Str(Arc::new(e.to_string()))]));
                    return NativeResult::Return(res);
                }
                drop(poller);

                vm.net.listeners.write().insert(token_id, listener);
                let res = Value::Tuple(Arc::new(vec![Value::Int(token_id as i64), Value::Nil]));
                return NativeResult::Return(res);
            }
            let res = Value::Tuple(Arc::new(vec![Value::Nil, Value::Str(Arc::new("Expected address string or port".to_string()))]));
            NativeResult::Return(res)
        }),
    );

    // 2. http.read_request(client_id: int) -> (req: map | nil, err: str | nil)
    http_module.insert(
        "read_request".to_string(),
        Value::Native(|args, vm| {
            if let Some(Value::Int(client_id)) = args.first() {
                let client_id = *client_id as usize;

                let mut streams = vm.net.streams.write();
                let stream = match streams.get_mut(&client_id) {
                    Some(s) => s,
                    None => {
                        let res = Value::Tuple(Arc::new(vec![Value::Nil, Value::Str(Arc::new("Invalid client_id".to_string()))]));
                        return NativeResult::Return(res);
                    }
                };

                let mut total_buffer = Vec::new();
                let mut chunk = [0u8; 4096];
                let mut header_len = None;
                let mut expected_total = None;

                loop {
                    match stream.read(&mut chunk) {
                        Ok(0) => {
                            if total_buffer.is_empty() {
                                streams.remove(&client_id);
                                let res = Value::Tuple(Arc::new(vec![Value::Nil, Value::Nil]));
                                return NativeResult::Return(res);
                            }
                            break;
                        }
                        Ok(n) => {
                            total_buffer.extend_from_slice(&chunk[..n]);

                            if header_len.is_none() {
                                if let Some(pos) = total_buffer.windows(4).position(|w| w == b"\r\n\r\n") {
                                    let hlen = pos + 4;
                                    header_len = Some(hlen);

                                    // Check content-length
                                    let mut headers_buf = [httparse::Header { name: "", value: &[] }; 64];
                                    let mut req = httparse::Request::new(&mut headers_buf);
                                    if let Ok(httparse::Status::Complete(_)) = req.parse(&total_buffer) {
                                        for h in req.headers {
                                            if h.name.eq_ignore_ascii_case("content-length") {
                                                if let Ok(s) = std::str::from_utf8(h.value) {
                                                    if let Ok(cl) = s.trim().parse::<usize>() {
                                                        expected_total = Some(hlen + cl);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            if let Some(exp) = expected_total {
                                if total_buffer.len() >= exp {
                                    break;
                                }
                            } else if header_len.is_some() {
                                break;
                            }
                        }
                        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            if total_buffer.is_empty() {
                                return NativeResult::SuspendIO(Token(client_id));
                            } else {
                                break;
                            }
                        }
                        Err(e) => {
                            streams.remove(&client_id);
                            let res = Value::Tuple(Arc::new(vec![Value::Nil, Value::Str(Arc::new(e.to_string()))]));
                            return NativeResult::Return(res);
                        }
                    }
                }
                drop(streams);

                match parse_http_request_bytes(&total_buffer, vm) {
                    Ok(req_val) => {
                        let res = Value::Tuple(Arc::new(vec![req_val, Value::Nil]));
                        NativeResult::Return(res)
                    }
                    Err(e) => {
                        let res = Value::Tuple(Arc::new(vec![Value::Nil, Value::Str(Arc::new(e))]));
                        NativeResult::Return(res)
                    }
                }
            } else {
                let res = Value::Tuple(Arc::new(vec![Value::Nil, Value::Str(Arc::new("Expected integer client_id".to_string()))]));
                NativeResult::Return(res)
            }
        }),
    );

    // 3. http.parse_request(raw_str: str) -> (req: map | nil, err: str | nil)
    http_module.insert(
        "parse_request".to_string(),
        Value::Native(|args, vm| {
            if let Some(Value::Str(raw)) = args.first() {
                match parse_http_request_bytes(raw.as_bytes(), vm) {
                    Ok(req_val) => {
                        let res = Value::Tuple(Arc::new(vec![req_val, Value::Nil]));
                        NativeResult::Return(res)
                    }
                    Err(e) => {
                        let res = Value::Tuple(Arc::new(vec![Value::Nil, Value::Str(Arc::new(e))]));
                        NativeResult::Return(res)
                    }
                }
            } else {
                let res = Value::Tuple(Arc::new(vec![Value::Nil, Value::Str(Arc::new("Expected HTTP raw string".to_string()))]));
                NativeResult::Return(res)
            }
        }),
    );

    // 4. http.status_text(code: int) -> str
    http_module.insert(
        "status_text".to_string(),
        Value::Native(|args, _vm| {
            let code = match args.first() {
                Some(Value::Int(c)) => *c,
                _ => 200,
            };
            let text = status_text_for_code(code);
            NativeResult::Return(Value::Str(Arc::new(text.to_string())))
        }),
    );

    // 5. http.response(status_code: int, headers: map | nil = nil, body: str = "") -> str
    http_module.insert(
        "response".to_string(),
        Value::Native(|args, vm| {
            let status_code = match args.first() {
                Some(Value::Int(c)) => *c,
                _ => 200,
            };

            let custom_headers = if args.len() >= 2 {
                if let Some(Value::ObjRef(h_id)) = args.get(1) {
                    vm.heap.with_read(*h_id, |obj| {
                        if let Obj::Map(m) = obj {
                            Some(m.clone())
                        } else {
                            None
                        }
                    }).ok().flatten()
                } else {
                    None
                }
            } else {
                None
            };

            let body = if args.len() >= 3 {
                match args.get(2) {
                    Some(Value::Str(s)) => s.as_str().to_string(),
                    Some(val) => val.stringify(&vm.heap),
                    None => String::new(),
                }
            } else {
                String::new()
            };

            let resp_str = build_http_response(
                status_code,
                None,
                custom_headers.as_ref(),
                &body,
                &vm.heap,
            );
            NativeResult::Return(Value::Str(Arc::new(resp_str)))
        }),
    );

    // 6. http.json_response(status_code: int, data: any, headers: map | nil = nil) -> str
    http_module.insert(
        "json_response".to_string(),
        Value::Native(|args, vm| {
            let status_code = match args.first() {
                Some(Value::Int(c)) => *c,
                _ => 200,
            };

            let json_body = if args.len() >= 2 {
                let jv = crate::stdlib::json::value_to_json(&args[1], &vm.heap);
                serde_json::to_string(&jv).unwrap_or_else(|_| "null".to_string())
            } else {
                "null".to_string()
            };

            let custom_headers = if args.len() >= 3 {
                if let Some(Value::ObjRef(h_id)) = args.get(2) {
                    vm.heap.with_read(*h_id, |obj| {
                        if let Obj::Map(m) = obj {
                            Some(m.clone())
                        } else {
                            None
                        }
                    }).ok().flatten()
                } else {
                    None
                }
            } else {
                None
            };

            let resp_str = build_http_response(
                status_code,
                Some("application/json"),
                custom_headers.as_ref(),
                &json_body,
                &vm.heap,
            );
            NativeResult::Return(Value::Str(Arc::new(resp_str)))
        }),
    );

    // 7. http.text_response(status_code: int, text: str, headers: map | nil = nil) -> str
    http_module.insert(
        "text_response".to_string(),
        Value::Native(|args, vm| {
            let status_code = match args.first() {
                Some(Value::Int(c)) => *c,
                _ => 200,
            };

            let text_body = if args.len() >= 2 {
                match args.get(1) {
                    Some(Value::Str(s)) => s.as_str().to_string(),
                    Some(val) => val.stringify(&vm.heap),
                    None => String::new(),
                }
            } else {
                String::new()
            };

            let custom_headers = if args.len() >= 3 {
                if let Some(Value::ObjRef(h_id)) = args.get(2) {
                    vm.heap.with_read(*h_id, |obj| {
                        if let Obj::Map(m) = obj {
                            Some(m.clone())
                        } else {
                            None
                        }
                    }).ok().flatten()
                } else {
                    None
                }
            } else {
                None
            };

            let resp_str = build_http_response(
                status_code,
                Some("text/plain; charset=utf-8"),
                custom_headers.as_ref(),
                &text_body,
                &vm.heap,
            );
            NativeResult::Return(Value::Str(Arc::new(resp_str)))
        }),
    );

    // 8. http.html_response(status_code: int, html: str, headers: map | nil = nil) -> str
    http_module.insert(
        "html_response".to_string(),
        Value::Native(|args, vm| {
            let status_code = match args.first() {
                Some(Value::Int(c)) => *c,
                _ => 200,
            };

            let html_body = if args.len() >= 2 {
                match args.get(1) {
                    Some(Value::Str(s)) => s.as_str().to_string(),
                    Some(val) => val.stringify(&vm.heap),
                    None => String::new(),
                }
            } else {
                String::new()
            };

            let custom_headers = if args.len() >= 3 {
                if let Some(Value::ObjRef(h_id)) = args.get(2) {
                    vm.heap.with_read(*h_id, |obj| {
                        if let Obj::Map(m) = obj {
                            Some(m.clone())
                        } else {
                            None
                        }
                    }).ok().flatten()
                } else {
                    None
                }
            } else {
                None
            };

            let resp_str = build_http_response(
                status_code,
                Some("text/html; charset=utf-8"),
                custom_headers.as_ref(),
                &html_body,
                &vm.heap,
            );
            NativeResult::Return(Value::Str(Arc::new(resp_str)))
        }),
    );

    // 9. http.match_route(router_or_routes, method: str, path: str) -> (handler: func | nil, params: map | nil)
    http_module.insert(
        "match_route".to_string(),
        Value::Native(|args, vm| {
            if args.len() >= 3 {
                if let (Value::ObjRef(r_id), Value::Str(method), Value::Str(path)) = (&args[0], &args[1], &args[2]) {
                    let method_upper = method.to_uppercase();
                    let target_path = path.as_str();

                    // Find routes map
                    let matched_result = vm.heap.with_read(*r_id, |obj| {
                        let Obj::Map(map) = obj else { return None };

                        let routes_map_id = match map.get("routes") {
                            Some(Value::ObjRef(routes_id)) => *routes_id,
                            _ => *r_id,
                        };

                        vm.heap.with_read(routes_map_id, |routes_obj| {
                            let Obj::Map(methods_map) = routes_obj else { return None };
                            let method_routes_id = match methods_map.get(&method_upper) {
                                Some(Value::ObjRef(m_id)) => *m_id,
                                _ => return None,
                            };

                            vm.heap.with_read(method_routes_id, |m_obj| {
                                let Obj::Map(path_routes) = m_obj else { return None };

                                // 1. Check exact match
                                if let Some(handler) = path_routes.get(target_path) {
                                    return Some((handler.clone(), None));
                                }

                                // 2. Check parameterized match (/users/:id)
                                let target_segments: Vec<&str> = target_path
                                    .trim_matches('/')
                                    .split('/')
                                    .filter(|s| !s.is_empty())
                                    .collect();

                                for (pattern, handler) in path_routes {
                                    let pat_segments: Vec<&str> = pattern
                                        .trim_matches('/')
                                        .split('/')
                                        .filter(|s| !s.is_empty())
                                        .collect();

                                    if pat_segments.len() != target_segments.len() {
                                        continue;
                                    }

                                    let mut matches = true;
                                    let mut params = HashMap::new();

                                    for (p_seg, t_seg) in pat_segments.iter().zip(target_segments.iter()) {
                                        if let Some(param_name) = p_seg.strip_prefix(':') {
                                            params.insert(param_name.to_string(), Value::Str(Arc::new(t_seg.to_string())));
                                        } else if *p_seg != *t_seg {
                                            matches = false;
                                            break;
                                        }
                                    }

                                    if matches {
                                        return Some((handler.clone(), Some(params)));
                                    }
                                }

                                None
                            }).ok().flatten()
                        }).ok().flatten()
                    }).ok().flatten();

                    if let Some((handler, params_opt)) = matched_result {
                        let params_val = if let Some(params_map) = params_opt {
                            let p_id = vm.heap.alloc(Obj::Map(params_map));
                            Value::ObjRef(p_id)
                        } else {
                            Value::Nil
                        };
                        let res = Value::Tuple(Arc::new(vec![handler, params_val]));
                        return NativeResult::Return(res);
                    }
                }
            }
            let res = Value::Tuple(Arc::new(vec![Value::Nil, Value::Nil]));
            NativeResult::Return(res)
        }),
    );

    // 10. http.router() -> Map
    http_module.insert(
        "router".to_string(),
        Value::Native(|_args, vm| {
            let mut router_map = HashMap::new();
            let mut methods_map = HashMap::new();

            for m in ["GET", "POST", "PUT", "DELETE", "PATCH", "HEAD", "OPTIONS"] {
                let m_id = vm.heap.alloc(Obj::Map(HashMap::new()));
                methods_map.insert(m.to_string(), Value::ObjRef(m_id));
            }

            let routes_id = vm.heap.alloc(Obj::Map(methods_map));
            router_map.insert("routes".to_string(), Value::ObjRef(routes_id));

            let router_id = vm.heap.alloc(Obj::Map(router_map));
            NativeResult::Return(Value::ObjRef(router_id))
        }),
    );

    // Socket primitives forwarded to http for self-contained server execution
    http_module.insert("accept".to_string(), Value::Native(crate::stdlib::net::net_accept));
    http_module.insert("write_all".to_string(), Value::Native(crate::stdlib::net::net_write_all));
    http_module.insert("set_nodelay".to_string(), Value::Native(crate::stdlib::net::net_set_nodelay));
    http_module.insert("close".to_string(), Value::Native(crate::stdlib::net::net_close));

    // Compile built-in Whalli routines for server execution
    let server_code = r#"
    func _whalli_http_serve_client(client_id, handler) {
        http.set_nodelay(client_id, true)
        let (req, r_err) = http.read_request(client_id)
        if r_err == nil and req != nil {
            let resp = nil
            if handler is map {
                let (fn_route, params) = http.match_route(handler, req["method"], req["path"])
                if fn_route != nil {
                    if params != nil {
                        req["params"] = params
                    }
                    resp = fn_route(req)
                } else {
                    resp = http.text_response(404, "404 page not found")
                }
            } else {
                resp = handler(req)
            }
            if resp != nil {
                http.write_all(client_id, resp)
            }
        }
        http.close(client_id)
    }

    func _whalli_http_listen_and_serve(addr, handler) {
        let (server_id, err) = http.listen(addr)
        if err != nil {
            return (false, err)
        }
        if handler is map {
            handler["server_id"] = server_id
        }
        while true {
            let client_id = http.accept(server_id)
            if client_id == nil {
                break
            }
            wo http._serve_client(client_id, handler)
        }
        return (true, nil)
    }
    "#;

    let mut lexer = Lexer::new(server_code);
    let tokens = lexer.tokenize().expect("Lexer error in http server_code");
    let mut parser = Parser::new(tokens);
    let stmts = parser.parse().expect("Parser error in http server_code");
    let compiler = Compiler::new();
    let bytecode = compiler.compile(&stmts);

    for op in &bytecode {
        if let OpCode::Closure(func_obj, _) = op {
            if func_obj.name == "_whalli_http_serve_client" {
                let c_id = vm.heap.alloc(Obj::Closure(func_obj.clone(), vec![]));
                vm.globals.insert("_whalli_http_serve_client".to_string(), Value::ObjRef(c_id));
                http_module.insert("_serve_client".to_string(), Value::ObjRef(c_id));
            } else if func_obj.name == "_whalli_http_listen_and_serve" {
                let c_id = vm.heap.alloc(Obj::Closure(func_obj.clone(), vec![]));
                vm.globals.insert("_whalli_http_listen_and_serve".to_string(), Value::ObjRef(c_id));
                http_module.insert("listen_and_serve".to_string(), Value::ObjRef(c_id));
            }
        }
    }

    let id = vm.heap.alloc(Obj::Map(http_module));
    Value::ObjRef(id)
}
