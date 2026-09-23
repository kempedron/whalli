---
title: Quick Start
description: How to install, build, and run Whalli programs.
---

## Prerequisites

- **Rust & Cargo** (`edition 2024` or later). Install via [rustup.rs](https://rustup.rs).
- Linux / macOS (tested on Linux with non-blocking MIO event loop).

## Build from Source

```bash
git clone https://github.com/kempedron/whalli.git
cd whalli

# Builds both interpreter and language server in release mode
cargo build --release
```

Output binaries:
- `target/release/whalli` — Bytecode VM & CLI interpreter.
- `target/release/whalli-lsp` — Language Server Protocol server.

## Run a Script

Run the demo REST API with SQLite:

```bash
./target/release/whalli main.wh
# or via Cargo:
cargo run --bin whalli -- main.wh
```

## Hello World

Create `hello.wh`:

```whalli
println("Hello, Whalli!")
let answer = 42
println(f"The answer is {answer}")
```

Run it:

```bash
cargo run --bin whalli -- hello.wh
```

## Environment Variables

- `WHALLI_NUM_THREADS`: Controls the number of worker threads for the work-stealing scheduler (`vm.rs`). Defaults to `available_parallelism()` or `4`.

## Next Steps

Learn the syntax and core concepts in the [Language Tour](/language-tour).
