use crate::heap::Obj;
use crate::value::{NativeResult, Value};
use crate::vm::VM;
use std::collections::HashMap;
use std::sync::Arc;

// --- SHA-256 implementation (RFC 6234) ---
pub fn sha256(data: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
        0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
        0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
        0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
        0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
        0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
        0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];

    let mut h = [
        0x6a09e667u32, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
        0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
    ];

    let msg_len = data.len();
    let bit_len = (msg_len as u64) * 8;

    let mut padded = Vec::with_capacity(msg_len + 64);
    padded.extend_from_slice(data);
    padded.push(0x80);

    while (padded.len() % 64) != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in padded.chunks_exact(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                chunk[i * 4],
                chunk[i * 4 + 1],
                chunk[i * 4 + 2],
                chunk[i * 4 + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }

        let mut a = h[0];
        let mut b = h[1];
        let mut c = h[2];
        let mut d = h[3];
        let mut e = h[4];
        let mut f = h[5];
        let mut g = h[6];
        let mut hh = h[7];

        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = hh.wrapping_add(s1).wrapping_add(ch).wrapping_add(K[i]).wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }

    let mut out = [0u8; 32];
    for (i, val) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&val.to_be_bytes());
    }
    out
}

// --- MD5 implementation (RFC 1321) ---
pub fn md5(data: &[u8]) -> [u8; 16] {
    let mut a: u32 = 0x67452301;
    let mut b: u32 = 0xefcdab89;
    let mut c: u32 = 0x98badcfe;
    let mut d: u32 = 0x10325476;

    let msg_len = data.len();
    let bit_len = (msg_len as u64) * 8;

    let mut padded = Vec::with_capacity(msg_len + 64);
    padded.extend_from_slice(data);
    padded.push(0x80);

    while (padded.len() % 64) != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_le_bytes());

    const S: [u32; 64] = [
        7, 12, 17, 22,  7, 12, 17, 22,  7, 12, 17, 22,  7, 12, 17, 22,
        5,  9, 14, 20,  5,  9, 14, 20,  5,  9, 14, 20,  5,  9, 14, 20,
        4, 11, 16, 23,  4, 11, 16, 23,  4, 11, 16, 23,  4, 11, 16, 23,
        6, 10, 15, 21,  6, 10, 15, 21,  6, 10, 15, 21,  6, 10, 15, 21,
    ];

    const K: [u32; 64] = [
        0xd76aa478, 0xe8c7b756, 0x242070db, 0xc1bdceee, 0xf57c0faf, 0x4787c62a, 0xa8304613, 0xfd469501,
        0x698098d8, 0x8b44f7af, 0xffff5bb1, 0x895cd7be, 0x6b901122, 0xfd987193, 0xa679438e, 0x49b40821,
        0xf61e2562, 0xc040b340, 0x265e5a51, 0xe9b6c7aa, 0xd62f105d, 0x02441453, 0xd8a1e681, 0xe7d3fbc8,
        0x21e1cde6, 0xc33707d6, 0xf4d50d87, 0x455a14ed, 0xa9e3e905, 0xfcefa3f8, 0x676f02d9, 0x8d2a4c8a,
        0xfffa3942, 0x8771f681, 0x6d9d6122, 0xfde5380c, 0xa4beea44, 0x4bdecfa9, 0xf6bb4b60, 0xbebfbc70,
        0x289b7ec6, 0xeaa127fa, 0xd4ef3085, 0x04881d05, 0xd9d4d039, 0xe6db99e5, 0x1fa27cf8, 0xc4ac5665,
        0xf4292244, 0x432aff97, 0xab9423a7, 0xfc93a039, 0x655b59c3, 0x8f0ccc92, 0xffeff47d, 0x85845dd1,
        0x6fa87e4f, 0xfe2ce6e0, 0xa3014314, 0x4e0811a1, 0xf7537e82, 0xbd3af235, 0x2ad7d2bb, 0xeb86d391,
    ];

    for chunk in padded.chunks_exact(64) {
        let mut m = [0u32; 16];
        for i in 0..16 {
            m[i] = u32::from_le_bytes([
                chunk[i * 4],
                chunk[i * 4 + 1],
                chunk[i * 4 + 2],
                chunk[i * 4 + 3],
            ]);
        }

        let mut aa = a;
        let mut bb = b;
        let mut cc = c;
        let mut dd = d;

        for i in 0..64 {
            let (f, g) = match i {
                0..=15 => ((bb & cc) | ((!bb) & dd), i),
                16..=31 => ((dd & bb) | ((!dd) & cc), (5 * i + 1) % 16),
                32..=47 => (bb ^ cc ^ dd, (3 * i + 5) % 16),
                _ => (cc ^ (bb | (!dd)), (7 * i) % 16),
            };

            let temp = dd;
            dd = cc;
            cc = bb;
            bb = bb.wrapping_add(
                aa.wrapping_add(f)
                    .wrapping_add(K[i])
                    .wrapping_add(m[g])
                    .rotate_left(S[i]),
            );
            aa = temp;
        }

        a = a.wrapping_add(aa);
        b = b.wrapping_add(bb);
        c = c.wrapping_add(cc);
        d = d.wrapping_add(dd);
    }

    let mut out = [0u8; 16];
    out[0..4].copy_from_slice(&a.to_le_bytes());
    out[4..8].copy_from_slice(&b.to_le_bytes());
    out[8..12].copy_from_slice(&c.to_le_bytes());
    out[12..16].copy_from_slice(&d.to_le_bytes());
    out
}

