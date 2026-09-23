mod common;
use common::run_code;
use whalli::value::Value;
use std::sync::Arc;

#[test]
fn test_strict_new_constructors() {
    let code = r#"
    let arr = new(list)
    arr.push(10)
    arr.push(20)

    let m = new(map)
    m["key"] = "val"

    let ch = new(chan, 5)
    "#;
    let vm = run_code(code);
    let arr = vm.globals.get("arr").expect("arr should exist");
    if let Value::ObjRef(id) = arr {
        assert!(matches!(vm.heap.get(*id), Ok(whalli::heap::Obj::List(_))));
    } else {
        panic!("Expected ObjRef for list");
    }

    let m = vm.globals.get("m").expect("m should exist");
    if let Value::ObjRef(id) = m {
        assert!(matches!(vm.heap.get(*id), Ok(whalli::heap::Obj::Map(_))));
    } else {
        panic!("Expected ObjRef for map");
    }

    let ch = vm.globals.get("ch").expect("ch should exist");
    if let Value::ObjRef(id) = ch {
        assert!(matches!(vm.heap.get(*id), Ok(whalli::heap::Obj::Channel { .. })));
    } else {
        panic!("Expected ObjRef for chan");
    }
}

#[test]
fn test_channel_close_and_for_loop() {
    let code = r#"
    func produce(ch) {
        for i in range(1, 4) {
            ch <- i
        }
        ch.close()
    }

    let ch = new(chan, 3)
    wo produce(ch)

    let collected = new(list)
    for item in ch {
        collected.push(item)
    }
    "#;
    let vm = run_code(code);
    let collected_val = vm.globals.get("collected").expect("collected should exist");
    if let Value::ObjRef(id) = collected_val {
        if let Ok(whalli::heap::Obj::List(list)) = vm.heap.get(*id) {
            assert_eq!(list, &[Value::Int(1), Value::Int(2), Value::Int(3)]);
        } else {
            panic!("Expected list");
        }
    } else {
        panic!("Expected ObjRef");
    }
}

#[test]
fn test_sync_waitgroup() {
    let code = r#"
    import sync

    let wg = sync.WaitGroup()
    let counter = new(list)

    func worker(id, wg, counter) {
        counter.push(id)
        wg.done()
    }

    for i in range(5) {
        wg.add(1)
        wo worker(i, wg, counter)
    }

    wg.wait()
    let final_len = counter.len()
    "#;
    let vm = run_code(code);
    let final_len = vm.globals.get("final_len").expect("final_len should exist");
    assert_eq!(final_len, &Value::Int(5));
}

#[test]
fn test_select_default() {
    let code = r#"
    let empty_ch = new(chan, 1)

    let result = select {
        msg <- empty_ch => "got msg",
        default => "fallback"
    }
    "#;
    let vm = run_code(code);
    let result = vm.globals.get("result").expect("result should exist");
    assert_eq!(result, &Value::Str(Arc::new("fallback".to_string())));
}

#[test]
fn test_select_recv_ready() {
    let code = r#"
    let ch1 = new(chan, 2)
    let ch2 = new(chan, 2)

    ch2 <- 999

    let result = select {
        msg <- ch1 => f"from ch1: {msg}",
        msg <- ch2 => f"from ch2: {msg}",
        default => "none"
    }
    "#;
    let vm = run_code(code);
    let result = vm.globals.get("result").expect("result should exist");
    assert_eq!(result, &Value::Str(Arc::new("from ch2: 999".to_string())));
}

#[test]
fn test_sync_mutex_concurrent_counter() {
    let code = r#"
    import sync

    let mu = sync.Mutex()
    let counter = new(list)

    func worker(id, wg, counter, mu) {
        for i in range(100) {
            mu.lock()
            counter.push(id)
            mu.unlock()
        }
        wg.done()
    }

    let wg = sync.WaitGroup()
    for i in range(10) {
        wg.add(1)
        wo worker(i, wg, counter, mu)
    }

    wg.wait()
    let total = counter.len()
    "#;
    let vm = run_code(code);
    let total = vm.globals.get("total").expect("total should exist");
    assert_eq!(total, &Value::Int(1000));
}

#[test]
fn test_sync_mutex_try_lock() {
    let code = r#"
    import sync

    let mu = sync.Mutex()
    let first_lock = mu.try_lock()
    let second_lock = mu.try_lock()
    mu.unlock()
    let third_lock = mu.try_lock()
    mu.unlock()
    "#;
    let vm = run_code(code);
    assert_eq!(vm.globals.get("first_lock"), Some(&Value::Bool(true)));
    assert_eq!(vm.globals.get("second_lock"), Some(&Value::Bool(false)));
    assert_eq!(vm.globals.get("third_lock"), Some(&Value::Bool(true)));
}

#[test]
fn test_whalli_num_threads_env() {
    unsafe { std::env::set_var("WHALLI_NUM_THREADS", "2"); }

    let code = r#"
    import sync

    let mu = sync.Mutex()
    let sum = 0

    let wg = sync.WaitGroup()
    func worker(wg, mu) {
        for i in range(50) {
            mu.lock()
            // critical section
            mu.unlock()
        }
        wg.done()
    }

    for i in range(4) {
        wg.add(1)
        wo worker(wg, mu)
    }
    wg.wait()
    let done = true
    "#;
    let vm = run_code(code);
    assert_eq!(vm.globals.get("done"), Some(&Value::Bool(true)));
    unsafe { std::env::remove_var("WHALLI_NUM_THREADS"); }
}

#[test]
fn test_work_stealing_high_throughput() {
    unsafe { std::env::set_var("WHALLI_NUM_THREADS", "4"); }

    // Spawns 100 woroutines, each performing computation and increments
    let code = r#"
    import sync

    let mu = sync.Mutex()
    let count = 0
    let wg = sync.WaitGroup()

    func compute(id, wg, mu) {
        let local_acc = 0
        for i in range(100) {
            local_acc += i
        }
        mu.lock()
        count += 1
        mu.unlock()
        wg.done()
    }

    for i in range(100) {
        wg.add(1)
        wo compute(i, wg, mu)
    }

    wg.wait()
    let final_count = count
    "#;
    let vm = run_code(code);
    assert_eq!(vm.globals.get("final_count"), Some(&Value::Int(100)));
    unsafe { std::env::remove_var("WHALLI_NUM_THREADS"); }
}
