---
title: Introduction
description: Overview and design goals of the Whalli programming language.
---

**Whalli** is a lightweight, dynamically-typed programming language with cooperative multitasking, non-blocking I/O, and integrated developer tooling. The reference implementation is a stack-based bytecode VM written in Rust (`edition 2024`).

## Design Goals

- **Minimal runtime overhead**: single VM work-stealing thread-pool, no OS thread per task.
- **Cooperative concurrency first**: woroutines and channels as native language primitives.
- **Non-blocking I/O via `mio`**: `net` and `http` use OS event polling without blocking worker threads.
- **Self-contained Backend Stack**: Go `net/http`-level features with Keep-Alive, REST routing, middlewares, and a unified SQL interface (`sqlite`, `postgres`, `mysql`).
- **Tooling built-in**: Language Server Protocol (LSP) and VS Code extension ship with the project.

## Status

- Version `0.1.0`.
- License: MIT.

## Table of Contents

1. [Quick Start](/quick-start) — Building from source and running your first script.
2. [Language Tour](/language-tour) — Syntax, types, control flow, functions, structs, and pattern matching.
3. [Concurrency](/concurrency) — Woroutines (`wo`), channels (`chan`), synchronization, and `select`.
4. [I/O and Networking](/io-net) — Non-blocking TCP sockets, modern HTTP server, REST router, and requests.
5. [Standard Library](/stdlib) — Modules overview including `sql`, `http`, `net`, `json`, `fs`, `os`, `time`, `sync`, and `math`.
6. [Toolchain and Architecture](/toolchain) — Bytecode compiler, VM architecture, memory management, and GC.
7. [Tooling & LSP](/tooling) — Language Server Protocol (`whalli-lsp`) and VS Code extension.
8. [Examples](/examples) — Complete runnable programs including the RESTful Tasks API with SQLite.
