---
title: Toolchain and Architecture
description: Whalli virtual machine architecture, bytecode instructions, and memory model.
---

## Architecture Diagram

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

## Compiler Pipeline

1. **Lexer (`src/lexer.rs`)**: Tokenizes source text, processes format-string interpolations (`f"..."`), comments, and symbols.
2. **Parser (`src/parser.rs`)**: Recursive descent parser producing Abstract Syntax Tree nodes (`src/ast.rs`).
3. **Compiler (`src/compiler.rs`)**: Emits flat bytecode chunks with symbol resolution, loop jump patches, and upvalue tracking.
4. **VM (`src/vm.rs`)**: Executes bytecode with an M:N work-stealing thread pool, cooperative woroutine scheduler, and non-blocking I/O event polling.

## Memory Management & GC

Whalli manages heap allocations through a chunk-based memory pool:
- **Mark & Sweep Garbage Collector**: Periodically scans live roots across woroutine call stacks, globals, modules, and injector queues.
- **Dynamic Threshold**: The GC threshold scales adaptively based on live object retention (`max(live_count * 2, 256)`).
- **Concurrency Safety**: Heap slots are protected by fine-grained read-write locks (`parking_lot::RwLock`).
