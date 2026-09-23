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

`import http` (`http.rs:217-767`) — higher-level HTTP over `net`.

### Server

| Function | Signature |
|---|---|
| `listen` | `listen(addr: str\|int) -> (server_id, err)` — `parse_addr_str` supports `":8080"`, `"127.0.0.1:8080"`, `8080` (`http.rs:16-49, 220-267`). |
| `read_request` | `read_request(client_id) -> (req: map\|nil, err)` — reads until headers + `Content-Length` bytes satisfied (`http.rs:269-364`). |
| `parse_request` | `parse_request(raw: str) -> (req, err)` — uses `httparse` + `url_decode` (`http.rs:366-387, 134-154`). |
| `response` | `response(code: int, headers: map\|nil, body: str) -> str` |
| `json_response` | `json_response(code: int, data: any, headers: map\|nil) -> str` — `Content-Type: application/json` |
| `text_response` | `text_response(code: int, text: str, headers) -> str` — `text/plain; charset=utf-8` |
| `html_response` | `html_response(code: int, html: str, headers) -> str` — `text/html; charset=utf-8` |
| `status_text` | `status_text(code: int) -> str` — table `http.rs:51-84` |
| `router` | `router() -> Router (map)` — creates `{ routes: { "GET": {}, ... } }` (`http.rs:673-691`) |
| `match_route` | `match_route(router, method: str, path: str) -> (handler\|nil, params\|nil)` — exact + `:param` segment matching (`http.rs:583-670`) |
| `listen_and_serve` | `listen_and_serve(addr, handler: Router|func) -> (ok, err)` — loops `accept`, spawns `wo _serve_client` per connection (`http.rs:698-741`) |
| `accept/write_all/set_nodelay/close` | re-exported from `net` for self-contained server (`http.rs:693-696`) |

### Request Map

`parse_http_request_bytes` (`http.rs:156-215`) produces map:

```
{
  "method": "GET",
  "url": "/api/users/42?active=true",
  "path": "/api/users/42",
  "query": map["active": "true"],   // URL-decoded
  "headers": map[lowercase keys],
  "body": str,
  "proto": "HTTP/1.1",
}
```

Added `json()` method: `req.json()` parses `req["body"]` (or `req["text"]`) via `serde_json` (`value.rs:313-334`, `http_test.rs:74`).

### Response Builder

`build_http_response` (`http.rs:86-132`): `HTTP/1.1 <code> <reason>\r\nContent-Length: <n>\r\nConnection: close\r\n[Content-Type]\r\n[custom headers]\r\n\r\n<body>`. Custom headers override `Content-Type` if `content-type` key present (case-insensitive).

### Routing

- Registration via `map` method sugar (`value.rs:336-389`): `router.get(path, handler)` etc. `router.handle(method, path, handler)` for arbitrary method.
- Matching: first exact `path_routes.get(target)`, else segment-wise `:param` (`http.rs:608-648`). `params` map values are `Str`.
- Handler signature: `func(req) -> http_response_str`. Called inside `_whalli_http_serve_client` (`http.rs:699-723`): sets `req["params"]` if present, invokes handler, `write_all` response, `close`.

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
