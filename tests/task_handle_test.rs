mod common;
use common::run_code;
use whalli::value::Value;
use std::sync::Arc;

#[test]
fn test_task_handle_success_result() {
    let code = r#"
    func calculate(a: int, b: int) -> int {
        return a + b
    }

    let task = wo calculate(20, 22)
    let (res, err) = task.result()
    "#;
    let vm = run_code(code);
    assert_eq!(vm.globals.get("res"), Some(&Value::Int(42)));
    assert_eq!(vm.globals.get("err"), Some(&Value::Nil));
}

#[test]
fn test_task_handle_error_isolation() {
    let code = r#"
    func failing_worker() {
        let x = 10 / 0
        return x
    }

    let task = wo failing_worker()
    let (res, err) = task.result()
    let survived = true
    "#;
    let vm = run_code(code);
    assert_eq!(vm.globals.get("res"), Some(&Value::Nil));
    let err_val = vm.globals.get("err").expect("err should exist");
    assert!(matches!(err_val, Value::Str(_)), "err should be string error message");
    assert_eq!(vm.globals.get("survived"), Some(&Value::Bool(true)));
}

#[test]
fn test_task_handle_defer_unwinds_on_error() {
    let code = r#"
    import sync

    let mu = sync.Mutex()

    func risky(mu) {
        mu.lock()
        defer mu.unlock()
        let bad = 100 / 0 // panics!
    }

    let task = wo risky(mu)
    let (res, err) = task.result()
    let unlocked = mu.try_lock()
    mu.unlock()
    "#;
    let vm = run_code(code);
    assert_eq!(vm.globals.get("res"), Some(&Value::Nil));
    assert!(vm.globals.get("err").is_some());
    assert_eq!(vm.globals.get("unlocked"), Some(&Value::Bool(true)));
}

#[test]
fn test_task_handle_status_and_is_done() {
    let code = r#"
    import time

    func slow() -> str {
        time.sleep(0.02)
        return "done"
    }

    let task = wo slow()
    let (res, err) = task.result()
    let done = task.is_done()
    let st = task.status()
    "#;
    let vm = run_code(code);
    assert_eq!(vm.globals.get("res"), Some(&Value::Str(Arc::new("done".to_string()))));
    assert_eq!(vm.globals.get("done"), Some(&Value::Bool(true)));
    assert_eq!(vm.globals.get("st"), Some(&Value::Str(Arc::new("completed".to_string()))));
}

#[test]
fn test_task_handle_chan_recv_operator() {
    let code = r#"
    func worker() -> str {
        return "hello from task"
    }

    let task = wo worker()
    let (res, err) = <- task
    "#;
    let vm = run_code(code);
    assert_eq!(vm.globals.get("res"), Some(&Value::Str(Arc::new("hello from task".to_string()))));
    assert_eq!(vm.globals.get("err"), Some(&Value::Nil));
}

#[test]
fn test_task_handle_in_select() {
    let code = r#"
    func quick() -> int {
        return 777
    }

    let task = wo quick()
    let got = select {
        item <- task => item[0],
        default => 0
    }
    "#;
    let vm = run_code(code);
    // Since task completes quickly or default is ready
    let got_val = vm.globals.get("got").expect("got should exist");
    assert!(got_val == &Value::Int(777) || got_val == &Value::Int(0));
}

#[test]
fn test_task_handle_try_operator() {
    let code = r#"
    func compute_ok() -> int {
        return 100
    }

    func compute_fail() -> int {
        let x = 10 / 0
        return x
    }

    func caller() {
        let t1 = wo compute_ok()
        let res1 = t1.result()?

        let t2 = wo compute_fail()
        let res2 = t2.result()? // Should early return error

        return (res1 + res2, nil)
    }

    let (val, err) = caller()
    "#;
    let vm = run_code(code);
    assert_eq!(vm.globals.get("val"), Some(&Value::Nil));
    assert!(matches!(vm.globals.get("err"), Some(Value::Str(_))));
}
