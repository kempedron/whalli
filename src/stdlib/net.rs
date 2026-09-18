use crate::heap::Obj;
use crate::value::{NativeResult, Value};
use crate::vm::VM;
use mio::net::TcpListener;
use mio::{Interest, Token};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::rc::Rc;

pub fn register(vm: &mut VM) -> Value {
    let mut net_module = HashMap::new();

    net_module.insert(
        "listen".to_string(),
        Value::Native(|args, vm| {
            if let Some(Value::Int(port)) = args.first() {
                let addr = format!("127.0.0.1:{}", port).parse().unwrap();

                let mut listener = TcpListener::bind(addr).unwrap();

                let token_id = vm.next_token;
                vm.next_token += 1;
                let token = Token(token_id);

                vm.poll
                    .registry()
                    .register(&mut listener, token, Interest::READABLE)
                    .unwrap();

                vm.listeners.insert(token_id, listener);

                return NativeResult::Return(Value::Int(token_id as i64));
            }
            NativeResult::Return(Value::Nil)
        }),
    );

    net_module.insert(
        "accept".to_string(),
        Value::Native(|args, vm| {
            if let Some(Value::Int(server_id)) = args.first() {
                let server_id = *server_id as usize;

                let listener = match vm.listeners.get(&server_id) {
                    Some(l) => l,
                    None => return NativeResult::Return(Value::Nil),
                };

                match listener.accept() {
                    Ok((mut stream, _addr)) => {
                        let token_id = vm.next_token;
                        vm.next_token += 1;
                        let token = Token(token_id);

                        vm.poll
                            .registry()
                            .register(&mut stream, token, Interest::READABLE | Interest::WRITABLE)
                            .unwrap();

                        vm.streams.insert(token_id, stream);

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

    // net.read(client_id) -> string
    net_module.insert(
        "read".to_string(),
        Value::Native(|args, vm| {
            if let Some(Value::Int(client_id)) = args.first() {
                let client_id = *client_id as usize;

                let stream = match vm.streams.get_mut(&client_id) {
                    Some(s) => s,
                    None => return NativeResult::Return(Value::Nil),
                };

                let mut buffer = [0; 4096];
                match stream.read(&mut buffer) {
                    Ok(0) => {
                        vm.streams.remove(&client_id);
                        return NativeResult::Return(Value::Nil);
                    }
                    Ok(n) => {
                        let data = String::from_utf8_lossy(&buffer[..n]).to_string();
                        return NativeResult::Return(Value::Str(Rc::new(data)));
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        return NativeResult::SuspendIO(Token(client_id));
                    }
                    Err(_) => {
                        vm.streams.remove(&client_id);
                        return NativeResult::Return(Value::Nil);
                    }
                }
            }
            NativeResult::Return(Value::Nil)
        }),
    );

    // net.write(client_id, "string") -> bool
    net_module.insert(
        "write".to_string(),
        Value::Native(|args, vm| {
            if args.len() >= 2 {
                if let (Value::Int(client_id), Value::Str(data)) = (&args[0], &args[1]) {
                    let client_id = *client_id as usize;

                    let stream = match vm.streams.get_mut(&client_id) {
                        Some(s) => s,
                        None => return NativeResult::Return(Value::Bool(false)),
                    };

                    match stream.write(data.as_bytes()) {
                        Ok(_) => {
                            return NativeResult::Return(Value::Bool(true));
                        }
                        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            return NativeResult::SuspendIO(Token(client_id));
                        }
                        Err(_) => {
                            vm.streams.remove(&client_id);
                            return NativeResult::Return(Value::Bool(false));
                        }
                    }
                }
            }
            NativeResult::Return(Value::Bool(false))
        }),
    );

    // net.close(client_id) -> bool
    net_module.insert(
        "close".to_string(),
        Value::Native(|args, vm| {
            if let Some(Value::Int(client_id)) = args.first() {
                let client_id = *client_id as usize;
                if vm.streams.remove(&client_id).is_some() {
                    return NativeResult::Return(Value::Bool(true));
                }
            }
            NativeResult::Return(Value::Bool(false))
        }),
    );

    let id = vm.heap.alloc(Obj::Map(net_module));
    Value::ObjRef(id)
}
