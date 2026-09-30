use crate::value::Value;
use crate::vm::VM;
use std::io::{Read, Write};
use std::sync::Arc;

pub const WS_OPCODE_TEXT: u8 = 0x1;
pub const WS_OPCODE_BINARY: u8 = 0x2;
pub const WS_OPCODE_CLOSE: u8 = 0x8;
pub const WS_OPCODE_PING: u8 = 0x9;
pub const WS_OPCODE_PONG: u8 = 0xA;

/// Encodes a WebSocket frame (Server-to-Client frames must NOT be masked according to RFC 6455)
pub fn encode_frame(opcode: u8, payload: &[u8], fin: bool) -> Vec<u8> {
    let mut frame = Vec::with_capacity(payload.len() + 10);
    let b0 = (if fin { 0x80 } else { 0x00 }) | (opcode & 0x0F);
    frame.push(b0);

    let len = payload.len();
    if len <= 125 {
        frame.push(len as u8);
    } else if len <= 65535 {
        frame.push(126);
        frame.extend_from_slice(&(len as u16).to_be_bytes());
    } else {
        frame.push(127);
        frame.extend_from_slice(&(len as u64).to_be_bytes());
    }

    frame.extend_from_slice(payload);
    frame
}

#[derive(Debug, PartialEq, Eq)]
pub enum FrameParseResult {
    Complete {
        opcode: u8,
        payload: Vec<u8>,
        bytes_consumed: usize,
    },
    Incomplete,
    Error(String),
}

/// Parses a Client-to-Server frame (Client frames MUST be masked according to RFC 6455)
pub fn parse_frame(data: &[u8]) -> FrameParseResult {
    if data.len() < 2 {
        return FrameParseResult::Incomplete;
    }

    let b0 = data[0];
    let b1 = data[1];

    let _fin = (b0 & 0x80) != 0;
    let opcode = b0 & 0x0F;
    let masked = (b1 & 0x80) != 0;
    let base_len = (b1 & 0x7F) as usize;

    let mut offset = 2;
    let payload_len: usize = if base_len <= 125 {
        base_len
    } else if base_len == 126 {
        if data.len() < offset + 2 {
            return FrameParseResult::Incomplete;
        }
        let len = u16::from_be_bytes([data[offset], data[offset + 1]]) as usize;
        offset += 2;
        len
    } else if base_len == 127 {
        if data.len() < offset + 8 {
            return FrameParseResult::Incomplete;
        }
        let len = u64::from_be_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
            data[offset + 4],
            data[offset + 5],
            data[offset + 6],
            data[offset + 7],
        ]) as usize;
        offset += 8;
        len
    } else {
        return FrameParseResult::Error("Invalid payload length".to_string());
    };

    let mask_key = if masked {
        if data.len() < offset + 4 {
            return FrameParseResult::Incomplete;
        }
        let key = [data[offset], data[offset + 1], data[offset + 2], data[offset + 3]];
        offset += 4;
        Some(key)
    } else {
        None
    };

    if data.len() < offset + payload_len {
        return FrameParseResult::Incomplete;
    }

    let mut payload = data[offset..offset + payload_len].to_vec();
    if let Some(mask) = mask_key {
        for i in 0..payload.len() {
            payload[i] ^= mask[i % 4];
        }
    }

    FrameParseResult::Complete {
        opcode,
        payload,
        bytes_consumed: offset + payload_len,
    }
}

pub fn ws_send_internal(client_id: usize, msg_val: &Value, vm: &mut VM) -> (bool, Option<String>) {
    let (payload, opcode) = match msg_val {
        Value::Str(s) => (s.as_bytes().to_vec(), WS_OPCODE_TEXT),
        Value::Bytes(b) => ((**b).clone(), WS_OPCODE_BINARY),
        other => (other.stringify(&vm.heap).into_bytes(), WS_OPCODE_TEXT),
    };

    let frame = encode_frame(opcode, &payload, true);
    let mut streams = vm.net.streams.write();
    let stream = match streams.get_mut(&client_id) {
        Some(s) => s,
        None => return (false, Some("Socket not found or already closed".to_string())),
    };

    let mut written = 0;
    while written < frame.len() {
        match stream.write(&frame[written..]) {
            Ok(0) => return (false, Some("Connection closed by peer".to_string())),
            Ok(n) => written += n,
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            Err(e) => return (false, Some(e.to_string())),
        }
    }
    (true, None)
}

