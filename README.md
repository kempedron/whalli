<p align="center">
  <img src="whalli-vscode/icon.png" width="160" height="160" alt="Whalli Logo" />
</p>

<h1 align="center">Whalli Programming Language</h1>

<p align="center">
  <b>A lightweight, modern language with native cooperative multitasking, non-blocking I/O, and rich developer tooling.</b>
</p>

<p align="center">
  <a href="https://whalli.is-a.dev"><b>📚 Read the Book</b></a> •
  <a href="#key-features">Features</a> •
  <a href="#quick-start">Quick Start</a> •
  <a href="#language-tour">Language Tour</a> •
  <a href="#ide-support">IDE Support</a> •
  <a href="#architecture">Architecture</a> •
  <a href="#license">License</a>
</p>

<p align="center">
  <a href="https://whalli.is-a.dev"><img src="https://img.shields.io/badge/Book-whalli.is--a.dev-4a9c4a?style=flat-square" alt="Whalli Book" /></a>
  <a href="https://github.com/kempedron/whalli/actions/workflows/deploy-starlight.yml"><img src="https://github.com/kempedron/whalli/actions/workflows/deploy-starlight.yml/badge.svg" alt="Deploy Book" /></a>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Language-Rust_2024-orange.svg?style=flat-square" alt="Rust 2024" />
  <img src="https://img.shields.io/badge/Concurrency-Woroutines_%26_Channels-00c0f0.svg?style=flat-square" alt="Concurrency" />
  <img src="https://img.shields.io/badge/I%2FO-Non--blocking_MIO-38bdf8.svg?style=flat-square" alt="MIO I/O" />
  <img src="https://img.shields.io/badge/LSP-Enabled-green.svg?style=flat-square" alt="LSP Enabled" />
  <img src="https://img.shields.io/badge/License-MIT-blue.svg?style=flat-square" alt="License" />
</p>

---

## ⚡ Key Features

