---
title: Standard Library
description: Reference of Whalli's built-in standard library modules.
---

## Overview

Whalli comes with built-in modules in its runtime. Standard modules are imported with `import <module>` without local file prefixes.

---

## `requests` Module (HTTP Client)

Ergonomic HTTP/HTTPS client:

- `requests.get(url: str, options: map = nil) -> map`
- `requests.post(url: str, options: map = nil) -> map`
- `requests.put(url: str, options: map = nil) -> map`
- `requests.delete(url: str, options: map = nil) -> map`

The returned response object provides: `status_code`, `status_text`, `ok` (bool), `text`, `url`, `error`, `.json()`, and `.header(name)`.

```whalli
import requests

let res = requests.get("https://httpbin.org/get", {"timeout": 5})
if res.ok {
    println("HTTP Status:", res.status_code)
    let body = res.json()
}
```

---

## `sql` Module

Unified SQL database interface supporting **SQLite**, **PostgreSQL**, and **MySQL**.

### Supported Drivers

- `"sqlite"` / `"sqlite3"`: Zero-dependency embedded SQLite (`":memory:"` or `"app.db"`).
- `"postgres"` / `"postgresql"` / `"pg"`: PostgreSQL client with automatic `?` to `$1` parameter translation.
- `"mysql"` / `"mariadb"`: MySQL client.

### API Reference

- `sql.open(driver: str, conn_str: str) -> (db: map | nil, err: str | nil)`
- `db.exec(query: str, params: list = []) -> (result: map | nil, err: str | nil)`: runs DDL and mutations. Returns `{"rows_affected": int, "last_insert_id": int}`.
- `db.query(query: str, params: list = []) -> (rows: list[map] | nil, err: str | nil)`: runs SELECT queries. Returns a list of maps keyed by column name.
- `db.query_row(query: str, params: list = []) -> (row: map | nil, err: str | nil)`: returns first matching row as a map, or `nil`.
- `db.close() -> bool`: closes connection.

```whalli
import sql

let (db, err) = sql.open("sqlite", "app.db")
db.exec("CREATE TABLE IF NOT EXISTS users (id INTEGER PRIMARY KEY, name TEXT)")
db.exec("INSERT INTO users (name) VALUES (?)", ["Alice"])

let (rows, _) = db.query("SELECT * FROM users")
for u in rows {
    println(u["id"], u["name"])
}
db.close()
```

---

## `http` Module

High-level backend HTTP server, router, response builders, and built-in middleware.

- `http.router()`: Creates REST router.
- `http.listen_and_serve(addr, router)`: Runs non-blocking server.
- `http.response(code, headers, body)`: Generic response builder.
- `http.json_response(code, data, headers)`: JSON response builder.
- `http.text_response(code, text, headers)`: Plain text response builder.
- `http.html_response(code, html, headers)`: HTML response builder.
- `http.file_response(base_dir, rel_path, headers)`: Static file server.
- `http.redirect(url, code = 302)`: HTTP redirect builder.
- `http.error(code, msg)`: Plain text error response.
- `http.json_error(code, msg)`: Structured JSON error response.
- `http.set_cookie(name, value, opts)`: Builds `Set-Cookie` header.
- `http.cors(opts)`: CORS middleware.
- `http.logger()`: Logger middleware.
- `http.secure_headers()`: Browser security headers.
- `http.request_id()`: Request ID tracing middleware.
- `http.rate_limiter(max_req, window_sec)`: IP rate limiter.

---

## `json` Module

- `json.encode(value: any) -> str`: Serializes any Whalli data structure into JSON text.
- `json.decode(json_str: str) -> (value: any, err: str | nil)`: Parses JSON text into Whalli maps, lists, numbers, and strings.

---

## `fs` Module

Synchronous file system operations:

- `fs.read(path: str) -> (content: str, err: str | nil)`
- `fs.write(path: str, data: str) -> (ok: bool, err: str | nil)`

---

## `os` Module

System utilities and environment variables:

- `os.args() -> list`: Command-line arguments.
- `os.env(key: str) -> str | nil`: Environment variable lookup.
- `os.set_env(key: str, val: str)`: Sets environment variable.
- `os.exit(code: int)`: Terminates process.

---

## `time` Module

- `time.now() -> float`: Epoch timestamp in seconds.
- `time.sleep(seconds: float | int)`: Non-blocking cooperative sleep.
- `time.after(seconds: float | int) -> chan`: Channel that fires after duration.

---

## `math` Module

Standard mathematical functions: `math.sqrt`, `math.pow`, `math.sin`, `math.cos`, `math.tan`, `math.floor`, `math.ceil`, `math.round`, `math.abs`, and constants `math.pi`, `math.e`.

---

## `crypto` Module

Cryptographic algorithms, hashing, random generation, and Base64/Hex utilities:

- `crypto.sha256(data: str | bytes) -> str`: Computes SHA-256 hash (hex string).
- `crypto.sha1(data: str | bytes) -> str`: Computes SHA-1 hash (hex string).
- `crypto.md5(data: str | bytes) -> str`: Computes MD5 hash (hex string).
- `crypto.hmac_sha256(key: str | bytes, message: str | bytes) -> str`: Computes HMAC-SHA256 signature.
- `crypto.base64_encode(data: str | bytes) -> str`: Base64 encodes input.
- `crypto.base64_decode(encoded_str: str) -> (data: bytes | nil, err: str | nil)`: Decodes Base64 to bytes.
- `crypto.hex_encode(data: bytes | str) -> str`: Encodes bytes into hexadecimal text.
- `crypto.hex_decode(hex_str: str) -> (data: bytes | nil, err: str | nil)`: Decodes hex string into bytes.
- `crypto.random_bytes(length: int = 16) -> bytes`: Generates cryptographically secure random bytes.
- `crypto.random_hex(length: int = 16) -> str`: Generates random hex token (length in bytes, output string len is 2 * length).
- `crypto.uuid4() -> str`: Generates random UUID version 4 string.

```whalli
import crypto

let password_hash = crypto.sha256("my_secret_pass")
let session_token = crypto.random_hex(32)
let request_uuid = crypto.uuid4()

let signature = crypto.hmac_sha256("api_secret", "action=pay&amount=100")
```
