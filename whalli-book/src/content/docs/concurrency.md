---
title: Concurrency
description: Woroutines, channels, synchronization primitives, and select in Whalli.
---

## Concurrency Model

Whalli implements an M:N work-stealing cooperative multitasking scheduler backed by `crossbeam-deque`. Lightweight green threads are called **woroutines** (`wo`).

- **Zero OS Thread Overhead**: Thousands of concurrent woroutines can run across a fixed worker pool.
- **Non-blocking Operations**: Tasks yield control on `time.sleep`, channel operations, I/O events, and synchronization barriers.
- **Dedicated I/O & Timer Poller**: A background event thread manages `mio::Poll` events and timer channels.

## Woroutines (`wo`) and Task Handles

Spawn a woroutine using the `wo` keyword:

```whalli
import time

func worker(id: int) {
    for i in range(1, 4) {
        time.sleep(0.1)
        println(f"Worker {id} step {i}")
    }
}

wo worker(1)
wo worker(2)
```

The `wo func(...)` expression returns a `TaskHandle`. This handle lets you monitor execution status, wait for results, and isolate panics:

```whalli
func calculate(a: int, b: int) -> int {
    return a + b
}

let task = wo calculate(20, 22)

// 1. Await via .result() -> (res, err)
let (res, err) = task.result()
println("Result:", res) // 42

// 2. Await via channel receive operator '<-'
let task2 = wo calculate(10, 5)
let (val, _) = <- task2

// 3. Non-blocking status check
if task.is_done() {
    println("Status:", task.status()) // "completed" or "failed"
}
```

If a woroutine panics (e.g. division by zero), the VM isolates the error within the task handle and returns it in `err`, while ensuring all registered `defer` blocks are safely unwound.

## Channels (`chan`)

Channels provide synchronized, typed message passing between woroutines.

```whalli
import time

let ch = new(chan, 5) // buffered capacity 5

wo func() {
    for i in range(1, 4) {
        time.sleep(0.2)
        ch <- f"payload #{i}"
    }
    ch.close()
}()

// Receive loop
while true {
    let msg = <- ch
    if msg == nil {
        break
    }
    println("Received:", msg)
}
```

## Select Statement

Multiplex across multiple channels or timeouts with Go-style `select`:

```whalli
import time

let ch1 = new(chan, 1)
let ch2 = new(chan, 1)

let result = select {
    msg <- ch1 => f"channel 1: {msg}",
    msg <- ch2 => f"channel 2: {msg}",
    <- time.after(1.0) => "timeout after 1 second",
    default => "no channel ready immediately",
}
```

## Synchronization (`sync`)

```whalli
import sync

// 1. WaitGroup
let wg = sync.WaitGroup()
wg.add(2)

wo func() {
    // task 1
    wg.done()
}()

wo func() {
    // task 2
    wg.done()
}()

wg.wait() // blocks until counter returns to 0

// 2. Mutex
let mu = sync.Mutex()
mu.lock()
// critical section
mu.unlock()

let acquired = mu.try_lock() // non-blocking check
if acquired {
    mu.unlock()
}
```