// --- HMAC-SHA256 implementation (RFC 2104) ---
pub fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    const BLOCK_SIZE: usize = 64;
    let mut k = [0u8; BLOCK_SIZE];

    if key.len() > BLOCK_SIZE {
        let hash = sha256(key);
        k[..32].copy_from_slice(&hash);
    } else {
        k[..key.len()].copy_from_slice(key);
    }

    let mut o_key_pad = [0x5cu8; BLOCK_SIZE];
    let mut i_key_pad = [0x36u8; BLOCK_SIZE];

    for i in 0..BLOCK_SIZE {
        o_key_pad[i] ^= k[i];
        i_key_pad[i] ^= k[i];
    }

    let mut inner = Vec::with_capacity(BLOCK_SIZE + message.len());
    inner.extend_from_slice(&i_key_pad);
    inner.extend_from_slice(message);
    let inner_hash = sha256(&inner);

    let mut outer = Vec::with_capacity(BLOCK_SIZE + 32);
    outer.extend_from_slice(&o_key_pad);
    outer.extend_from_slice(&inner_hash);
    sha256(&outer)
}

pub fn to_hex(data: &[u8]) -> String {
    data.iter().map(|b| format!("{:02x}", b)).collect()
}

pub fn from_hex(hex_str: &str) -> Result<Vec<u8>, String> {
    let clean = hex_str.trim();
    if clean.len() % 2 != 0 {
        return Err("Hex string must have an even length".to_string());
    }
    let mut bytes = Vec::with_capacity(clean.len() / 2);
    for i in (0..clean.len()).step_by(2) {
        let byte = u8::from_str_radix(&clean[i..i + 2], 16)
            .map_err(|e| format!("Invalid hex byte '{}': {}", &clean[i..i + 2], e))?;
        bytes.push(byte);
    }
    Ok(bytes)
}

pub fn base64_decode(encoded: &str) -> Result<Vec<u8>, String> {
    let clean: String = encoded.chars().filter(|c| !c.is_whitespace()).collect();
    if clean.is_empty() {
        return Ok(Vec::new());
    }
    if clean.len() % 4 != 0 {
        return Err("Base64 string length must be a multiple of 4".to_string());
    }

    fn b64_val(c: u8) -> Result<u32, String> {
        match c {
            b'A'..=b'Z' => Ok((c - b'A') as u32),
            b'a'..=b'z' => Ok((c - b'a' + 26) as u32),
            b'0'..=b'9' => Ok((c - b'0' + 52) as u32),
            b'+' => Ok(62),
            b'/' => Ok(63),
            b'=' => Ok(0),
            _ => Err(format!("Invalid base64 character: {}", c as char)),
        }
    }

    let bytes = clean.as_bytes();
    let mut out = Vec::with_capacity(clean.len() / 4 * 3);

    for chunk in bytes.chunks_exact(4) {
        let b0 = b64_val(chunk[0])?;
        let b1 = b64_val(chunk[1])?;
        let b2 = b64_val(chunk[2])?;
        let b3 = b64_val(chunk[3])?;

        let triple = (b0 << 18) | (b1 << 12) | (b2 << 6) | b3;

        out.push(((triple >> 16) & 0xFF) as u8);
        if chunk[2] != b'=' {
            out.push(((triple >> 8) & 0xFF) as u8);
        }
        if chunk[3] != b'=' {
            out.push((triple & 0xFF) as u8);
        }
    }
    Ok(out)
}

