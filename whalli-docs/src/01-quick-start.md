# Quick Start

## Prerequisites

- Rust & Cargo, `edition 2024` or later (`Cargo.toml:4`). Install via <https://rustup.rs>.
- Linux/macOS with `mio` `os-poll` support (Windows is not tested).

## Build

```bash
git clone https://github.com/kempedron/whalli.git
cd whalli
cargo build --release          # builds `whalli` and `whalli-lsp`
```

Binaries: `target/release/whalli`, `target/release/whalli-lsp` (`Cargo.toml:1-3`).

## Run a Script

```bash
./target/release/whalli main.wh
# or
cargo run --bin whalli -- main.wh
```

`src/main.rs` compiles the file to bytecode and starts `VM::run()`.

## Project Layout

```
whalli/
  main.wh              # REST API example (this book's primary runnable)
  math_utils.wh        # import test fixture
  test_import.wh       # `import "./math_utils.wh"` demo
  src/
    lexer.rs           # tokenization, f-string handling
    parser.rs          # recursive-descent, Stmt/Expr/Match/Select
    ast.rs             # AST definitions
    compiler.rs        # bytecode emission (OpCode)
    opcode.rs          # instruction set
    vm.rs              # scheduler + poll integration
    heap.rs            # GC heap (mark & sweep)
    value.rs           # Value, method dispatch, type checks
    stdlib/{net,http,json,requests,fs,math,time,os,sync}.rs
    bin/whalli-lsp.rs  # LSP server (JSON-RPC over stdio, tower-lsp)
  whalli-vscode/       # VS Code / VSCodium extension
  whalli-docs/         # this book (mdBook)
```

## Hello World

```whalli
println("Hello, Whalli!")
let x = 42
println(f"answer={x}")
```

## Environment Variables

- `WHALLI_NUM_THREADS` — worker pool size (`vm.rs:343-350`). Default: `available_parallelism() || 4`, min 1.

## Next

Continue to [Language Tour](./02-language-tour.md).
