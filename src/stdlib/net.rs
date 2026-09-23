use crate::heap::Obj;
use crate::value::{NativeResult, Value};
use crate::vm::VM;
use mio::net::{TcpListener, TcpStream};
use mio::{Interest, Token};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::SocketAddr;
use std::sync::atomic::Ordering;
use std::sync::Arc;

pub fn register(vm: &mut VM) -> Value {
    let mut net_module = HashMap::new();

    // net.listen(port: int, host: str = "127.0.0.1") -> (server_id: int, err: str | nil)
    net_module.insert(
        "listen".to_string(),
        Value::Native(|args, vm| {
            if let Some(Value::Int(port)) = args.first() {
                let host = if args.len() >= 2 {
                    if let Value::Str(ref h) = args[1] {
                        h.as_str()
                    } else {
                        "127.0.0.1"
                    }
                } else {
                    "127.0.0.1"
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
            let res = Value::Tuple(Arc::new(vec![Value::Nil, Value::Str(Arc::new("Expected integer port".to_string()))]));
            NativeResult::Return(res)
        }),
    );

    // net.connect(host: str, port: int) -> (client_id: int, err: str | nil)
    net_module.insert(
        "connect".to_string(),
        Value::Native(|args, vm| {
            if args.len() >= 2 {
                if let (Value::Str(host), Value::Int(port)) = (&args[0], &args[1]) {
                    let addr_str = format!("{}:{}", host, port);
                    let addr: SocketAddr = match addr_str.parse() {
                        Ok(a) => a,
                        Err(e) => {
                            let res = Value::Tuple(Arc::new(vec![Value::Nil, Value::Str(Arc::new(e.to_string()))]));
                            return NativeResult::Return(res);
                        }
                    };

                    let mut stream = match TcpStream::connect(addr) {
                        Ok(s) => s,
                        Err(e) => {
                            let res = Value::Tuple(Arc::new(vec![Value::Nil, Value::Str(Arc::new(e.to_string()))]));
                            return NativeResult::Return(res);
                        }
                    };

                    let token_id = vm.net.next_token.fetch_add(1, Ordering::Relaxed);
                    let token = Token(token_id);

                    let poller = vm.net.poll.lock();
                    if let Err(e) = poller.registry().register(&mut stream, token, Interest::READABLE | Interest::WRITABLE) {
                        let res = Value::Tuple(Arc::new(vec![Value::Nil, Value::Str(Arc::new(e.to_string()))]));
                        return NativeResult::Return(res);
                    }
                    drop(poller);

                    vm.net.streams.write().insert(token_id, stream);
                    let res = Value::Tuple(Arc::new(vec![Value::Int(token_id as i64), Value::Nil]));
                    return NativeResult::Return(res);
                }
            }
            let res = Value::Tuple(Arc::new(vec![Value::Nil, Value::Str(Arc::new("Expected host string and port integer".to_string()))]));
            NativeResult::Return(res)
        }),
    );

    // net.accept(server_id: int) -> client_id: int | nil
    net_module.insert(
        "accept".to_string(),
        Value::Native(|args, vm| {
            if let Some(Value::Int(server_id)) = args.first() {
                let server_id = *server_id as usize;

                let listeners = vm.net.listeners.read();
                let listener = match listeners.get(&server_id) {
                    Some(l) => l,
                    None => return NativeResult::Return(Value::Nil),
                };

                match listener.accept() {
                    Ok((mut stream, _addr)) => {
                        drop(listeners);
                        let token_id = vm.net.next_token.fetch_add(1, Ordering::Relaxed);
                        let token = Token(token_id);

                        let poller = vm.net.poll.lock();
                        poller
                            .registry()
                            .register(&mut stream, token, Interest::READABLE | Interest::WRITABLE)
                            .unwrap();
                        drop(poller);

                        vm.net.streams.write().insert(token_id, stream);
                        return NativeResult::Return(Value::Int(token_id as i64));
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        return NativeResult::SuspendIO(Token(server_id));
                    }
                    Err(_) => {
                        return NativeResult::Return(Value::Nil);
                    }
                }
            }
            NativeResult::Return(Value::Nil)
        }),
    );

    // net.read(client_id: int, max_bytes: int = 4096) -> (data: str | nil, err: str | nil)
    net_module.insert(
        "read".to_string(),
        Value::Native(|args, vm| {
            if let Some(Value::Int(client_id)) = args.first() {
                let client_id = *client_id as usize;
                let max_bytes = if args.len() >= 2 {
                    if let Value::Int(n) = args[1] {
                        (n as usize).max(1).min(65536)
                    } else {
                        4096
                    }
                } else {
                    4096
                };

                let mut streams = vm.net.streams.write();
                let stream = match streams.get_mut(&client_id) {
                    Some(s) => s,
                    None => {
                        let res = Value::Tuple(Arc::new(vec![Value::Nil, Value::Str(Arc::new("Invalid socket client_id".to_string()))]));
                        return NativeResult::Return(res);
                    }
                };

                let mut buffer = vec![0u8; max_bytes];
                match stream.read(&mut buffer) {
                    Ok(0) => {
                        streams.remove(&client_id);
                        let res = Value::Tuple(Arc::new(vec![Value::Nil, Value::Nil])); // EOF
                        return NativeResult::Return(res);
                    }
                    Ok(n) => {
                        let data = String::from_utf8_lossy(&buffer[..n]).to_string();
                        let res = Value::Tuple(Arc::new(vec![Value::Str(Arc::new(data)), Value::Nil]));
                        return NativeResult::Return(res);
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        return NativeResult::SuspendIO(Token(client_id));
                    }
                    Err(e) => {
                        streams.remove(&client_id);
                        let res = Value::Tuple(Arc::new(vec![Value::Nil, Value::Str(Arc::new(e.to_string()))]));
                        return NativeResult::Return(res);
                    }
                }
            }
            let res = Value::Tuple(Arc::new(vec![Value::Nil, Value::Str(Arc::new("Expected integer client_id".to_string()))]));
            NativeResult::Return(res)
        }),
    );

    // net.read_until(client_id: int, delimiter: str = "\r\n\r\n") -> (data: str | nil, err: str | nil)
    net_module.insert(
        "read_until".to_string(),
        Value::Native(|args, vm| {
            if let Some(Value::Int(client_id)) = args.first() {
                let client_id = *client_id as usize;
                let delimiter = if args.len() >= 2 {
                    if let Value::Str(ref d) = args[1] {
                        d.as_str()
                    } else {
                        "\r\n\r\n"
                    }
                } else {
                    "\r\n\r\n"
                };

                let mut streams = vm.net.streams.write();
                let stream = match streams.get_mut(&client_id) {
                    Some(s) => s,
                    None => {
                        let res = Value::Tuple(Arc::new(vec![Value::Nil, Value::Str(Arc::new("Invalid socket client_id".to_string()))]));
                        return NativeResult::Return(res);
                    }
                };

                let mut total_buffer = Vec::new();
                let mut chunk = [0u8; 1024];

                loop {
                    match stream.read(&mut chunk) {
                        Ok(0) => {
                            if total_buffer.is_empty() {
                                streams.remove(&client_id);
                                let res = Value::Tuple(Arc::new(vec![Value::Nil, Value::Nil]));
                                return NativeResult::Return(res);
                            } else {
                                break;
                            }
                        }
                        Ok(n) => {
                            total_buffer.extend_from_slice(&chunk[..n]);
                            if let Ok(text) = std::str::from_utf8(&total_buffer) {
                                if text.contains(delimiter) {
                                    break;
                                }
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

                let data = String::from_utf8_lossy(&total_buffer).to_string();
                let res = Value::Tuple(Arc::new(vec![Value::Str(Arc::new(data)), Value::Nil]));
                return NativeResult::Return(res);
            }
            let res = Value::Tuple(Arc::new(vec![Value::Nil, Value::Str(Arc::new("Expected integer client_id".to_string()))]));
            NativeResult::Return(res)
        }),
    );

    // net.write(client_id, "string") -> (ok: bool, err: str | nil)
    net_module.insert(
        "write".to_string(),
        Value::Native(|args, vm| {
            if args.len() >= 2 {
                if let (Value::Int(client_id), Value::Str(data)) = (&args[0], &args[1]) {
                    let client_id = *client_id as usize;

                    let mut streams = vm.net.streams.write();
                    let stream = match streams.get_mut(&client_id) {
                        Some(s) => s,
                        None => {
                            let res = Value::Tuple(Arc::new(vec![Value::Bool(false), Value::Str(Arc::new("Invalid client_id".to_string()))]));
                            return NativeResult::Return(res);
                        }
                    };

                    match stream.write(data.as_bytes()) {
                        Ok(_) => {
                            let res = Value::Tuple(Arc::new(vec![Value::Bool(true), Value::Nil]));
                            return NativeResult::Return(res);
                        }
                        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            return NativeResult::SuspendIO(Token(client_id));
                        }
                        Err(e) => {
                            streams.remove(&client_id);
                            let res = Value::Tuple(Arc::new(vec![Value::Bool(false), Value::Str(Arc::new(e.to_string()))]));
                            return NativeResult::Return(res);
                        }
                    }
                }
            }
            let res = Value::Tuple(Arc::new(vec![Value::Bool(false), Value::Str(Arc::new("Expected client_id and data string".to_string()))]));
            NativeResult::Return(res)
        }),
    );

    // net.write_all(client_id, "string") -> (ok: bool, err: str | nil)
    net_module.insert(
        "write_all".to_string(),
        Value::Native(|args, vm| {
            if args.len() >= 2 {
                if let (Value::Int(client_id), Value::Str(data)) = (&args[0], &args[1]) {
                    let client_id = *client_id as usize;

                    let mut streams = vm.net.streams.write();
                    let stream = match streams.get_mut(&client_id) {
                        Some(s) => s,
                        None => {
                            let res = Value::Tuple(Arc::new(vec![Value::Bool(false), Value::Str(Arc::new("Invalid client_id".to_string()))]));
                            return NativeResult::Return(res);
                        }
                    };

                    let bytes = data.as_bytes();
                    let mut written = 0;
                    while written < bytes.len() {
                        match stream.write(&bytes[written..]) {
                            Ok(0) => {
                                streams.remove(&client_id);
                                let res = Value::Tuple(Arc::new(vec![Value::Bool(false), Value::Str(Arc::new("Connection closed by peer".to_string()))]));
                                return NativeResult::Return(res);
                            }
                            Ok(n) => written += n,
                            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                                return NativeResult::SuspendIO(Token(client_id));
                            }
                            Err(e) => {
                                streams.remove(&client_id);
                                let res = Value::Tuple(Arc::new(vec![Value::Bool(false), Value::Str(Arc::new(e.to_string()))]));
                                return NativeResult::Return(res);
                            }
                        }
                    }

                    let res = Value::Tuple(Arc::new(vec![Value::Bool(true), Value::Nil]));
                    return NativeResult::Return(res);
                }
            }
            let res = Value::Tuple(Arc::new(vec![Value::Bool(false), Value::Str(Arc::new("Expected client_id and data string".to_string()))]));
            NativeResult::Return(res)
        }),
    );

    // net.peer_addr(client_id) -> str | nil
    net_module.insert(
        "peer_addr".to_string(),
        Value::Native(|args, vm| {
            if let Some(Value::Int(client_id)) = args.first() {
                let client_id = *client_id as usize;
                let streams = vm.net.streams.read();
                if let Some(stream) = streams.get(&client_id) {
                    if let Ok(addr) = stream.peer_addr() {
                        return NativeResult::Return(Value::Str(Arc::new(addr.to_string())));
                    }
                }
            }
            NativeResult::Return(Value::Nil)
        }),
    );

    // net.local_addr(id) -> str | nil
    net_module.insert(
        "local_addr".to_string(),
        Value::Native(|args, vm| {
            if let Some(Value::Int(id)) = args.first() {
                let id = *id as usize;
                let streams = vm.net.streams.read();
                if let Some(stream) = streams.get(&id) {
                    if let Ok(addr) = stream.local_addr() {
                        return NativeResult::Return(Value::Str(Arc::new(addr.to_string())));
                    }
                }
                drop(streams);

                let listeners = vm.net.listeners.read();
                if let Some(listener) = listeners.get(&id) {
                    if let Ok(addr) = listener.local_addr() {
                        return NativeResult::Return(Value::Str(Arc::new(addr.to_string())));
                    }
                }
            }
            NativeResult::Return(Value::Nil)
        }),
    );

    // net.set_nodelay(client_id, enabled: bool) -> bool
    net_module.insert(
        "set_nodelay".to_string(),
        Value::Native(|args, vm| {
            if args.len() >= 2 {
                if let (Value::Int(client_id), Value::Bool(enabled)) = (&args[0], &args[1]) {
                    let client_id = *client_id as usize;
                    let streams = vm.net.streams.read();
                    if let Some(stream) = streams.get(&client_id) {
                        let res = stream.set_nodelay(*enabled).is_ok();
                        return NativeResult::Return(Value::Bool(res));
                    }
                }
            }
            NativeResult::Return(Value::Bool(false))
        }),
    );

    // net.close(id: int) -> bool
    net_module.insert(
        "close".to_string(),
        Value::Native(|args, vm| {
            if let Some(Value::Int(id)) = args.first() {
                let id = *id as usize;
                if let Some(mut stream) = vm.net.streams.write().remove(&id) {
                    let poller = vm.net.poll.lock();
                    let _ = poller.registry().deregister(&mut stream);
                    return NativeResult::Return(Value::Bool(true));
                }
                if let Some(mut listener) = vm.net.listeners.write().remove(&id) {
                    let poller = vm.net.poll.lock();
                    let _ = poller.registry().deregister(&mut listener);
                    return NativeResult::Return(Value::Bool(true));
                }
            }
            NativeResult::Return(Value::Bool(false))
        }),
    );

    let id = vm.heap.alloc(Obj::Map(net_module));
    Value::ObjRef(id)
}