pub fn ws_read_internal(client_id: usize, vm: &mut VM) -> (Option<Value>, Option<String>, bool) {
    let mut chunk = [0u8; 4096];

    // First check if there is an existing buffer
    let mut buffer = {
        let mut bufs = vm.net.stream_buffers.lock();
        bufs.remove(&client_id).unwrap_or_default()
    };

    if !buffer.is_empty() {
        match parse_frame(&buffer) {
            FrameParseResult::Complete { opcode, payload, bytes_consumed } => {
                let leftover = buffer.split_off(bytes_consumed);
                if !leftover.is_empty() {
                    vm.net.stream_buffers.lock().insert(client_id, leftover);
                }

                match opcode {
                    WS_OPCODE_TEXT => {
                        let text = String::from_utf8_lossy(&payload).to_string();
                        return (Some(Value::Str(Arc::new(text))), None, false);
                    }
                    WS_OPCODE_BINARY => {
                        return (Some(Value::Bytes(Arc::new(payload))), None, false);
                    }
                    WS_OPCODE_CLOSE => {
                        return (None, None, false);
                    }
                    WS_OPCODE_PING => {
                        let pong = encode_frame(WS_OPCODE_PONG, &payload, true);
                        let mut streams = vm.net.streams.write();
                        if let Some(stream) = streams.get_mut(&client_id) {
                            let _ = stream.write_all(&pong);
                        }
                    }
                    _ => {}
                }
            }
            FrameParseResult::Error(e) => {
                return (None, Some(e), false);
            }
            FrameParseResult::Incomplete => {
                vm.net.stream_buffers.lock().insert(client_id, buffer);
            }
        }
    }

    // Read more from socket
    let mut streams = vm.net.streams.write();
    let stream = match streams.get_mut(&client_id) {
        Some(s) => s,
        None => return (None, Some("Socket not found".to_string()), false),
    };

        match stream.read(&mut chunk) {
            Ok(0) => {
                return (None, None, false);
            }
            Ok(n) => {
                eprintln!("[WS DEBUG SERVER] Read {} bytes!", n);
                drop(streams);
                let mut bufs = vm.net.stream_buffers.lock();
                let b = bufs.entry(client_id).or_insert_with(Vec::new);
                b.extend_from_slice(&chunk[..n]);
                let mut buf_clone = b.clone();
                drop(bufs);

                match parse_frame(&buf_clone) {
                    FrameParseResult::Complete { opcode, payload, bytes_consumed } => {
                        let leftover = buf_clone.split_off(bytes_consumed);
                        let mut bufs = vm.net.stream_buffers.lock();
                        if leftover.is_empty() {
                            bufs.remove(&client_id);
                        } else {
                            bufs.insert(client_id, leftover);
                        }
                        match opcode {
                            WS_OPCODE_TEXT => {
                                let text = String::from_utf8_lossy(&payload).to_string();
                                return (Some(Value::Str(Arc::new(text))), None, false);
                            }
                            WS_OPCODE_BINARY => {
                                return (Some(Value::Bytes(Arc::new(payload))), None, false);
                            }
                            WS_OPCODE_CLOSE => {
                                return (None, None, false);
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
                (None, Some("WouldBlock".to_string()), true)
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                eprintln!("[WS DEBUG SERVER] WouldBlock");
                // If stream would block, give thread a short sleep to allow incoming OS socket bytes to arrive!
                drop(streams);
                std::thread::sleep(std::time::Duration::from_millis(5));
                (None, Some("WouldBlock".to_string()), true)
            }
            Err(e) => {
                eprintln!("[WS DEBUG SERVER] Err: {:?}", e);
                return (None, Some(e.to_string()), false);
            }
        }
}

pub fn ws_close_internal(client_id: usize, vm: &mut VM) {
    let close_frame = encode_frame(WS_OPCODE_CLOSE, &[0x03, 0xE8], true); // 1000 Normal Closure
    let mut streams = vm.net.streams.write();
    if let Some(stream) = streams.get_mut(&client_id) {
        let _ = stream.write_all(&close_frame);
        let _ = stream.flush();
    }
    drop(streams);
    crate::stdlib::net::net_close(vec![Value::Int(client_id as i64)], vm);
}