pub fn get_random_bytes(len: usize) -> Vec<u8> {
    let mut buf = vec![0u8; len];
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        use std::io::Read;
        let _ = f.read_exact(&mut buf);
        return buf;
    }

    // Fallback pseudo-random using high-resolution time & process info
    let mut seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(123456789);

    for b in &mut buf {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        *b = (seed >> 33) as u8;
    }
    buf
}

pub fn generate_uuid4() -> String {
    let mut b = get_random_bytes(16);
    // Set version 4: 0100xxxx
    b[6] = (b[6] & 0x0F) | 0x40;
    // Set variant 1: 10xxxxxx
    b[8] = (b[8] & 0x3F) | 0x80;

    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        b[0], b[1], b[2], b[3],
        b[4], b[5],
        b[6], b[7],
        b[8], b[9],
        b[10], b[11], b[12], b[13], b[14], b[15]
    )
}

fn extract_bytes_arg(val: &Value, heap: &crate::heap::Heap) -> Vec<u8> {
    match val {
        Value::Str(s) => s.as_bytes().to_vec(),
        Value::Bytes(b) => (**b).clone(),
        other => other.stringify(heap).into_bytes(),
    }
}

pub fn register(vm: &mut VM) -> Value {
    let mut crypto_module = HashMap::new();

    // crypto.sha256(data: str | bytes) -> str (hex)
    crypto_module.insert(
        "sha256".to_string(),
        Value::Native(|args, vm| {
            if let Some(val) = args.first() {
                let data = extract_bytes_arg(val, &vm.heap);
                let digest = sha256(&data);
                NativeResult::Return(Value::Str(Arc::new(to_hex(&digest))))
            } else {
                NativeResult::Return(Value::Str(Arc::new(String::new())))
            }
        }),
    );

    // crypto.sha1(data: str | bytes) -> str (hex)
    crypto_module.insert(
        "sha1".to_string(),
        Value::Native(|args, vm| {
            if let Some(val) = args.first() {
                let data = extract_bytes_arg(val, &vm.heap);
                let digest = crate::stdlib::ws_crypto::sha1(&data);
                NativeResult::Return(Value::Str(Arc::new(to_hex(&digest))))
            } else {
                NativeResult::Return(Value::Str(Arc::new(String::new())))
            }
        }),
    );

    // crypto.md5(data: str | bytes) -> str (hex)
    crypto_module.insert(
        "md5".to_string(),
        Value::Native(|args, vm| {
            if let Some(val) = args.first() {
                let data = extract_bytes_arg(val, &vm.heap);
                let digest = md5(&data);
                NativeResult::Return(Value::Str(Arc::new(to_hex(&digest))))
            } else {
                NativeResult::Return(Value::Str(Arc::new(String::new())))
            }
        }),
    );

    // crypto.hmac_sha256(key: str | bytes, message: str | bytes) -> str (hex)
    crypto_module.insert(
        "hmac_sha256".to_string(),
        Value::Native(|args, vm| {
            if args.len() >= 2 {
                let key = extract_bytes_arg(&args[0], &vm.heap);
                let msg = extract_bytes_arg(&args[1], &vm.heap);
                let digest = hmac_sha256(&key, &msg);
                NativeResult::Return(Value::Str(Arc::new(to_hex(&digest))))
            } else {
                NativeResult::Return(Value::Str(Arc::new(String::new())))
            }
        }),
    );

    // crypto.base64_encode(data: str | bytes) -> str
    crypto_module.insert(
        "base64_encode".to_string(),
        Value::Native(|args, vm| {
            if let Some(val) = args.first() {
                let data = extract_bytes_arg(val, &vm.heap);
                let encoded = crate::stdlib::ws_crypto::base64_encode(&data);
                NativeResult::Return(Value::Str(Arc::new(encoded)))
            } else {
                NativeResult::Return(Value::Str(Arc::new(String::new())))
            }
        }),
    );

    // crypto.base64_decode(encoded_str: str) -> (data: bytes | nil, err: str | nil)
    crypto_module.insert(
        "base64_decode".to_string(),
        Value::Native(|args, vm| {
            if let Some(val) = args.first() {
                let s = match val {
                    Value::Str(s) => s.as_str(),
                    other => other.stringify(&vm.heap).leak(),
                };
                match base64_decode(s) {
                    Ok(b) => {
                        let res = Value::Tuple(Arc::new(vec![Value::Bytes(Arc::new(b)), Value::Nil]));
                        NativeResult::Return(res)
                    }
                    Err(e) => {
                        let res = Value::Tuple(Arc::new(vec![Value::Nil, Value::Str(Arc::new(e))]));
                        NativeResult::Return(res)
                    }
                }
            } else {
                let res = Value::Tuple(Arc::new(vec![Value::Nil, Value::Str(Arc::new("Expected base64 string".to_string()))]));
                NativeResult::Return(res)
            }
        }),
    );

    // crypto.hex_encode(data: bytes | str) -> str
    crypto_module.insert(
        "hex_encode".to_string(),
        Value::Native(|args, vm| {
            if let Some(val) = args.first() {
                let data = extract_bytes_arg(val, &vm.heap);
                NativeResult::Return(Value::Str(Arc::new(to_hex(&data))))
            } else {
                NativeResult::Return(Value::Str(Arc::new(String::new())))
            }
        }),
    );

    // crypto.hex_decode(hex_str: str) -> (data: bytes | nil, err: str | nil)
    crypto_module.insert(
        "hex_decode".to_string(),
        Value::Native(|args, vm| {
            if let Some(val) = args.first() {
                let s = match val {
                    Value::Str(s) => s.as_str(),
                    other => other.stringify(&vm.heap).leak(),
                };
                match from_hex(s) {
                    Ok(b) => {
                        let res = Value::Tuple(Arc::new(vec![Value::Bytes(Arc::new(b)), Value::Nil]));
                        NativeResult::Return(res)
                    }
                    Err(e) => {
                        let res = Value::Tuple(Arc::new(vec![Value::Nil, Value::Str(Arc::new(e))]));
                        NativeResult::Return(res)
                    }
                }
            } else {
                let res = Value::Tuple(Arc::new(vec![Value::Nil, Value::Str(Arc::new("Expected hex string".to_string()))]));
                NativeResult::Return(res)
            }
        }),
    );

    // crypto.random_bytes(length: int = 16) -> bytes
    crypto_module.insert(
        "random_bytes".to_string(),
        Value::Native(|args, _vm| {
            let len = match args.first() {
                Some(Value::Int(n)) => (*n as usize).max(0).min(1024 * 1024),
                _ => 16,
            };
            let bytes = get_random_bytes(len);
            NativeResult::Return(Value::Bytes(Arc::new(bytes)))
        }),
    );

    // crypto.random_hex(length: int = 16) -> str
    crypto_module.insert(
        "random_hex".to_string(),
        Value::Native(|args, _vm| {
            let len = match args.first() {
                Some(Value::Int(n)) => (*n as usize).max(0).min(1024 * 1024),
                _ => 16,
            };
            let bytes = get_random_bytes(len);
            NativeResult::Return(Value::Str(Arc::new(to_hex(&bytes))))
        }),
    );

    // crypto.uuid4() -> str
    crypto_module.insert(
        "uuid4".to_string(),
        Value::Native(|_args, _vm| {
            let uuid = generate_uuid4();
            NativeResult::Return(Value::Str(Arc::new(uuid)))
        }),
    );

    let id = vm.heap.alloc(Obj::Map(crypto_module));
    Value::ObjRef(id)
}
