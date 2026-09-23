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
        300 => "Multiple Choices",
        301 => "Moved Permanently",
        302 => "Found",
        303 => "See Other",
        304 => "Not Modified",
        305 => "Use Proxy",
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
    resp.push_str("\r\n");
    let mut has_content_type = false;
    let mut has_connection = false;
    if let Some(headers) = custom_headers {
        for (k, v) in headers {
            let key_lower = k.to_lowercase();
            if key_lower == "content-type" {
                has_content_type = true;
            }
            if key_lower == "connection" {
                has_connection = true;
            }
            resp.push_str(k);
            resp.push_str(": ");
            resp.push_str(&v.stringify(heap));
            resp.push_str("\r\n");
        }
    }

    if !has_connection {
        resp.push_str("Connection: keep-alive\r\nKeep-Alive: timeout=5, max=100\r\n");
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

pub fn url_decode(s: &str) -> String {
    let mut bytes_out = Vec::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(val) = u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""), 16) {
                bytes_out.push(val);
                i += 3;
                continue;
            }
        } else if bytes[i] == b'+' {
            bytes_out.push(b' ');
            i += 1;
            continue;
        }
        bytes_out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(bytes_out).unwrap_or_else(|e| String::from_utf8_lossy(&e.into_bytes()).into_owned())
}

pub fn parse_query_pairs(query_str: &str) -> HashMap<String, Value> {
    let mut map = HashMap::new();
    if !query_str.is_empty() {
        for pair in query_str.split('&') {
            if let Some((k, v)) = pair.split_once('=') {
                map.insert(url_decode(k), Value::Str(Arc::new(url_decode(v))));
            } else if !pair.is_empty() {
                map.insert(url_decode(pair), Value::Str(Arc::new(String::new())));
            }
        }
    }
    map
}

pub fn mime_type_for_path(path: &std::path::Path) -> &'static str {
    match path.extension().and_then(|ext| ext.to_str()).map(|s| s.to_ascii_lowercase()).as_deref() {
        Some("html") | Some("htm") => "text/html; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("js") | Some("mjs") => "application/javascript; charset=utf-8",
        Some("json") => "application/json",
        Some("txt") | Some("toml") | Some("yaml") | Some("yml") | Some("md") | Some("csv") => "text/plain; charset=utf-8",
        Some("xml") => "application/xml",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("ico") => "image/x-icon",
        Some("webp") => "image/webp",
        Some("wasm") => "application/wasm",
        Some("pdf") => "application/pdf",
        Some("zip") => "application/zip",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("ttf") => "font/ttf",
        _ => "application/octet-stream",
    }
}

pub fn serve_static_file(
    base_dir: &str,
    rel_path: &str,
    custom_headers: Option<&HashMap<String, Value>>,
    heap: &crate::heap::Heap,
) -> String {
    let clean_rel = rel_path.trim_start_matches('/');
    if clean_rel.contains("..") {
        return build_http_response(
            403,
            Some("text/plain; charset=utf-8"),
            custom_headers,
            "403 Forbidden: Invalid file path",
            heap,
        );
    }

    let mut target = std::path::PathBuf::from(base_dir);
    if !clean_rel.is_empty() {
        target.push(clean_rel);
    }

    if target.is_dir() {
        let index_path = target.join("index.html");
        if index_path.is_file() {
            target = index_path;
        }
    }

    if !target.is_file() {
        return build_http_response(
            404,
            Some("text/plain; charset=utf-8"),
            custom_headers,
            "404 Not Found",
            heap,
        );
    }

    match std::fs::read(&target) {
        Ok(bytes) => {
            let mime = mime_type_for_path(&target);
            let body_str = String::from_utf8(bytes.clone())
                .unwrap_or_else(|_| String::from_utf8_lossy(&bytes).into_owned());
            build_http_response(200, Some(mime), custom_headers, &body_str, heap)
        }
        Err(e) => {
            let err_msg = format!("500 Internal Server Error: {}", e);
            build_http_response(
                500,
                Some("text/plain; charset=utf-8"),
                custom_headers,
                &err_msg,
                heap,
            )
        }
    }
}

