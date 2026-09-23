# Introduction

**Whalli** is a lightweight, dynamically-typed programming language with cooperative multitasking, non-blocking I/O, and integrated developer tooling. The reference implementation is a stack-based bytecode VM written in Rust (`edition 2024`).

## Design Goals

- **Minimal runtime overhead**: single VM thread-pool, no OS thread per task.
- **Cooperative concurrency first**: woroutines and channels as primitives, not libraries.
- **Non-blocking I/O via `mio`**: `net`/`http` use OS poll without blocking scheduler.
- **Tooling built-in**: Language Server Protocol (LSP) and VS Code extension ship with the compiler.

## Status

- Version `0.1.0` (`Cargo.toml:2-4`). Language is **alpha** — syntax and stdlib may change.
- License: MIT (`LICENSE`).

## Audience of This Book

This documentation is a **concise technical summary** intended as the skeleton for a future `Rust Book`-style comprehensive guide. It states facts extracted from `src/` verbatim; no speculative semantics.

## How to Read

1. [Quick Start](./01-quick-start.md) — build and run.
2. [Language Tour](./02-language-tour.md) — syntax.
3. [Concurrency](./03-concurrency.md) — `wo`/`chan`/`select`.
4. [I/O and Networking](./04-io-net.md) — `net`/`http`.
5. [Standard Library](./05-stdlib.md) — module reference.
6. [Toolchain and Architecture](./06-toolchain.md) — compilation pipeline and VM.
7. [Tooling](./07-tooling.md) — LSP and editor.
8. [Examples](./08-examples.md) — runnable programs including the REST API in `main.wh`.

Source of truth: `src/lexer.rs`, `src/parser.rs`, `src/ast.rs`, `src/compiler.rs`, `src/opcode.rs`, `src/vm.rs`, `src/heap.rs`, `src/value.rs`, `src/stdlib/*.rs`.