- 🐋 **Woroutines (`wo`) & Task Handles**: Lightweight cooperative green threads running on a work-stealing scheduler. Spawn thousands of concurrent tasks without kernel thread overhead; await results, monitor status, and isolate errors with `task.result()`.
- 📡 **Channels (`chan`) & `select`**: Safe message passing between woroutines (`ch <- val`, `<- ch`) with buffer capacity, channel close detection, and multiplexed `select` with `default` and timer cases.
- ⚡ **Asynchronous Non-blocking I/O**: High-performance TCP networking backed by [`mio`](https://github.com/tokio-rs/mio) OS event polling.
- 🧱 **Object-Oriented Constructs**: Data structures with `struct`, method bindings with `impl`, abstract contracts with `interface`, and runtime type introspection via `is`.
- 🧹 **Guaranteed Cleanup (`defer`)**: LIFO deferred statement execution upon function return or panic, ensuring mutex unlocks and resource cleanup.
- ❓ **Ergonomic Error Propagation (`?`)**: Idiomatic `(value, err)` tuples with the postfix `?` operator for clean early-return error handling.
- 📦 **Rich Standard Library**: First-class modules for HTTP backend & WebSockets (`http`), SQL databases (`sql` with SQLite, PostgreSQL, MySQL), cryptography (`crypto` with SHA-256, HMAC, UUIDv4), networking (`net`), HTTP client (`requests`), filesystem (`fs`), OS utilities (`os`), JSON (`json`), timing (`time`), synchronization (`sync`), and math (`math`).
- 🌐 **Modern Backend Web Framework**: Go `net/http`-level features including RESTful routing (`:id`, `*wildcard`), route grouping, HTTP/1.1 Keep-Alive, WebSocket support (RFC 6455), built-in middleware (`cors`, `logger`, `secure_headers`, `request_id`, `rate_limiter`), and static file serving.
- 🗄️ **Unified SQL Database Interface**: Single API (`sql.open`, `db.exec`, `db.query`, `db.query_row`, `db.close`) with bundled zero-dependency SQLite, plus PostgreSQL and MySQL support with automatic query placeholder translation.
- 🛠️ **Production-grade LSP (`whalli-lsp`) & VS Code Extension**: Built-in Language Server providing real-time diagnostics, semantic highlighting, inlay hints, symbol search, formatting, and auto-complete.

---

## 🚀 Quick Start

### Prerequisites

- [Rust & Cargo](https://rustup.rs/) (edition 2024 or later)

### Build and Run

Clone the repository and build the runtime:

```bash
git clone https://github.com/kempedron/whalli.git
cd whalli

# Build interpreter and language server in release mode
cargo build --release
```

Run a Whalli script:

```bash
./target/release/whalli main.wh
# or via Cargo:
cargo run --bin whalli -- main.wh
```

---

## 📖 Language Tour

### 1. Variables, Formatted Strings and Tuples

```whalli
let x = 42
let pi = 3.14159
let name = "Whalli"
let is_active = true

// Formatted strings (f-strings)
let greeting = f"Language: {name}, answer: {x}"
println(greeting)

// Multiline strings ("""...""") and raw strings (r"...")
let sql = """
SELECT id, title FROM tasks
"""
let regex = r"path\to\dir\d+"

// Byte literals (bytes) and raw bytes (br"...")
let raw = b"PING"
let (decoded, _) = raw.decode()
let raw_bytes = br"raw\x00data"

// Tuple, list, and object destructuring:
let (status, code) = (true, 200)
let [first, second] = [10, 20]
let { name: user_name, age } = {"name": "Alice", "age": 25}
println(status, code, first, second, user_name, age)
```

### 2. Functions, Errors and Defer

```whalli
import sync

let mu = sync.Mutex()

func calculate(a: int, b: int) -> (int, nil) {
    mu.lock()
    defer mu.unlock() // Guaranteed cleanup in LIFO order upon return

    return (a + b, nil)
}

let (sum, err) = calculate(10, 20)
println("Sum:", sum)
```

### 3. Woroutines, Task Handles & Channels

```whalli
import time

func compute(val: int) -> int {
    time.sleep(0.05)
    return val * 2
}

// Spawn woroutine returning a TaskHandle
let task = wo compute(21)
let (res, err) = task.result()
println("Task result:", res) // 42

// Channel communication & select
let ch = new(chan, 2)
ch <- "msg"

let selected = select {
    item <- ch => f"Got: {item}",
    <- time.after(1.0) => "Timeout",
    default => "Nothing ready",
}
println(selected)
```

### 4. Full-Featured HTTP Backend & SQLite Database

```whalli
import http
import sql

// 1. Database setup (bundled zero-config SQLite, PostgreSQL, or MySQL)
let (db, _) = sql.open("sqlite", "app.db")
db.exec("CREATE TABLE IF NOT EXISTS tasks (id INTEGER PRIMARY KEY AUTOINCREMENT, title TEXT, done BOOLEAN)")

// 2. High-performance RESTful router with Middleware
let router = http.router()
router.use(http.cors())           // Automatic CORS & Preflight handling
router.use(http.logger())         // Structured request logging
router.use(http.secure_headers()) // Security headers (X-Frame-Options, CSP, etc.)

// 3. Grouped API routes with Keep-Alive by default
let api = router.group("/api")

api.get("/tasks", func(req) {
    let (rows, _) = db.query("SELECT * FROM tasks ORDER BY id DESC")
    return http.json_response(200, rows)
})

api.post("/tasks", func(req) {
    let body = req.json()
    let (res, err) = db.exec("INSERT INTO tasks (title, done) VALUES (?, ?)", [body["title"], false])
    if err != nil {
        return http.json_error(400, "Failed to create task")
    }
    return http.json_response(201, {"id": res["last_insert_id"], "title": body["title"]})
})

api.get("/tasks/:id", func(req) {
    let (task, _) = db.query_row("SELECT * FROM tasks WHERE id = ?", [req.param("id")])
    if task == nil {
        return http.json_error(404, "Task not found")
    }
    return http.json_response(200, task)
})

// 4. Static file serving with MIME detection & path traversal safety
router.static("/public", "./static")

// 5. Start non-blocking server
http.listen_and_serve(":8080", router)
```

### 5. HTTP Client (`requests`)

```whalli
import requests

let res = requests.get("https://httpbin.org/get", {"timeout": 5})
if res.ok {
    println("Status:", res.status_code)
    let data = res.json()
}
```

### 6. WebSockets (RFC 6455)

```whalli
import http

let router = http.router()

router.get("/ws", func(req) {
    let (ws, err) = http.upgrade(req)
    if err != nil {
        return http.text_response(400, err)
    }

    while true {
        let (msg, r_err) = ws.read()
        if msg != nil {
            ws.send(f"Echo: {msg}")
        }
        if r_err != nil and r_err != "WouldBlock" {
            break
        }
    }
    ws.close()
    return nil
})

http.listen_and_serve(":8080", router)
```

### 7. Raw TCP Networking (MIO)

```whalli
import net

let (server_id, err) = net.listen(8080)
println("Listening on http://127.0.0.1:8080 ...")

while true {
    let client_id = net.accept(server_id)
    if client_id != nil {
        let (request, _) = net.read(client_id)
        println("Request received:", request)

        let response = "HTTP/1.1 200 OK\r\nContent-Length: 13\r\n\r\nHello Whalli!"
        net.write(client_id, response)
        net.close(client_id)
    }
}
```

### 8. Structs, Methods & Interfaces

```whalli
struct Vector2 {
    x: int,
    y: int
}

impl Vector2 {
    func norm_sq() -> int {
        return this.x * this.x + this.y * this.y
    }
}

interface Printable {
    func str()
}

let v = Vector2(3, 4)
println("Norm squared:", v.norm_sq()) // 25
```

### 9. Collections & Ranges

```whalli
// Lists
let arr = new(list)
arr.push(10)
arr.push(20)
println("List len:", arr.len(), "First item:", arr[0])

// Dictionaries (Maps)
let user = new(map)
user["username"] = "kepr"
user["role"] = "admin"
println("Keys:", user.keys())

// Numeric ranges
for i in range(0, 10, 2) {
    print(i, " ") // 0 2 4 6 8
}
println()
```

### 10. Module Exports & Selective Imports

```whalli
// In ./math_utils.wh
pub let pi = 3.14159
pub func add(a: int, b: int) -> int {
    return a + b
}

// In main.wh
// Whole module import:
import "./math_utils.wh" as math_utils

// Selective symbol import via 'from':
from "./math_utils.wh" import pi, add as sum_fn
from math import sin

println(sum_fn(10, 20), sin(pi / 2))
```

---

## 💻 IDE Support

Whalli comes with official tooling for **VS Code** and **VSCodium**:

<p align="center">
  <img src="https://raw.githubusercontent.com/kempedron/whalli/main/whalli-vscode/icon.png" width="96" height="96" alt="Whalli Extension" />
</p>

### Features in the Editor:
- **▶️ One-Click Run**: Run scripts instantly with the title bar button or `Ctrl+F5`.
- **IntelliSense & Auto-complete**: Built-ins, stdlib modules (`http.`, `sql.`, `requests.`, `net.`, `fs.`, `time.`, `math.`), and user symbols.
- **Hover Documentation**: Signatures, parameters, and executable code examples.
- **Real-time Diagnostics**: Syntax and semantic error checks as you type.
- **Inlay Hints**: Inline type annotations for inferred variables (`let x = 42` displays `: int`).
- **Rename Symbol (`F2`)**: Project-wide identifier refactoring.
- **Go to Definition (`F12`)**: Instant navigation to definitions.
- **Document Formatting (`Shift+Alt+F`)**: Clean automatic indentation and formatting.

### Installing the Extension:
- **Open VSX (VSCodium)**: Search for `whalli-lang` in Extensions.
- **VS Code Marketplace**: Search for `whalli-lang` or install the packaged `.vsix` from `whalli-vscode/`.

---

## 🏗️ Architecture

```text
               ┌──────────────────────┐
               │  Source Code (.wh)   │
               └──────────┬───────────┘
                          │
                          ▼
               ┌──────────────────────┐
               │    Lexer & Parser    │ ◄─── whalli-lsp (Language Server)
               └──────────┬───────────┘
                          │ AST
                          ▼
               ┌──────────────────────┐
               │   Bytecode Compiler  │
               └──────────┬───────────┘
                          │ Bytecode Chunks
                          ▼
         ┌───────────────────────────────────┐
         │     Stack-based Virtual Machine   │
         │ ┌───────────────┐ ┌─────────────┐ │
         │ │ Woroutines    │ │  MIO Poller │ │
         │ │ Scheduler     │ │ (EventLoop) │ │
         │ └───────────────┘ └─────────────┘ │
         │ ┌───────────────┐ ┌─────────────┐ │
         │ │ Heap & GC     │ │ Stdlib APIs │ │
         │ └───────────────┘ └─────────────┘ │
         └───────────────────────────────────┘
```

1. **Lexer (`src/lexer.rs`)**: Tokenizes source text, handles byte literals (`b"..."`), string interpolation, and lexical tokens.
2. **Parser (`src/parser.rs`)**: Recursive-descent parser producing strongly-typed Abstract Syntax Trees (`src/ast.rs`).
3. **Compiler (`src/compiler.rs`)**: Emits compact stack-based bytecode instructions (`src/opcode.rs`).
4. **VM (`src/vm.rs`)**: Stack-based execution engine with cooperative work-stealing fiber scheduler and non-blocking polling integration via `mio`.
5. **LSP Server (`src/bin/whalli-lsp.rs`)**: Standalone language server communicating via JSON-RPC over `stdio`.

---

## 📚 Documentation

The documentation is built with Astro Starlight and hosted at [whalli.is-a.dev](https://whalli.is-a.dev):
- **English**: [whalli.is-a.dev](https://whalli.is-a.dev)
- **Русский**: [whalli.is-a.dev/ru/](https://whalli.is-a.dev/ru/)

Covers Quick Start, Language Tour, Concurrency, I/O & Networking, Standard Library, Toolchain, Tooling, and Runnable Examples.

---

## 📄 License

This project is licensed under the [MIT License](LICENSE).
