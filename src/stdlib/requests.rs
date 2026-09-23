use crate::heap::Obj;
use crate::value::{NativeResult, Value};
use crate::vm::VM;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use ureq::ResponseExt;

struct RequestOptions {
    params: Vec<(String, String)>,
    headers: Vec<(String, String)>,
    body: Option<(Vec<u8>, Option<String>)>, // (bytes, content_type)
    timeout: Option<Duration>,
    allow_redirects: bool,
}

fn parse_options(opts_val: Option<&Value>, vm: &VM) -> RequestOptions {
    let mut options = RequestOptions {
        params: Vec::new(),
        headers: Vec::new(),
        body: None,
        timeout: Some(Duration::from_secs(30)),
        allow_redirects: true,
    };

    let Some(Value::ObjRef(opts_id)) = opts_val else {
        return options;
    };

    let _ = vm.heap.with_read(*opts_id, |obj| {
        let Obj::Map(map) = obj else { return };

        // 1. params
        if let Some(Value::ObjRef(p_id)) = map.get("params") {
            let _ = vm.heap.with_read(*p_id, |p_obj| {
                if let Obj::Map(p_map) = p_obj {
                    for (k, v) in p_map {
                        options.params.push((k.clone(), v.stringify(&vm.heap)));
                    }
                }
            });
        }

        // 2. headers
        if let Some(Value::ObjRef(h_id)) = map.get("headers") {
            let _ = vm.heap.with_read(*h_id, |h_obj| {
                if let Obj::Map(h_map) = h_obj {
                    for (k, v) in h_map {
                        options.headers.push((k.clone(), v.stringify(&vm.heap)));
                    }
                }
            });
        }

        // 3. timeout
        if let Some(t_val) = map.get("timeout") {
            match t_val {
                Value::Float(f) => {
                    if *f > 0.0 {
                        options.timeout = Some(Duration::from_secs_f64(*f));
                    }
                }
                Value::Int(i) => {
                    if *i > 0 {
                        options.timeout = Some(Duration::from_secs(*i as u64));
                    }
                }
                _ => {}
            }
        }

        // 4. allow_redirects
        if let Some(Value::Bool(b)) = map.get("allow_redirects") {
            options.allow_redirects = *b;
        }

        // 5. json
        if let Some(json_val) = map.get("json") {
            let jv = crate::stdlib::json::value_to_json(json_val, &vm.heap);
            let s = serde_json::to_string(&jv).unwrap_or_else(|_| "null".to_string());
            options.body = Some((s.into_bytes(), Some("application/json".to_string())));
        } else if let Some(data_val) = map.get("data") {
            match data_val {
                Value::Str(s) => {
                    options.body = Some((s.as_bytes().to_vec(), None));
                }
                Value::ObjRef(d_id) => {
                    // Map as urlencoded form
                    let mut form_parts = Vec::new();
                    let _ = vm.heap.with_read(*d_id, |d_obj| {
                        if let Obj::Map(d_map) = d_obj {
                            for (k, v) in d_map {
                                let key_enc = urlencoding_encode(k);
                                let val_enc = urlencoding_encode(&v.stringify(&vm.heap));
                                form_parts.push(format!("{}={}", key_enc, val_enc));
                            }
                        }
                    });
                    options.body = Some((
                        form_parts.join("&").into_bytes(),
                        Some("application/x-www-form-urlencoded".to_string()),
                    ));
                }
                _ => {
                    let s = data_val.stringify(&vm.heap);
                    options.body = Some((s.into_bytes(), None));
                }
            }
        }
    });

    options
}

fn urlencoding_encode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.' || b == b'~' {
            result.push(b as char);
        } else {
            result.push_str(&format!("%{:02X}", b));
        }
    }
    result
}

