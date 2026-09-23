# Examples

## RESTful Tasks API (`main.wh`)

Complete runnable API. See `main.wh:1-171` for annotated source.

```bash
cargo run --bin whalli -- main.wh
# server prints:
# Whalli Tasks REST API listening on http://127.0.0.1:8888
```

Endpoints (`http.router` + `http.listen_and_serve` from `src/stdlib/http.rs`):

| Method | Path | Handler | Response |
|---|---|---|---|
| GET | `/api/health` | `health` | `200 {"status":"ok"}` |
| GET | `/api/tasks` | `list_tasks` | `200 [task...]` |
| GET | `/api/tasks/:id` | `get_task` | `200 task` or `404` |
| POST | `/api/tasks` | `create_task` | `201 task`, body `{"title":str}` |
| PUT | `/api/tasks/:id` | `update_task` | `200 task`, body `{"title"?, "done"?}` |
| DELETE | `/api/tasks/:id` | `delete_task` | `200 {"deleted": id}` or `404` |

Store: `new(map)` guarded by `sync.Mutex` (`src/stdlib/sync.rs`), `next_id` auto-increment, `"" + next_id` keys (avoid `str()` / `f"{x}"` single-interpolation — see Known Limitations).

### Try it (port `8888`)

```bash
curl http://127.0.0.1:8888/api/health
curl http://127.0.0.1:8888/api/tasks
curl -X POST http://127.0.0.1:8888/api/tasks -H "Content-Type: application/json" -d '{"title":"Buy milk"}'
curl http://127.0.0.1:8888/api/tasks/1
curl -X PUT http://127.0.0.1:8888/api/tasks/1 -H "Content-Type: application/json" -d '{"done":true}'
curl -X DELETE http://127.0.0.1:8888/api/tasks/1
# 404 check
curl http://127.0.0.1:8888/api/tasks/1
```

Corresponding test that exercises the same path: `tests/http_server_and_requests_integration_test.rs:6-119` (woroutine server + `requests` client).

> **Known VM limitations (workarounds in `main.wh`):**
> - `str()`, `int()`, `bool()` globals are shadowed by `Value::Type` (`vm.rs:329`) — use `"" + x` for int→str and direct `== true/false` for bool.
> - `f"{x}"` single-interpolation returns the raw value, not a string — use `"" + x` or `f" {x}"`.
> - `for ... in` / `while` with many locals + `mu.lock()` can trigger `IterNext` / `LoadLocal` OOB panics — `main.wh:list_tasks` uses `while i < keys.len()` without filter locals.

Corresponding test that exercises the same path: `tests/http_server_and_requests_integration_test.rs:6-119` (woroutine server + `requests` client).

---

## Woroutines and Channels

```whalli
func producer(ch) {
    for i in range(1, 4) {
        time.sleep(0.5)
        ch <- f"task #{i}"
    }
}
let ch = new(chan, 3)
wo producer(ch)
println(<- ch)
println(<- ch)
println(<- ch)
```

---

## Raw TCP Echo

```whalli
import net
let (server_id, err) = net.listen(8080)
println("Listening on http://127.0.0.1:8080 ...")
while true {
    let client_id = net.accept(server_id)
    if client_id != nil {
        let (request, _) = net.read(client_id)
        println("Request:", request)
        net.write(client_id, "HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nHello")
        net.close(client_id)
    }
}
```

---

## Structs and Interfaces

```whalli
struct Vector2 { x: int, y: int }
impl Vector2 {
    func norm_sq() -> int { return this.x * this.x + this.y * this.y }
}
interface Printable { func str() }
let v = Vector2(3, 4)
println(v.norm_sq())
```

---

## Select with Timeout

```whalli
import time
let ch = new(chan, 1)
wo func() { time.sleep(0.2); ch <- "ready" }()
let r = select {
    val <- ch => val,
    <- time.after(1.0) => "timeout",
}
println(r)
```

---

## Importing Local Files

`math_utils.wh`:
```whalli
func add(a: int, b: int) -> int { return a + b }
struct Point { x: int, y: int }
```
`test_import.wh`:
```whalli
import "./math_utils.wh"
println(add(15, 27))
let p = Point(100, 200)
println(p)
```

Run: `cargo run --bin whalli -- test_import.wh`.

---

## `requests` Client

```whalli
import requests
let r = requests.get("https://httpbin.org/get", {"timeout": 5})
if r.ok { println(r.json()) }

let r2 = requests.post("https://httpbin.org/post", {"json": {"name": "Alice"}})
println(r2.status_code, r2.text)
```