pub fn parse_http_request_bytes(raw_bytes: &[u8], vm: &mut VM, remote_addr: &str) -> Result<Value, String> {
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
    let query_map = parse_query_pairs(&query_str);
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

    // Parse cookies from Cookie header
    let mut cookies_map = HashMap::new();
    if let Some(Value::Str(cookie_header)) = headers_map.get("cookie") {
        for part in cookie_header.split(';') {
            let part = part.trim();
            if let Some((k, v)) = part.split_once('=') {
                let k = k.trim();
                let mut v = v.trim();
                if v.starts_with('"') && v.ends_with('"') && v.len() >= 2 {
                    v = &v[1..v.len() - 1];
                }
                cookies_map.insert(k.to_string(), Value::Str(Arc::new(url_decode(v))));
            }
        }
    }
    let cookies_id = vm.heap.alloc(Obj::Map(cookies_map));

    let body_bytes = &raw_bytes[header_len..];
    let body_str = String::from_utf8_lossy(body_bytes).to_string();

    // Parse form if application/x-www-form-urlencoded
    let form_map = if headers_map
        .get("content-type")
        .and_then(|v| if let Value::Str(s) = v { Some(s.as_str()) } else { None })
        .map_or(false, |ct| ct.to_lowercase().contains("application/x-www-form-urlencoded"))
    {
        parse_query_pairs(&body_str)
    } else {
        HashMap::new()
    };
    let form_id = vm.heap.alloc(Obj::Map(form_map));
    let res_headers_id = vm.heap.alloc(Obj::Map(HashMap::new()));

    let headers_id = vm.heap.alloc(Obj::Map(headers_map));

    let mut req_map = HashMap::new();
    req_map.insert("method".to_string(), Value::Str(Arc::new(method)));
    req_map.insert("url".to_string(), Value::Str(Arc::new(raw_path)));
    req_map.insert("path".to_string(), Value::Str(Arc::new(path_str)));
    req_map.insert("query".to_string(), Value::ObjRef(query_id));
    req_map.insert("headers".to_string(), Value::ObjRef(headers_id));
    req_map.insert("cookies".to_string(), Value::ObjRef(cookies_id));
    req_map.insert("form".to_string(), Value::ObjRef(form_id));
    req_map.insert("res_headers".to_string(), Value::ObjRef(res_headers_id));
    req_map.insert("body".to_string(), Value::Str(Arc::new(body_str)));
    req_map.insert("proto".to_string(), Value::Str(Arc::new("HTTP/1.1".to_string())));
    req_map.insert("remote_addr".to_string(), Value::Str(Arc::new(remote_addr.to_string())));

    let req_id = vm.heap.alloc(Obj::Map(req_map));
    Ok(Value::ObjRef(req_id))
}

pub fn match_path_in_routes<'a>(
    path_routes: &'a HashMap<String, Value>,
    target_path: &str,
) -> Option<(&'a Value, String, HashMap<String, Value>)> {
    // 1. Exact match
    if let Some(handler) = path_routes.get(target_path) {
        return Some((handler, target_path.to_string(), HashMap::new()));
    }
    // Check with or without trailing slash
    if target_path.len() > 1 {
        if target_path.ends_with('/') {
            let without = target_path.trim_end_matches('/');
            if let Some(handler) = path_routes.get(without) {
                return Some((handler, without.to_string(), HashMap::new()));
            }
        } else {
            let with_slash = format!("{}/", target_path);
            if let Some(handler) = path_routes.get(&with_slash) {
                return Some((handler, with_slash, HashMap::new()));
            }
        }
    }

    let target_segments: Vec<&str> = target_path
        .trim_matches('/')
        .split('/')
        .filter(|s| !s.is_empty())
        .collect();

    // Check parameterized matches (exact length first, no wildcards)
    for (pattern, handler) in path_routes {
        let pat_segments: Vec<&str> = pattern
            .trim_matches('/')
            .split('/')
            .filter(|s| !s.is_empty())
            .collect();

        let has_wildcard = pat_segments.last().map_or(false, |s| s.starts_with('*'));
        if has_wildcard {
            continue;
        }

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
            return Some((handler, pattern.clone(), params));
        }
    }

    // Check wildcard matches (*filepath)
    for (pattern, handler) in path_routes {
        let pat_segments: Vec<&str> = pattern
            .trim_matches('/')
            .split('/')
            .filter(|s| !s.is_empty())
            .collect();

        let has_wildcard = pat_segments.last().map_or(false, |s| s.starts_with('*'));
        if !has_wildcard {
            continue;
        }

        let prefix_len = pat_segments.len() - 1;
        if target_segments.len() < prefix_len {
            continue;
        }

        let mut matches = true;
        let mut params = HashMap::new();

        for i in 0..prefix_len {
            let p_seg = pat_segments[i];
            let t_seg = target_segments[i];
            if let Some(param_name) = p_seg.strip_prefix(':') {
                params.insert(param_name.to_string(), Value::Str(Arc::new(t_seg.to_string())));
            } else if p_seg != t_seg {
                matches = false;
                break;
            }
        }

        if matches {
            let wildcard_seg = pat_segments[prefix_len];
            let param_name = if wildcard_seg.len() > 1 {
                &wildcard_seg[1..]
            } else {
                "filepath"
            };
            let remainder = if target_segments.len() > prefix_len {
                target_segments[prefix_len..].join("/")
            } else {
                String::new()
            };
            params.insert(param_name.to_string(), Value::Str(Arc::new(remainder)));
            return Some((handler, pattern.clone(), params));
        }
    }

    None
}