fn make_error_response(url: &str, err_msg: &str, vm: &mut VM) -> Value {
    let mut resp_map = HashMap::new();
    let empty_headers_id = vm.heap.alloc(Obj::Map(HashMap::new()));

    resp_map.insert("status_code".to_string(), Value::Int(0));
    resp_map.insert("status_text".to_string(), Value::Str(Arc::new(String::new())));
    resp_map.insert("ok".to_string(), Value::Bool(false));
    resp_map.insert("text".to_string(), Value::Str(Arc::new(String::new())));
    resp_map.insert("headers".to_string(), Value::ObjRef(empty_headers_id));
    resp_map.insert("url".to_string(), Value::Str(Arc::new(url.to_string())));
    resp_map.insert("content_length".to_string(), Value::Int(0));
    resp_map.insert("error".to_string(), Value::Str(Arc::new(err_msg.to_string())));

    let id = vm.heap.alloc(Obj::Map(resp_map));
    Value::ObjRef(id)
}

fn execute_request(method_str: &str, url_str: &str, opts_val: Option<&Value>, vm: &mut VM) -> Value {
    let opts = parse_options(opts_val, vm);
    let method_upper = method_str.to_uppercase();

    let mut config_builder = ureq::Agent::config_builder();
    config_builder = config_builder.http_status_as_error(false);
    if let Some(to) = opts.timeout {
        config_builder = config_builder.timeout_global(Some(to));
    }
    if !opts.allow_redirects {
        config_builder = config_builder.max_redirects(0);
    }
    let agent: ureq::Agent = config_builder.build().into();

    macro_rules! configure_req {
        ($req:expr) => {{
            let mut r = $req;
            for (k, v) in &opts.params {
                r = r.query(k.as_str(), v.as_str());
            }
            for (k, v) in &opts.headers {
                r = r.header(k.as_str(), v.as_str());
            }
            r
        }};
    }

    let response_result = match method_upper.as_str() {
        "GET" => {
            let mut req = configure_req!(agent.get(url_str));
            if let Some((body_bytes, ct)) = opts.body {
                if let Some(ct_str) = ct {
                    req = req.header("Content-Type", ct_str.as_str());
                }
                req.force_send_body().send(body_bytes)
            } else {
                req.call()
            }
        }
        "DELETE" => {
            let mut req = configure_req!(agent.delete(url_str));
            if let Some((body_bytes, ct)) = opts.body {
                if let Some(ct_str) = ct {
                    req = req.header("Content-Type", ct_str.as_str());
                }
                req.force_send_body().send(body_bytes)
            } else {
                req.call()
            }
        }
        "HEAD" => {
            let req = configure_req!(agent.head(url_str));
            req.call()
        }
        "POST" => {
            let mut req = configure_req!(agent.post(url_str));
            if let Some((body_bytes, ct)) = opts.body {
                if let Some(ct_str) = ct {
                    req = req.header("Content-Type", ct_str.as_str());
                }
                req.send(body_bytes)
            } else {
                req.send_empty()
            }
        }
        "PUT" => {
            let mut req = configure_req!(agent.put(url_str));
            if let Some((body_bytes, ct)) = opts.body {
                if let Some(ct_str) = ct {
                    req = req.header("Content-Type", ct_str.as_str());
                }
                req.send(body_bytes)
            } else {
                req.send_empty()
            }
        }
        "PATCH" => {
            let mut req = configure_req!(agent.patch(url_str));
            if let Some((body_bytes, ct)) = opts.body {
                if let Some(ct_str) = ct {
                    req = req.header("Content-Type", ct_str.as_str());
                }
                req.send(body_bytes)
            } else {
                req.send_empty()
            }
        }
        other => {
            return make_error_response(url_str, &format!("Unsupported HTTP method: {}", other), vm);
        }
    };

    match response_result {
        Ok(mut res) => {
            let status_code = res.status().as_u16() as i64;
            let status_text = res.status().canonical_reason().unwrap_or("").to_string();
            let ok = status_code >= 200 && status_code < 400;
            let final_url = res.get_uri().to_string();

            let mut headers_map = HashMap::new();
            for (k, v) in res.headers() {
                if let Ok(val_str) = v.to_str() {
                    headers_map.insert(k.as_str().to_lowercase(), Value::Str(Arc::new(val_str.to_string())));
                }
            }
            let headers_id = vm.heap.alloc(Obj::Map(headers_map));

            let body_str = res.body_mut().read_to_string().unwrap_or_default();
            let content_len = body_str.len() as i64;

            let mut resp_map = HashMap::new();
            resp_map.insert("status_code".to_string(), Value::Int(status_code));
            resp_map.insert("status_text".to_string(), Value::Str(Arc::new(status_text)));
            resp_map.insert("ok".to_string(), Value::Bool(ok));
            resp_map.insert("text".to_string(), Value::Str(Arc::new(body_str)));
            resp_map.insert("headers".to_string(), Value::ObjRef(headers_id));
            resp_map.insert("url".to_string(), Value::Str(Arc::new(final_url)));
            resp_map.insert("content_length".to_string(), Value::Int(content_len));
            resp_map.insert("error".to_string(), Value::Nil);

            let id = vm.heap.alloc(Obj::Map(resp_map));
            Value::ObjRef(id)
        }
        Err(e) => make_error_response(url_str, &e.to_string(), vm),
    }
}

