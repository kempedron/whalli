# Toolchain and Architecture

> Source: `src/{lexer,parser,ast,compiler,opcode,vm,heap,value}.rs`, `README.md:224-257`.

## Pipeline

```
Source (.wh) → Lexer → Parser → AST → Compiler → Bytecode → VM
                                      ↘ whalli-lsp (reuses Lexer/Parser)
```

1. **Lexer** (`src/lexer.rs`): tokenizes UTF-8 source. Handles string literals, `f"..."` interpolation markers, keywords, operators (`<-`, `->`, `=>`, `..`, `..=`), comments, whitespace, line tracking.
2. **Parser** (`src/parser.rs:32-998`): recursive-descent with precedence (`or→and→equality→term→primary`). Produces `Vec<Stmt>` (`src/ast.rs:Stmt/Expr/Pattern/SelectArm`). Enforces statement terminators, destructuring, `match`/`select` blocks.
3. **Compiler** (`src/compiler.rs`): walks `Stmt`/`Expr`, emits `Vec<OpCode>` (`src/opcode.rs`). Handles scopes, upvalues, closures, control flow jumps (if/while/for/break/continue), `defer` chains, `is` checks, `Import` resolution.
4. **VM** (`src/vm.rs:278-580`): stack-based executor. Each `Task` has `stack: Vec<Value>`, `frames: Vec<CallFrame>` (`closure_id`, `function: Arc<FunctionObj>`, `ip`, `stack_offset`, `base_offset`, `defers`). Executes `fuel=2000` slice per scheduling quantum.
5. **Heap & GC** (`src/heap.rs:70-259`): chunked heap (`CHUNK_SIZE=1024` slots). `alloc` reuses free list or grows. `mark` traces via `push_value_refs`; `sweep` frees unmarked. Triggered in `execute_task_slice` when `live_count >= gc_threshold`.
6. **Stdlib** (`src/stdlib/mod.rs:16-211`): registers globals and modules into `VM.globals` / `VM.modules` before `VM::run()`.

## VM Scheduler Details

- **SharedRuntime** (`vm.rs:77-99`): holds `heap`, `globals/modules` (`RwLock`), `injector`, sleeping/IO queues, `net`, `active_tasks`, `fatal_error`, `gc_threshold`.
- **Main task** (`vm.rs:300-307`): single initial `Task` with synthetic `FunctionObj {name:"main"}`.
- **Workers** (`vm.rs:519-537`): `num_threads` threads each with `Worker<Task>` deque. `find_task` steals first from local, then `Injector::steal_batch_and_pop`, then other `Stealer`s (`vm.rs:642-677`).
- **Sleep/IO wake** (`vm.rs:382-514`): timer/IO thread handles `Sleeping(Instant)` and `ChannelTimer` (`vm.rs:64-67, 411-439`) and `Poll` events.

## Bytecode

`OpCode` variants (representative): `Push`, `Add/Sub/Mul/Div/Mod`, `LoadGlobal`, `StoreGlobal`, `LoadLocal`, `SetLocal`, `Call`, `MethodCall`, `Closure`, `Jump`, `Is`, `ChanSend/Recv`, `Select`, `Match`, `Try`, `Defer`. Exact enum in `src/opcode.rs`.

## `is` Semantics

`vm.rs:188-276` — `Left is Right`:
- For primitive `Type`: checks runtime variant (`int` ↔ `Value::Int`, `list` ↔ `Obj::List`, etc.).
- For `(T1, T2)` tuple type string: checks length and element types recursively.
- For `ObjRef` right side: struct instance → type identity (`left struct_id == right_id`); interface → struct's `methods` contain all required names.

## Imports

File imports are lexed/parsed/compiled eagerly inside `VM` import handling (reads file, lexes/parses/compiles, executes in current VM context). Stdlib imports resolve to `modules` map.

## `whalli-lsp` Interaction

LSP binary (`src/bin/whalli-lsp.rs`) reuses `Lexer`/`Parser` to produce diagnostics without executing VM. Communicates via `tower-lsp` over stdin/stdout `stdio` JSON-RPC. Not part of runtime execution.

## Next

See [Tooling](./07-tooling.md) for LSP and editor integration.
