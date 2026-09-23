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

- 🐋 **Woroutines (`wo`)**: Lightweight cooperative green threads. Spawn thousands of concurrent tasks without kernel thread overhead.
- 📡 **Channel Primitives (`chan`)**: Safe message passing between woroutines (`ch <- value`, `<- ch`) with customizable buffer capacity.
- ⚡ **Asynchronous Non-blocking I/O**: High-performance TCP networking backed by [`mio`](https://github.com/tokio-rs/mio) OS event polling.
- 🧱 **Object-Oriented Constructs**: Data structures with `struct`, method bindings with `impl`, abstract contracts with `interface`, and runtime type introspection via `is`.
- 📦 **Built-in Standard Library**: First-class modules for networking (`net`), modern HTTP server & REST framework (`http`), database clients (`sql` with SQLite, PostgreSQL, MySQL), file operations (`fs`), timing (`time`), requests client (`requests`), concurrency sync (`sync`), and mathematics (`math`).
- 🌐 **Modern Backend Web Framework**: Go `net/http`-level features including RESTful routing (`:id`, `*wildcard`), route grouping, HTTP/1.1 persistent connections (`Keep-Alive`), built-in middleware (`cors`, `logger`, `secure_headers`, `request_id`, `rate_limiter`), static file serving with MIME detection, and error builders.
- 🗄️ **Unified SQL Database Interface**: Single API (`sql.open`, `db.exec`, `db.query`, `db.query_row`, `db.close`) with bundled zero-dependency SQLite, plus PostgreSQL and MySQL support with automatic query placeholder translation.
- 🛠️ **Production-grade LSP (`whalli-lsp`)**: Built-in Language Server providing real-time diagnostics, semantic highlighting, parameter hints, symbol search, formatting, and auto-complete.
- 🎨 **Official VS Code / VSCodium Extension**: One-click run, syntax highlighting, and full IDE integration.

---

## 🚀 Quick Start

### Prerequisites

- [Rust & Cargo](https://rustup.rs/) (edition 2024 or later)

### Build and Run

Clone the repository and build the runtime:

```bash
git clone https://github.com/kempedron/whalli.git
cd whalli

# Build both interpreter and language server in release mode
cargo build --release
```

Run a Whalli script:

```bash
./target/release/whalli main.wh
```

Or run via Cargo directly:

```bash
cargo run --bin whalli -- main.wh
```

---

## 📖 Language Tour

### 1. Variables and Types

```whalli
// Dynamic typing with explicit type conversions
let x = 42
let pi = 3.14159
let name = "Whalli"
let is_active = true

// Formatted strings
let greeting = f"Language: {name}, answer: {x}"
println(greeting)

// Tuple binding and destructuring
let (status, code) = (true, 200)
println(status, code)
```

### 2. Functions & Signatures

```whalli
// Functions with optional type annotations and return type
func calculate(a: int, b: int) -> int {
    return a + b
}

let result = calculate(10, 20)
println("Sum:", result)
```

### 3. Woroutines & Channels (Concurrency)

```whalli
func producer(ch) {
    for i in range(1, 4) {
        time.sleep(0.5) // non-blocking sleep!
        ch <- f"task #{i}"
    }
}

// Allocate a buffered channel
let ch = new("chan", 3)

// Spawn asynchronous lightweight worker
wo producer(ch)

// Receive messages
println(<- ch)
println(<- ch)
println(<- ch)
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

### 5. Raw TCP Networking (MIO)

```whalli
import net

// Bind TCP server on port 8080
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

### 6. Structs, Methods & Interfaces

```whalli
struct Vector2 {
    x: int,
    y: int,
}

impl Vector2 {
    func norm_sq() -> int {
        return this.x * this.x + this.y * this.y
    }
}

interface Printable {
    func str()
}

let v = new("Vector2")
v.x = 3
v.y = 4
println("Vector:", v.x, v.y)
```

### 7. Collections & Ranges

```whalli
// Lists
let arr = new("list")
arr.push(10)
arr.push(20)
println("List len:", arr.len(), "First item:", arr[0])

// Dictionaries (Maps)
let user = new("map")
user["username"] = "kepr"
user["role"] = "admin"
println("Keys:", user.keys())

// Numeric ranges
for i in range(0, 10, 2) {
    print(i, "") // 0 2 4 6 8
}
println()
```

---

## 💻 IDE Support

Whalli comes with official tooling for **VS Code** and **VSCodium**:

<p align="center">
  <img src="https://raw.githubusercontent.com/kempedron/whalli/main/whalli-vscode/icon.png" width="96" height="96" alt="Whalli Extension" />
</p>

### Features in the Editor:
- **▶️ One-Click Run**: Run scripts instantly with the title bar button or `Ctrl+F5`.
- **IntelliSense & Auto-complete**: Built-ins, stdlib modules (`net.`, `fs.`, `time.`, `math.`), and user symbols.
- **Hover Documentation**: Signatures, parameters, and executable code examples.
- **Real-time Diagnostics**: Syntax and semantic error checks as you type.
- **Inlay Hints**: Inline type annotations for inferred variables (`let x = 42` displays `: int`).
- **Rename Symbol (`F2`)**: Project-wide identifier refactoring.
- **Go to Definition (`F12`)**: Instant navigation to definitions.
- **Document Formatting (`Shift+Alt+F`)**: Clean automatic indentation and formatting.

### Installing the Extension:
- **Open VSX (VSCodium)**: Search for `whalli-lang` in Extensions.
- **VS Code Marketplace**: Search for `whalli-lang` or install the packaged `.vsix`.

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

1. **Lexer (`src/lexer.rs`)**: Tokenizes source text, handles string interpolation and lexical validation.
2. **Parser (`src/parser.rs`)**: Recursive-descent parser producing strongly-typed Abstract Syntax Trees (`src/ast.rs`).
3. **Compiler (`src/compiler.rs`)**: Emits compact stack-based bytecode instructions (`src/opcode.rs`).
4. **VM (`src/vm.rs`)**: Stack-based execution engine with cooperative fiber scheduler and non-blocking polling integration via `mio`.
5. **LSP Server (`src/bin/whalli-lsp.rs`)**: Standalone language server communicating via JSON-RPC over `stdio`.

---

## 📚 Documentation

- **Starlight Book (primary):** [whalli.is-a.dev](https://whalli.is-a.dev) — 8 chapters covering Quick Start, Language Tour, Concurrency, I/O & Networking (http Keep-Alive, middleware, sql), Stdlib, Toolchain, Tooling and Examples. Source in `whalli-book/`, auto-deployed to GitHub Pages via `.github/workflows/deploy-starlight.yml` on push to `main`. Custom domain `whalli.is-a.dev` via `is-a.dev` (`whalli-book/public/CNAME`).
- **mdBook (legacy):** `whalli-docs/` (`book.toml`) — kept for reference, build locally with `mdbook build whalli-docs` → `whalli-docs/book/`.

To register/renew the `is-a.dev` domain: fork [is-a-dev/register](https://github.com/is-a-dev/register), add `domains/whalli.json`:
```json
{"owner":{"username":"kempedron","email":"kempedron@gmail.com"},"record":{"CNAME":"kempedron.github.io"},"proxied":false}
```
Then enable `Settings > Pages > Custom domain: whalli.is-a.dev` + `Enforce HTTPS` after first deploy.

---

## 📄 License

This project is licensed under the [MIT License](LICENSE).
