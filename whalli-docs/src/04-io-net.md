# I/O and Networking

> Source: `src/stdlib/net.rs`, `src/stdlib/http.rs`, `src/stdlib/requests.rs`, `src/vm.rs:69-75`, `src/value.rs`.

## Non-blocking Core

- `NetworkState` (`vm.rs:69-75`): `Poll`, `listeners: HashMap<Token, TcpListener>`, `streams: HashMap<Token, TcpStream>`, `next_token: AtomicUsize`, `closed_tokens`.
- All sockets are `mio::net::{TcpListener, TcpStream}` registered with `Interest::READABLE | WRITABLE` (`net.rs:28-32, 164-169`). Non-blocking: `WouldBlock` returns `NativeResult::SuspendIO(Token)` (`net.rs:38-39`, `vm.rs:1003-1019`), scheduler parks task in `waiting_io_tasks` and timer/IO thread re-wakes on `Poll` event (`vm.rs:457-462, 488-514`).

## `net` Module

`import net` (`net.rs:128-442`):

| Function | Signature | Description |
|---|---|---|
| `listen` | `listen(port: int, host: str = "127.0.0.1") -> (server_id: int, err: str\|nil)` | Binds `TcpListener`, registers `READABLE` (`net.rs:131-180`). |
| `connect` | `connect(host: str, port: int) -> (client_id: int, err)` | Connects `TcpStream` (`net.rs:182-223`). |
| `accept` | `accept(server_id: int) -> client_id \| nil` | Non-blocking accept; `WouldBlock` → `SuspendIO` (`net.rs:12-47`). |
| `read` | `read(client_id: int, max_bytes: int = 4096) -> (data: str\|nil, err)` | Reads up to 4096 (capped 1..65536) (`net.rs:228-278`). EOF → `(nil, nil)` and stream removed. |
| `read_until` | `read_until(client_id: int, delimiter: str = "\r\n\r\n") -> (data, err)` | Loops until delimiter (`net.rs:280-349`). |
| `write` | `write(client_id, str) -> (ok: bool, err)` | Single `write` syscall (`net.rs:351-388`). |
| `write_all` | `write_all(client_id, str) -> (ok, err)` | Loops until fully written (`net.rs:49-90`). |
| `peer_addr` | `peer_addr(client_id) -> str\|nil` | |
| `local_addr` | `local_addr(id) -> str\|nil` | Works for listener or stream (`net.rs:409-432`). |
| `set_nodelay` | `set_nodelay(client_id, bool) -> bool` | (`net.rs:92-104`) |
| `close` | `close(id: int) -> bool` | Deregisters from Poll, pushes to `closed_tokens` (`net.rs:106-126`). |

Raw TCP example (from `README.md:129-148`):
```whalli
import net
let (server_id, err) = net.listen(8080)
while true {
    let client_id = net.accept(server_id)
    if client_id != nil {
        let (request, _) = net.read(client_id)
        net.write(client_id, "HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nHello")
        net.close(client_id)
    }
}
```

## `http` Module

`import http` (`http.rs`) — high-level backend HTTP server and REST framework over `net`.

### Server Functions and Constants

| Function / Constant | Signature / Value | Description |
|---|---|---|
| `listen` | `listen(addr: str\|int) -> (server_id, err)` | Supports `":8080"`, `"127.0.0.1:8080"`, `8080`. |
| `read_request` | `read_request(client_id) -> (req: map\|nil, err)` | Reads until headers + `Content-Length` satisfied. |
| `parse_request` | `parse_request(raw: str) -> (req, err)` | Parses raw HTTP request text via `httparse` + URL decoder. |
| `response` | `response(code: int, headers: map\|nil, body: str) -> str` | Formats generic HTTP/1.1 response string. |
| `json_response` | `json_response(code: int, data: any, headers: map\|nil) -> str` | Automatic `Content-Type: application/json` + serialization. |
| `text_response` | `text_response(code: int, text: str, headers: map\|nil) -> str` | `Content-Type: text/plain; charset=utf-8`. |
| `html_response` | `html_response(code: int, html: str, headers: map\|nil) -> str` | `Content-Type: text/html; charset=utf-8`. |
| `file_response` | `file_response(base_dir: str, rel_path: str = "", headers: map\|nil) -> str` | Static file serving with MIME detection & traversal safety (`..` -> 403). |
| `redirect` | `redirect(url: str, code: int = 302, headers: map\|nil) -> str` | Builds HTTP redirect with `Location` header. |
| `error` | `error(code: int = 500, message: str = "") -> str` | Builds text error response with status code. |
| `json_error` | `json_error(code: int = 500, message: str = "") -> str` | Builds standard JSON error `{"error": msg, "code": code}`. |
| `set_cookie` | `set_cookie(name: str, value: str, opts: map\|nil) -> str` | Builds `Set-Cookie` header value with path, domain, max_age, http_only, secure, same_site. |
| `status_text` | `status_text(code: int) -> str` | Returns standard HTTP reason string for code. |
| `allowed_methods` | `allowed_methods(router, path: str) -> list[str]` | Inspects router and returns allowed HTTP methods for a path. |
| `router` | `router() -> Router (map)` | Creates new router with route registry, middlewares, and groups. |
| `match_route` | `match_route(router, method: str, path: str) -> (handler\|nil, params\|nil)` | Exact, `:param`, and `*filepath` wildcard matching. |
| `listen_and_serve` | `listen_and_serve(addr, handler: Router\|func) -> (ok, err)` | Loops `accept`, spawns concurrent `wo _serve_client` per connection. |
| `accept/write_all/set_nodelay/close` | Re-exported from `net` | For self-contained server execution. |
| Status Constants | `http.STATUS_OK` (200), `http.STATUS_CREATED` (201), `http.STATUS_NO_CONTENT` (204), `http.STATUS_BAD_REQUEST` (400), `http.STATUS_UNAUTHORIZED` (401), `http.STATUS_FORBIDDEN` (403), `http.STATUS_NOT_FOUND` (404), `http.STATUS_METHOD_NOT_ALLOWED` (405), `http.STATUS_INTERNAL_SERVER_ERROR` (500) | Standard HTTP status code constants. |