pub fn check_allowed_methods(
    router_id: usize,
    target_path: &str,
    heap: &crate::heap::Heap,
) -> Vec<String> {
    let mut allowed = Vec::new();
    let methods = ["GET", "POST", "PUT", "DELETE", "PATCH", "HEAD", "OPTIONS"];

    heap.with_read(router_id, |obj| {
        let Obj::Map(map) = obj else { return };
        let routes_map_id = match map.get("routes") {
            Some(Value::ObjRef(r_id)) => *r_id,
            _ => router_id,
        };

        heap.with_read(routes_map_id, |routes_obj| {
            let Obj::Map(methods_map) = routes_obj else { return };
            for &m in &methods {
                if let Some(Value::ObjRef(m_id)) = methods_map.get(m) {
                    heap.with_read(*m_id, |m_obj| {
                        if let Obj::Map(path_routes) = m_obj {
                            if match_path_in_routes(path_routes, target_path).is_some() {
                                allowed.push(m.to_string());
                            }
                        }
                    }).ok();
                }
            }
        }).ok();
    }).ok();

    if allowed.contains(&"GET".to_string()) && !allowed.contains(&"HEAD".to_string()) {
        allowed.push("HEAD".to_string());
    }

    allowed
}

pub fn register(vm: &mut VM) -> Value {
    let mut http_module = HashMap::new();

    //  http.listen(addr: str | int) -> (server_id: int, err: str | nil)
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

    //  http.read_request(client_id: int) -> (req: map | nil, err: str | nil)
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
                let remote_addr = streams
                    .get(&client_id)
                    .and_then(|s| s.peer_addr().ok())
                    .map(|a| a.to_string())
                    .unwrap_or_default();
                drop(streams);

                match parse_http_request_bytes(&total_buffer, vm, &remote_addr) {
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

    // http.parse_request(raw_str: str) -> (req: map | nil, err: str | nil)
    http_module.insert(
        "parse_request".to_string(),
        Value::Native(|args, vm| {
            if let Some(Value::Str(raw)) = args.first() {
                match parse_http_request_bytes(raw.as_bytes(), vm, "127.0.0.1:0") {
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

    // http.response(status_code: int, headers: map | nil = nil, body: str = "") -> str
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

    // http.json_response(status_code: int, data: any, headers: map | nil = nil) -> str
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

    // http.text_response(status_code: int, text: str, headers: map | nil = nil) -> str
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

    // http.html_response(status_code: int, html: str, headers: map | nil = nil) -> str
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

    // http.match_route(router_or_routes, method: str, path: str) -> (handler: func | nil, params: map | nil)
    http_module.insert(
        "match_route".to_string(),
        Value::Native(|args, vm| {
            if args.len() >= 3 {
                if let (Value::ObjRef(r_id), Value::Str(method), Value::Str(path)) = (&args[0], &args[1], &args[2]) {
                    let method_upper = method.to_uppercase();
                    let target_path = path.as_str();

                    let matched_result = vm.heap.with_read(*r_id, |obj| {
                        let Obj::Map(map) = obj else { return None };

                        let routes_map_id = match map.get("routes") {
                            Some(Value::ObjRef(routes_id)) => *routes_id,
                            _ => *r_id,
                        };

                        let route_middlewares_map_id = match map.get("route_middlewares") {
                            Some(Value::ObjRef(rm_id)) => Some(*rm_id),
                            _ => None,
                        };

                        vm.heap.with_read(routes_map_id, |routes_obj| {
                            let Obj::Map(methods_map) = routes_obj else { return None };
                            let method_routes_id = match methods_map.get(&method_upper) {
                                Some(Value::ObjRef(m_id)) => Some(*m_id),
                                _ => {
                                    if method_upper == "HEAD" {
                                        match methods_map.get("GET") {
                                            Some(Value::ObjRef(g_id)) => Some(*g_id),
                                            _ => None,
                                        }
                                    } else {
                                        None
                                    }
                                }
                            };

                            let method_routes_id = method_routes_id?;

                            vm.heap.with_read(method_routes_id, |m_obj| {
                                let Obj::Map(path_routes) = m_obj else { return None };

                                if let Some((handler, pat_matched, mut params)) = match_path_in_routes(path_routes, target_path) {
                                    if let Some(rm_id) = route_middlewares_map_id {
                                        let _ = vm.heap.with_read(rm_id, |rm_obj| {
                                            if let Obj::Map(rm_map) = rm_obj {
                                                if let Some(mws_val) = rm_map.get(&pat_matched) {
                                                    params.insert("_middlewares".to_string(), mws_val.clone());
                                                }
                                            }
                                        });
                                    }
                                    return Some((handler.clone(), if params.is_empty() { None } else { Some(params) }));
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

    // http.router() -> Map
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

            let mws_id = vm.heap.alloc(Obj::List(Vec::new()));
            router_map.insert("middlewares".to_string(), Value::ObjRef(mws_id));

            let rmws_id = vm.heap.alloc(Obj::Map(HashMap::new()));
            router_map.insert("route_middlewares".to_string(), Value::ObjRef(rmws_id));

            let router_id = vm.heap.alloc(Obj::Map(router_map));
            NativeResult::Return(Value::ObjRef(router_id))
        }),
    );

    // http.file_response(base_dir: str, rel_path: str = "", headers: map | nil = nil) -> str
    http_module.insert(
        "file_response".to_string(),
        Value::Native(|args, vm| {
            if args.is_empty() {
                let resp = build_http_response(400, Some("text/plain; charset=utf-8"), None, "400 Bad Request: Missing file path", &vm.heap);
                return NativeResult::Return(Value::Str(Arc::new(resp)));
            }
            let base_dir = match &args[0] {
                Value::Str(s) => s.as_str().to_string(),
                _ => {
                    let resp = build_http_response(400, Some("text/plain; charset=utf-8"), None, "400 Bad Request: Path must be string", &vm.heap);
                    return NativeResult::Return(Value::Str(Arc::new(resp)));
                }
            };
            let (rel_path, headers_map) = if args.len() == 2 {
                if let Value::Str(ref s) = args[1] {
                    (s.as_str().to_string(), None)
                } else if let Value::ObjRef(h_id) = args[1] {
                    let hm = vm.heap.with_read(h_id, |obj| {
                        if let Obj::Map(m) = obj { Some(m.clone()) } else { None }
                    }).ok().flatten();
                    (String::new(), hm)
                } else {
                    (String::new(), None)
                }
            } else if args.len() >= 3 {
                let r = match &args[1] {
                    Value::Str(s) => s.as_str().to_string(),
                    _ => String::new(),
                };
                let hm = if let Value::ObjRef(h_id) = args[2] {
                    vm.heap.with_read(h_id, |obj| {
                        if let Obj::Map(m) = obj { Some(m.clone()) } else { None }
                    }).ok().flatten()
                } else {
                    None
                };
                (r, hm)
            } else {
                (String::new(), None)
            };

            let resp_str = serve_static_file(&base_dir, &rel_path, headers_map.as_ref(), &vm.heap);
            NativeResult::Return(Value::Str(Arc::new(resp_str)))
        }),
    );

    // http.redirect(url: str, code: int = 302, headers: map | nil = nil) -> str
    http_module.insert(
        "redirect".to_string(),
        Value::Native(|args, vm| {
            let url = match args.first() {
                Some(Value::Str(u)) => u.as_str().to_string(),
                _ => "/".to_string(),
            };
            let status_code = match args.get(1) {
                Some(Value::Int(c)) => *c,
                _ => 302,
            };
            let mut custom_headers = HashMap::new();
            custom_headers.insert("Location".to_string(), Value::Str(Arc::new(url.clone())));
            if args.len() >= 3 {
                if let Some(Value::ObjRef(h_id)) = args.get(2) {
                    vm.heap.with_read(*h_id, |obj| {
                        if let Obj::Map(m) = obj {
                            for (k, v) in m {
                                custom_headers.insert(k.clone(), v.clone());
                            }
                        }
                    }).ok();
                }
            }
            let body = format!("Redirecting to {}", url);
            let resp_str = build_http_response(
                status_code,
                Some("text/plain; charset=utf-8"),
                Some(&custom_headers),
                &body,
                &vm.heap,
            );
            NativeResult::Return(Value::Str(Arc::new(resp_str)))
        }),
    );

    // http.error(code: int = 500, message: str = "") -> str
    http_module.insert(
        "error".to_string(),
        Value::Native(|args, vm| {
            let code = match args.first() {
                Some(Value::Int(c)) => *c,
                _ => 500,
            };
            let msg = match args.get(1) {
                Some(Value::Str(s)) => s.as_str().to_string(),
                Some(v) => v.stringify(&vm.heap),
                None => status_text_for_code(code).to_string(),
            };
            let resp = build_http_response(code, Some("text/plain; charset=utf-8"), None, &msg, &vm.heap);
            NativeResult::Return(Value::Str(Arc::new(resp)))
        }),
    );

    // http.json_error(code: int = 500, message: str = "") -> str
    http_module.insert(
        "json_error".to_string(),
        Value::Native(|args, vm| {
            let code = match args.first() {
                Some(Value::Int(c)) => *c,
                _ => 500,
            };
            let msg = match args.get(1) {
                Some(Value::Str(s)) => s.as_str().to_string(),
                Some(v) => v.stringify(&vm.heap),
                None => status_text_for_code(code).to_string(),
            };
            let mut err_obj = serde_json::Map::new();
            err_obj.insert("error".to_string(), serde_json::Value::String(msg));
            err_obj.insert("code".to_string(), serde_json::Value::Number(code.into()));
            let json_body = serde_json::to_string(&serde_json::Value::Object(err_obj)).unwrap_or_else(|_| "{}".to_string());
            let resp = build_http_response(code, Some("application/json"), None, &json_body, &vm.heap);
            NativeResult::Return(Value::Str(Arc::new(resp)))
        }),
    );

    // http.set_cookie(name: str, value: str, opts: map | nil = nil) -> str
    http_module.insert(
        "set_cookie".to_string(),
        Value::Native(|args, vm| {
            let name = match args.first() {
                Some(Value::Str(n)) => n.as_str().to_string(),
                _ => String::new(),
            };
            let value = match args.get(1) {
                Some(Value::Str(v)) => v.as_str().to_string(),
                Some(v) => v.stringify(&vm.heap),
                None => String::new(),
            };
            let mut cookie = format!("{}={}", name, value);
            if let Some(Value::ObjRef(opt_id)) = args.get(2) {
                vm.heap.with_read(*opt_id, |obj| {
                    if let Obj::Map(m) = obj {
                        if let Some(Value::Str(path)) = m.get("path") {
                            cookie.push_str("; Path=");
                            cookie.push_str(path);
                        } else if !cookie.contains("; Path=") {
                            cookie.push_str("; Path=/");
                        }
                        if let Some(Value::Str(domain)) = m.get("domain") {
                            cookie.push_str("; Domain=");
                            cookie.push_str(domain);
                        }
                        if let Some(Value::Int(max_age)) = m.get("max_age") {
                            cookie.push_str(&format!("; Max-Age={}", max_age));
                        }
                        if let Some(Value::Str(expires)) = m.get("expires") {
                            cookie.push_str("; Expires=");
                            cookie.push_str(expires);
                        }
                        if let Some(Value::Bool(true)) = m.get("http_only") {
                            cookie.push_str("; HttpOnly");
                        }
                        if let Some(Value::Bool(true)) = m.get("secure") {
                            cookie.push_str("; Secure");
                        }
                        if let Some(Value::Str(same_site)) = m.get("same_site") {
                            cookie.push_str("; SameSite=");
                            cookie.push_str(same_site);
                        }
                    }
                }).ok();
            } else {
                cookie.push_str("; Path=/");
            }
            NativeResult::Return(Value::Str(Arc::new(cookie)))
        }),
    );

    // http.allowed_methods(router, path: str) -> list[str]
    http_module.insert(
        "allowed_methods".to_string(),
        Value::Native(|args, vm| {
            if args.len() >= 2 {
                if let (Value::ObjRef(r_id), Value::Str(path)) = (&args[0], &args[1]) {
                    let allowed = check_allowed_methods(*r_id, path.as_str(), &vm.heap);
                    let list_items: Vec<Value> = allowed.into_iter().map(|m| Value::Str(Arc::new(m))).collect();
                    let list_id = vm.heap.alloc(Obj::List(list_items));
                    return NativeResult::Return(Value::ObjRef(list_id));
                }
            }
            let empty_id = vm.heap.alloc(Obj::List(Vec::new()));
            NativeResult::Return(Value::ObjRef(empty_id))
        }),
    );

    // 17. http.merge_headers(resp_str: str, headers_map: map) -> str
    http_module.insert(
        "merge_headers".to_string(),
        Value::Native(|args, vm| {
            if args.is_empty() {
                return NativeResult::Return(Value::Str(Arc::new(String::new())));
            }
            let resp_str = match &args[0] {
                Value::Str(s) => s.as_str(),
                _ => return NativeResult::Return(args[0].clone()),
            };

            let headers = if args.len() >= 2 {
                if let Value::ObjRef(h_id) = &args[1] {
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

            let Some(extra_map) = headers else {
                return NativeResult::Return(Value::Str(Arc::new(resp_str.to_string())));
            };

            if extra_map.is_empty() {
                return NativeResult::Return(Value::Str(Arc::new(resp_str.to_string())));
            }

            let Some((head, body)) = resp_str.split_once("\r\n\r\n") else {
                return NativeResult::Return(Value::Str(Arc::new(resp_str.to_string())));
            };

            let mut new_resp = String::with_capacity(resp_str.len() + 128);
            new_resp.push_str(head);
            new_resp.push_str("\r\n");

            let head_lower = head.to_lowercase();
            for (k, v) in &extra_map {
                let check_key = format!("\r\n{}:", k.to_lowercase());
                if !head_lower.contains(&check_key) && !head_lower.starts_with(&format!("{}:", k.to_lowercase())) {
                    new_resp.push_str(k);
                    new_resp.push_str(": ");
                    new_resp.push_str(&v.stringify(&vm.heap));
                    new_resp.push_str("\r\n");
                }
            }

            new_resp.push_str("\r\n");
            new_resp.push_str(body);
            NativeResult::Return(Value::Str(Arc::new(new_resp)))
        }),
    );

    // 18. http.cors(options: map | nil = nil) -> func(req)
    http_module.insert(
        "cors".to_string(),
        Value::Native(|args, vm| {
            let mut opts_map = HashMap::new();
            opts_map.insert("origin".to_string(), Value::Str(Arc::new("*".to_string())));
            opts_map.insert("methods".to_string(), Value::Str(Arc::new("GET, POST, PUT, DELETE, PATCH, HEAD, OPTIONS".to_string())));
            opts_map.insert("headers".to_string(), Value::Str(Arc::new("Content-Type, Authorization, X-Requested-With, X-Request-ID".to_string())));
            opts_map.insert("max_age".to_string(), Value::Str(Arc::new("86400".to_string())));
            opts_map.insert("credentials".to_string(), Value::Bool(false));

            if let Some(Value::ObjRef(opt_id)) = args.first() {
                vm.heap.with_read(*opt_id, |obj| {
                    if let Obj::Map(m) = obj {
                        if let Some(Value::Str(o)) = m.get("origin").or_else(|| m.get("origins")) {
                            opts_map.insert("origin".to_string(), Value::Str(Arc::new(o.as_str().to_string())));
                        }
                        if let Some(Value::Str(met)) = m.get("methods") {
                            opts_map.insert("methods".to_string(), Value::Str(Arc::new(met.as_str().to_string())));
                        }
                        if let Some(Value::Str(h)) = m.get("headers") {
                            opts_map.insert("headers".to_string(), Value::Str(Arc::new(h.as_str().to_string())));
                        }
                        if let Some(Value::Int(age)) = m.get("max_age") {
                            opts_map.insert("max_age".to_string(), Value::Str(Arc::new(age.to_string())));
                        }
                        if let Some(Value::Bool(c)) = m.get("credentials") {
                            opts_map.insert("credentials".to_string(), Value::Bool(*c));
                        }
                    }
                }).ok();
            }

            let opts_id = vm.heap.alloc(Obj::Map(opts_map));
            let mut mw_map = HashMap::new();
            mw_map.insert("__is_cors_mw".to_string(), Value::Bool(true));
            mw_map.insert("options".to_string(), Value::ObjRef(opts_id));
            let mw_id = vm.heap.alloc(Obj::Map(mw_map));
            NativeResult::Return(Value::ObjRef(mw_id))
        }),
    );

    // 19. http.logger() -> func(req)
    http_module.insert(
        "logger".to_string(),
        Value::Native(|_args, vm| {
            let mut mw_map = HashMap::new();
            mw_map.insert("__is_logger_mw".to_string(), Value::Bool(true));
            let mw_id = vm.heap.alloc(Obj::Map(mw_map));
            NativeResult::Return(Value::ObjRef(mw_id))
        }),
    );

    // 20. http.secure_headers() -> func(req)
    http_module.insert(
        "secure_headers".to_string(),
        Value::Native(|_args, vm| {
            let mut mw_map = HashMap::new();
            mw_map.insert("__is_secure_headers_mw".to_string(), Value::Bool(true));
            let mw_id = vm.heap.alloc(Obj::Map(mw_map));
            NativeResult::Return(Value::ObjRef(mw_id))
        }),
    );

    // 21. http.request_id(header_name: str = "X-Request-ID") -> func(req)
    http_module.insert(
        "request_id".to_string(),
        Value::Native(|args, vm| {
            let header_key = match args.first() {
                Some(Value::Str(s)) => s.as_str().to_string(),
                _ => "X-Request-ID".to_string(),
            };
            let mut mw_map = HashMap::new();
            mw_map.insert("__is_request_id_mw".to_string(), Value::Bool(true));
            mw_map.insert("header_name".to_string(), Value::Str(Arc::new(header_key)));
            mw_map.insert("next_id".to_string(), Value::Int(1));
            let mw_id = vm.heap.alloc(Obj::Map(mw_map));
            NativeResult::Return(Value::ObjRef(mw_id))
        }),
    );

    // 22. http.rate_limiter(max_requests: int = 60, window_secs: int = 60) -> func(req)
    http_module.insert(
        "rate_limiter".to_string(),
        Value::Native(|args, vm| {
            let max_requests = match args.first() {
                Some(Value::Int(n)) => *n,
                _ => 60,
            };
            let window_secs = match args.get(1) {
                Some(Value::Int(n)) => *n,
                Some(Value::Float(f)) => *f as i64,
                _ => 60,
            };

            let mut mw_map = HashMap::new();
            mw_map.insert("__is_rate_limiter_mw".to_string(), Value::Bool(true));
            mw_map.insert("max_requests".to_string(), Value::Int(max_requests));
            mw_map.insert("window_secs".to_string(), Value::Int(window_secs));
            mw_map.insert("history".to_string(), Value::ObjRef(vm.heap.alloc(Obj::Map(HashMap::new()))));
            let mw_id = vm.heap.alloc(Obj::Map(mw_map));
            NativeResult::Return(Value::ObjRef(mw_id))
        }),
    );

    // Status Constants
    http_module.insert("STATUS_OK".to_string(), Value::Int(200));
    http_module.insert("STATUS_CREATED".to_string(), Value::Int(201));
    http_module.insert("STATUS_ACCEPTED".to_string(), Value::Int(202));
    http_module.insert("STATUS_NO_CONTENT".to_string(), Value::Int(204));
    http_module.insert("STATUS_MOVED_PERMANENTLY".to_string(), Value::Int(301));
    http_module.insert("STATUS_FOUND".to_string(), Value::Int(302));
    http_module.insert("STATUS_NOT_MODIFIED".to_string(), Value::Int(304));
    http_module.insert("STATUS_TEMPORARY_REDIRECT".to_string(), Value::Int(307));
    http_module.insert("STATUS_PERMANENT_REDIRECT".to_string(), Value::Int(308));
    http_module.insert("STATUS_BAD_REQUEST".to_string(), Value::Int(400));
    http_module.insert("STATUS_UNAUTHORIZED".to_string(), Value::Int(401));
    http_module.insert("STATUS_FORBIDDEN".to_string(), Value::Int(403));
    http_module.insert("STATUS_NOT_FOUND".to_string(), Value::Int(404));
    http_module.insert("STATUS_METHOD_NOT_ALLOWED".to_string(), Value::Int(405));
    http_module.insert("STATUS_CONFLICT".to_string(), Value::Int(409));
    http_module.insert("STATUS_UNPROCESSABLE_ENTITY".to_string(), Value::Int(422));
    http_module.insert("STATUS_TOO_MANY_REQUESTS".to_string(), Value::Int(429));
    http_module.insert("STATUS_INTERNAL_SERVER_ERROR".to_string(), Value::Int(500));
    http_module.insert("STATUS_BAD_GATEWAY".to_string(), Value::Int(502));
    http_module.insert("STATUS_SERVICE_UNAVAILABLE".to_string(), Value::Int(503));
    http_module.insert("STATUS_GATEWAY_TIMEOUT".to_string(), Value::Int(504));

    // Socket primitives forwarded to http for self-contained server execution
    http_module.insert("accept".to_string(), Value::Native(crate::stdlib::net::net_accept));
    http_module.insert("write_all".to_string(), Value::Native(crate::stdlib::net::net_write_all));
    http_module.insert("set_nodelay".to_string(), Value::Native(crate::stdlib::net::net_set_nodelay));
    http_module.insert("close".to_string(), Value::Native(crate::stdlib::net::net_close));

    // Compile built-in Whalli routines for server execution
    let server_code = r#"
    import time
    func _whalli_http_run_mw_step(mw, req) {
        if mw is map {
            if mw["__is_cors_mw"] == true {
                let opts = mw["options"]
                if req["method"] == "OPTIONS" {
                    let opt_headers = {
                        "Access-Control-Allow-Origin": opts["origin"],
                        "Access-Control-Allow-Methods": opts["methods"],
                        "Access-Control-Allow-Headers": opts["headers"],
                        "Access-Control-Max-Age": opts["max_age"]
                    }
                    if opts["credentials"] == true {
                        opt_headers["Access-Control-Allow-Credentials"] = "true"
                    }
                    return http.response(204, opt_headers, "")
                }
                if req["res_headers"] != nil {
                    req["res_headers"]["Access-Control-Allow-Origin"] = opts["origin"]
                    req["res_headers"]["Access-Control-Allow-Methods"] = opts["methods"]
                    req["res_headers"]["Access-Control-Allow-Headers"] = opts["headers"]
                    req["res_headers"]["Access-Control-Max-Age"] = opts["max_age"]
                    if opts["credentials"] == true {
                        req["res_headers"]["Access-Control-Allow-Credentials"] = "true"
                    }
                }
                return nil
            }
            if mw["__is_logger_mw"] == true {
                println(f"[{req[\"method\"]}] {req[\"path\"]}")
                return nil
            }
            if mw["__is_secure_headers_mw"] == true {
                if req["res_headers"] != nil {
                    req["res_headers"]["X-Content-Type-Options"] = "nosniff"
                    req["res_headers"]["X-Frame-Options"] = "DENY"
                    req["res_headers"]["X-XSS-Protection"] = "1; mode=block"
                    req["res_headers"]["Referrer-Policy"] = "strict-origin-when-cross-origin"
                }
                return nil
            }
            return nil
        }
        return mw(req)
    }

    func _whalli_http_run_mws(mws, req) {
        if mws == nil {
            return nil
        }
        let i = 0
        while i < mws.len() {
            let mw = mws[i]
            let resp = http._run_mw_step(mw, req)
            if resp != nil {
                return resp
            }
            i += 1
        }
        return nil
    }

    func _whalli_http_handle_missing(handler, req) {
        let allowed = http.allowed_methods(handler, req["path"])
        if allowed != nil {
            if allowed.len() > 0 {
                if handler["method_not_allowed"] != nil {
                    req["allowed_methods"] = allowed
                    return handler["method_not_allowed"](req)
                }
                return http.response(405, {"Allow": allowed.join(", "), "Content-Type": "text/plain; charset=utf-8"}, "405 Method Not Allowed")
            }
        }
        if handler["not_found"] != nil {
            return handler["not_found"](req)
        }
        return http.text_response(404, "404 page not found")
    }

    func _whalli_http_serve_client(client_id, handler) {
        http.set_nodelay(client_id, true)
        let (req, r_err) = http.read_request(client_id)
        if r_err != nil or req == nil {
            http.close(client_id)
            return
        }

        let resp = nil
        if handler is map {
            resp = http._run_mws(handler["middlewares"], req)
            if resp == nil {
                let (fn_route, params) = http.match_route(handler, req["method"], req["path"])
                if fn_route != nil {
                    if params != nil {
                        req["params"] = params
                        resp = http._run_mws(params["_middlewares"], req)
                    }
                    if resp == nil {
                        if fn_route is map {
                            let fp = ""
                            if req["params"] != nil {
                                fp = req["params"]["filepath"]
                            }
                            resp = http.file_response(fn_route["static_dir"], fp)
                        } else {
                            resp = fn_route(req)
                        }
                    }
                } else {
                    resp = http._handle_missing(handler, req)
                }
            }
        } else {
            resp = handler(req)
        }

        if resp != nil {
            if req["res_headers"] != nil {
                resp = http.merge_headers(resp, req["res_headers"])
            }
            if req["method"] == "HEAD" {
                let parts = resp.split("\r\n\r\n")
                if parts.len() >= 2 {
                    resp = parts[0] + "\r\n\r\n"
                }
            }
            http.write_all(client_id, resp)
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
            let c_id = vm.heap.alloc(Obj::Closure(func_obj.clone(), vec![]));
            if func_obj.name == "_whalli_http_serve_client" {
                http_module.insert("_serve_client".to_string(), Value::ObjRef(c_id));
            } else if func_obj.name == "_whalli_http_serve_client_step" {
                http_module.insert("_serve_step".to_string(), Value::ObjRef(c_id));
            } else if func_obj.name == "_whalli_http_listen_and_serve" {
                http_module.insert("listen_and_serve".to_string(), Value::ObjRef(c_id));
            } else if func_obj.name == "_whalli_http_run_mws" {
                http_module.insert("_run_mws".to_string(), Value::ObjRef(c_id));
            } else if func_obj.name == "_whalli_http_run_mw_step" {
                http_module.insert("_run_mw_step".to_string(), Value::ObjRef(c_id));
            } else if func_obj.name == "_whalli_http_handle_missing" {
                http_module.insert("_handle_missing".to_string(), Value::ObjRef(c_id));
            }
        }
    }

    let id = vm.heap.alloc(Obj::Map(http_module));
    Value::ObjRef(id)
}