pub fn register(vm: &mut VM) -> Value {
    let mut req_module = HashMap::new();

    // requests.get(url, options = nil)
    req_module.insert(
        "get".to_string(),
        Value::Native(|args, vm| {
            if let Some(Value::Str(url)) = args.first() {
                let opts = args.get(1);
                let res = execute_request("GET", url.as_str(), opts, vm);
                NativeResult::Return(res)
            } else {
                let res = make_error_response("", "requests.get expects a URL string", vm);
                NativeResult::Return(res)
            }
        }),
    );

    // requests.post(url, options = nil)
    req_module.insert(
        "post".to_string(),
        Value::Native(|args, vm| {
            if let Some(Value::Str(url)) = args.first() {
                let opts = args.get(1);
                let res = execute_request("POST", url.as_str(), opts, vm);
                NativeResult::Return(res)
            } else {
                let res = make_error_response("", "requests.post expects a URL string", vm);
                NativeResult::Return(res)
            }
        }),
    );

    // requests.put(url, options = nil)
    req_module.insert(
        "put".to_string(),
        Value::Native(|args, vm| {
            if let Some(Value::Str(url)) = args.first() {
                let opts = args.get(1);
                let res = execute_request("PUT", url.as_str(), opts, vm);
                NativeResult::Return(res)
            } else {
                let res = make_error_response("", "requests.put expects a URL string", vm);
                NativeResult::Return(res)
            }
        }),
    );

    // requests.delete(url, options = nil)
    req_module.insert(
        "delete".to_string(),
        Value::Native(|args, vm| {
            if let Some(Value::Str(url)) = args.first() {
                let opts = args.get(1);
                let res = execute_request("DELETE", url.as_str(), opts, vm);
                NativeResult::Return(res)
            } else {
                let res = make_error_response("", "requests.delete expects a URL string", vm);
                NativeResult::Return(res)
            }
        }),
    );

    // requests.patch(url, options = nil)
    req_module.insert(
        "patch".to_string(),
        Value::Native(|args, vm| {
            if let Some(Value::Str(url)) = args.first() {
                let opts = args.get(1);
                let res = execute_request("PATCH", url.as_str(), opts, vm);
                NativeResult::Return(res)
            } else {
                let res = make_error_response("", "requests.patch expects a URL string", vm);
                NativeResult::Return(res)
            }
        }),
    );

    // requests.head(url, options = nil)
    req_module.insert(
        "head".to_string(),
        Value::Native(|args, vm| {
            if let Some(Value::Str(url)) = args.first() {
                let opts = args.get(1);
                let res = execute_request("HEAD", url.as_str(), opts, vm);
                NativeResult::Return(res)
            } else {
                let res = make_error_response("", "requests.head expects a URL string", vm);
                NativeResult::Return(res)
            }
        }),
    );

    // requests.request(method, url, options = nil)
    req_module.insert(
        "request".to_string(),
        Value::Native(|args, vm| {
            if args.len() >= 2 {
                if let (Value::Str(method), Value::Str(url)) = (&args[0], &args[1]) {
                    let opts = args.get(2);
                    let res = execute_request(method.as_str(), url.as_str(), opts, vm);
                    return NativeResult::Return(res);
                }
            }
            let res = make_error_response("", "requests.request expects method and url strings", vm);
            NativeResult::Return(res)
        }),
    );

    let id = vm.heap.alloc(Obj::Map(req_module));
    Value::ObjRef(id)
}
