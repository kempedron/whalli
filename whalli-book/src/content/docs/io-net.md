---
title: I/O and Networking
description: Non-blocking TCP sockets, modern HTTP backend framework, Keep-Alive, and requests.
---

## Overview

Whalli's networking is non-blocking and integrated directly with the cooperative scheduler via [`mio`](https://github.com/tokio-rs/mio). When an I/O operation returns `WouldBlock`, the VM suspends the active woroutine and re-schedules it as soon as the operating system signals socket readiness.

---

## `http` Module

The `http` module is a high-performance backend framework modeled after Go's `net/http` and Gin.

### Starting a Server

```whalli
import http

let router = http.router()

router.get("/hello", func(req) {
    return http.json_response(200, {"message": "Hello from Whalli!"})
})

// Starts non-blocking server on port 8080 with Keep-Alive enabled
http.listen_and_serve(":8080", router)
```

### HTTP/1.1 Keep-Alive

All responses default to persistent connections (`Connection: keep-alive` with `Keep-Alive: timeout=5, max=100`). If a client sends `Connection: close` (or HTTP/1.0 without keep-alive), the server automatically flushes the response and terminates the socket connection.

### Request (`req`) Object

`req` provides rich ergonomic helpers for backend handlers:

| Method / Property | Description |
|---|---|
| `req["method"]` | HTTP Method (`"GET"`, `"POST"`, etc.) |
| `req["path"]` | Normalized URL path (`"/api/tasks"`) |
| `req["remote_addr"]` | Client socket address (`"127.0.0.1:54321"`) |
| `req.header(key, default = nil)` | Case-insensitive header lookup |
| `req.cookie(name, default = nil)` | Parsed cookie lookup from `Cookie` header |
| `req.query(key, default = nil)` | URL query string parameter |
| `req.param(key, default = nil)` | Route parameter (`:id` or `*filepath`) |
| `req.json()` | Parses request body via `serde_json` |
| `req.form()` | Parsed `application/x-www-form-urlencoded` map |
| `req.form_value(key, default = nil)` | Form parameter lookup |

### REST Routing & Groups

```whalli
let router = http.router()

// Route parameters
router.get("/users/:id", func(req) {
    let id = req.param("id")
    return http.json_response(200, {"user_id": id})
})

// Catch-all wildcards
router.get("/static/*filepath", func(req) {
    let path = req.param("filepath")
    return http.text_response(200, f"Serving: {path}")
})

// Route Groups
let api = router.group("/api")
let v1 = api.group("/v1")
v1.get("/status", func(req) { return http.json_response(200, {"ok": true}) })
```

### Built-in Middlewares

Middlewares can be attached globally (`router.use(...)`) or to route groups (`group.use(...)`):

```whalli
import http

let router = http.router()

// 1. CORS Middleware (handles preflight OPTIONS and Access-Control-* headers)
router.use(http.cors({
    "origin": "https://myapp.com",
    "methods": "GET, POST, PUT, DELETE",
    "credentials": true
}))

// 2. Structured Request Logger
router.use(http.logger())

// 3. Security Headers (X-Frame-Options, nosniff, CSP)
router.use(http.secure_headers())

// 4. Request ID tracing (attaches X-Request-ID)
router.use(http.request_id())

// 5. Rate Limiter (sliding window per remote IP)
router.use(http.rate_limiter(100, 60)) // 100 requests per 60 seconds
```

### Static File Serving

```whalli
// Safely serves files from directory with MIME detection and path traversal protection
router.static("/public", "./static_files")
```

---

## `net` Module (Raw TCP)

Low-level non-blocking sockets:

```whalli
import net

let (server_id, err) = net.listen(9000)
while true {
    let client_id = net.accept(server_id)
    if client_id != nil {
        let (data, _) = net.read(client_id)
        net.write_all(client_id, "Echo: " + data)
        net.close(client_id)
    }
}
```

---

## `requests` Module (Client)

High-level synchronous HTTP/HTTPS client:

```whalli
import requests

let r = requests.get("https://httpbin.org/get", {"timeout": 5})
if r.ok {
    println("Status:", r.status_code)
    let json_data = r.json()
}

let r2 = requests.post("https://httpbin.org/post", {
    "json": {"name": "Alice", "role": "admin"}
})
```
