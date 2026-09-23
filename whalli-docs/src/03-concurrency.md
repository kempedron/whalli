# Concurrency

> Source: `src/vm.rs:44-57, 69-113, 342-767`, `src/heap.rs:36-52`, `src/stdlib/sync.rs`, `src/stdlib/time.rs`, `src/value.rs`.

## Model

- **Cooperative woroutines** (`wo`): lightweight tasks multiplexed on a fixed worker pool. No preemption; a task yields only on blocking operations (`time.sleep`, `chan` send/recv, `net.accept/read/write`, `sync` wait).
- Scheduler: `VM::run()` creates `num_threads = max(1, WHALLI_NUM_THREADS || available_parallelism || 4)` (`vm.rs:343-351`). Each worker owns a `crossbeam_deque::Worker<Task>` deque; a global `Injector<Task>` serves as the entry point; workers steal via `Stealer`.
- Task states (`vm.rs:44-49`): `Runnable`, `Sleeping(SystemTime)`, `Waiting`, `WaitingIO(Token)`.
- Timer/IO thread: single dedicated thread polls `mio::Poll` and channel timers, wakes sleepers (`vm.rs:382-515`). Closed tokens also wake `WaitingIO` tasks.

## Spawning

```whalli
func producer(ch) {
    for i in range(1, 4) {
        time.sleep(0.5) // non-blocking; SuspendSleep
        ch <- f"task #{i}"
    }
}
let ch = new(chan, 3)
wo producer(ch)            // Stmt::Spawn / Expr::Spawn (parser.rs:286,514)
wo http.listen_and_serve(":8080", router) // also MethodCall form via wo
```

- Syntax: `wo callee(args)` or `wo obj.method(args)` — desugared to `Stmt::Spawn` (`parser.rs:290-301`).
- Semantics: new `Task` is pushed to `Injector`, `active_tasks` incremented (`vm.rs` spawn handling).

## Channels

Heap object: `Obj::Channel { queue: VecDeque<Value>, capacity, closed }` (`heap.rs:36-40`). Allocated by `new(chan, capacity)` (`stdlib/mod.rs:147-154`, capacity defaults 1).

Operations:

- **Send**: `ch <- value` (`Expr::ChanSend`, `parser.rs:365-367`). If queue length >= capacity, task parks (waiting).
- **Recv**: `<- ch` (`Expr::ChanRecv`, `parser.rs:514-517`). If queue empty, parks.
- **Close**: `ch.close()` (`value.rs` dispatch for `Channel::close`, `vm.rs:1158-1164`).
- **Select**: Go-style `select` (`parser.rs:919-997`, `ast.rs:SelectArm`) — evaluable as expression.

Capacity note: `stdlib/mod.rs:148` clamps `cap==0` to 1.

## Sync Primitives

`import sync` (`stdlib/sync.rs`):

```whalli
import sync

let wg = sync.WaitGroup()
wg.add(2)
wo func() { /* work */ wg.done() }()
wg.wait()

let mu = sync.Mutex()
mu.lock()
 // critical section
mu.unlock()
mu.try_lock() // -> bool
```

- `WaitGroup { count, waiting_tasks }` (`heap.rs:42-45`): `add(delta)`, `done()`, `wait()` (parks if count>0) — `vm.rs:1168-1192, 1640` handling.
- `Mutex { locked, waiting_tasks }` (`heap.rs:46-49`): `lock()` parks if already locked, `unlock()`, `try_lock() -> bool` — `vm.rs:1194-1217`.

## Time

`import time` (`stdlib/time.rs`):

- `time.sleep(secs: float|int)` → `NativeResult::SuspendSleep(secs)` (`time.rs:19-32`), task state `Sleeping(Instant)` woken by timer thread.
- `time.after(secs)` → `chan` that receives current epoch float after duration (`time.rs:34-58`, `vm.rs:411-430` firing logic).
- `time.now() -> float` — epoch seconds (`time.rs:10-17`).

## Select

```whalli
let result = select {
    val <- ch => f"got {val}",
    ch2 <- 42 => "sent",
    <- time.after(1.0) => "timeout",
    default => "no ready channel",
}
```

- Cases: `var <- chan => body` (recv), `chan <- val => body` (send), `<- chan => body` (anonymous recv), `default => body` (`parser.rs:943-997`). Evaluated atomically; `default` chosen if no channel ready.

## Task Handles

Heap `Obj::TaskHandle { status: Running|Completed(Value)|Failed(String) }` (`heap.rs:13-17, 50-52`). Background task panics are isolated: defers run, status set to `Failed`, `active_tasks` decremented without aborting main (`vm.rs:783-797`).

## Garbage Collection

- Incremental mark-and-sweep (`vm.rs:127-186`, `heap.rs:170-259`), triggered when `live_count >= gc_threshold` (`vm.rs:817`). Roots: current task stack/frames, sleeping/IO tasks, globals/modules, injector queue. After sweep, threshold = `max(live*2, 256)`.

## Tuning

- `WHALLI_NUM_THREADS` env sets worker count. Timer thread granularity: `next_timeout = min(50ms, 5ms if pending sleepers/timers)` (`vm.rs:392, 443-445`).