### Request Map and Methods

`parse_http_request_bytes` (`http.rs`) produces `req`:

```whalli
{
  "method": "POST",
  "url": "/api/users/42?active=true",
  "path": "/api/users/42",
  "query": {"active": "true"},      // URL-decoded query map
  "headers": {"content-type": "..."},// lowercase keys
  "cookies": {"session": "xyz"},    // parsed from Cookie header
  "form": {"field": "val"},         // parsed application/x-www-form-urlencoded
  "body": str,
  "proto": "HTTP/1.1",
  "remote_addr": "127.0.0.1:54321", // Client socket address
}
```

Methods on `req`:
- `req.json()`: parses `req["body"]` via `serde_json` into Whalli structures.
- `req.header(key, default = nil)`: case-insensitive header lookup.
- `req.cookie(name, default = nil)`: cookie lookup.
- `req.query(key, default = nil)`: query string parameter lookup.
- `req.param(key, default = nil)`: URL route parameter lookup (e.g. `:id` or `*filepath`).
- `req.form()`: returns parsed urlencoded form map.
- `req.form_value(key, default = nil)`: urlencoded form field lookup.

### Routing & Middleware Features

- **Route registration**: `router.get(path, handler)`, `router.post(path, handler)`, `router.put(...)`, `router.delete(...)`, `router.patch(...)`, `router.head(...)`, `router.options(...)`, `router.handle(method, path, handler)`.
- **Parameter matching**: `:name` segment parameters (e.g. `/api/users/:id`), extracted into `req["params"]["id"]` or `req.param("id")`.
- **Catch-all wildcards**: `*filepath` matching (e.g. `/static/*filepath`), matches trailing subpath.
- **Middlewares**: `router.use(func(req))` registers global or group middlewares. Middleware returning non-nil response halts execution and sends that response immediately. Out-of-the-box middlewares:
  - `http.cors(opts)`: Handles `OPTIONS` preflight with 204 and adds `Access-Control-*` headers.
  - `http.logger()`: Logs formatted request lines `[METHOD] /path (from IP)`.
  - `http.secure_headers()`: Automatically sets browser security headers (`X-Frame-Options`, `nosniff`, `XSS`, `CSP`).
  - `http.request_id(header_name)`: Traces request IDs via `req["id"]` and output header `X-Request-ID`.
  - `http.rate_limiter(max_req, window_sec)`: Sliding-window rate limiter returning 429 Too Many Requests.
- **Route groups**: `let api = router.group("/api")`, `let v1 = api.group("/v1")`. Groups inherit prefix and middlewares.
- **Static file serving**: `router.static(url_prefix, directory_path)` registers automatic static file serving with MIME detection and path traversal protection (`..` -> 403).
- **Keep-Alive**: All HTTP responses default to HTTP/1.1 `Connection: keep-alive` (with `Keep-Alive: timeout=5, max=100`) and handle `Connection: close` requests appropriately.
- **Custom Error Handlers**:
  - `router.not_found(func(req))` sets custom 404 handler (e.g. returning JSON).
  - `router.method_not_allowed(func(req))` sets custom 405 handler (automatic `Allow` header with permitted methods).
- **Graceful shutdown**: `http.close(router.server_id)` closes the listener socket.

## `requests` Module (Client)

`import requests` (`requests.rs:271-382`) — blocking HTTP client via `ureq` (synchronous, not via `mio`; blocks worker thread).

Functions: `get`, `post`, `put`, `delete`, `patch`, `head`, `request(method, url, opts)`:

```whalli
import requests
let r = requests.get("http://127.0.0.1:8080/api/tasks", {"timeout": 3})
let r2 = requests.post("http://127.0.0.1:8080/api/tasks", {
    "headers": {"User-Agent": "whalli"},
    "json": {"title": "Buy milk"},
    "timeout": 3
})
```

Options map (`requests.rs:9-113`):
- `params: map` — query string appended.
- `headers: map` — extra headers.
- `json: any` — serialized as JSON, `Content-Type: application/json`.
- `data: str|map` — else form-encoded if map, else raw string.
- `timeout: int|float` seconds (default 30), `allow_redirects: bool` (default true).

Response map:
```
{ "status_code": int, "status_text": str, "ok": bool (200..399), "text": str,
  "headers": map[lowercase], "url": str, "content_length": int, "error": str|nil }
```
Methods on response map: `r.json()` — same as `req.json()`, parses `r["text"]` (`value.rs:313`).

Error case: `status_code==0`, `ok==false`, `error` set (`requests.rs:127-142`).
