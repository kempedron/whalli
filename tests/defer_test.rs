mod common;
use common::run_code;
use whalli::value::Value;
use std::sync::Arc;

#[test]
fn test_defer_basic_lifo_order() {
    let code = r#"
    let log = new(list)

    func run_task() {
        defer log.push("first")
        defer log.push("second")
        defer log.push("third")
        log.push("body")
    }

    run_task()
    "#;
    let vm = run_code(code);
    let log_val = vm.globals.get("log").expect("log should exist");
    if let Value::ObjRef(id) = log_val {
        if let Ok(whalli::heap::Obj::List(list)) = vm.heap.get(*id) {
            assert_eq!(
                list,
                &[
                    Value::Str(Arc::new("body".to_string())),
                    Value::Str(Arc::new("third".to_string())),
                    Value::Str(Arc::new("second".to_string())),
                    Value::Str(Arc::new("first".to_string())),
                ]
            );
        } else {
            panic!("Expected list");
        }
    } else {
        panic!("Expected ObjRef");
    }
}

#[test]
fn test_defer_with_mutex_and_return() {
    let code = r#"
    import sync

    let mu = sync.Mutex()

    func critical_section(mu) -> int {
        mu.lock()
        defer mu.unlock()
        return 123
    }

    let result = critical_section(mu)
    let can_lock_again = mu.try_lock()
    mu.unlock()
    "#;
    let vm = run_code(code);
    assert_eq!(vm.globals.get("result"), Some(&Value::Int(123)));
    assert_eq!(vm.globals.get("can_lock_again"), Some(&Value::Bool(true)));
}

#[test]
fn test_defer_evaluates_args_immediately() {
    let code = r#"
    let out = new(list)

    func run() {
        let x = 10
        defer out.push(x)
        x = 20
        out.push(x)
    }

    run()
    "#;
    let vm = run_code(code);
    let out_val = vm.globals.get("out").expect("out should exist");
    if let Value::ObjRef(id) = out_val {
        if let Ok(whalli::heap::Obj::List(list)) = vm.heap.get(*id) {
            assert_eq!(list, &[Value::Int(20), Value::Int(10)]);
        } else {
            panic!("Expected list");
        }
    } else {
        panic!("Expected ObjRef");
    }
}

#[test]
fn test_defer_early_return_branches() {
    let code = r#"
    import sync

    let mu = sync.Mutex()

    func check_flag(flag: bool, mu) -> str {
        mu.lock()
        defer mu.unlock()

        if flag {
            return "true_path"
        } else {
            return "false_path"
        }
    }

    let r1 = check_flag(true, mu)
    let unlocked1 = mu.try_lock()
    mu.unlock()

    let r2 = check_flag(false, mu)
    let unlocked2 = mu.try_lock()
    mu.unlock()
    "#;
    let vm = run_code(code);
    assert_eq!(vm.globals.get("r1"), Some(&Value::Str(Arc::new("true_path".to_string()))));
    assert_eq!(vm.globals.get("unlocked1"), Some(&Value::Bool(true)));
    assert_eq!(vm.globals.get("r2"), Some(&Value::Str(Arc::new("false_path".to_string()))));
    assert_eq!(vm.globals.get("unlocked2"), Some(&Value::Bool(true)));
}

#[test]
fn test_defer_anonymous_closure() {
    let code = r#"
    let trace = new(list)

    func cleanup(t) {
        t.push("deferred_closure")
    }

    func do_work() {
        trace.push("start")
        defer cleanup(trace)
        trace.push("end")
    }

    do_work()
    "#;
    let vm = run_code(code);
    let trace_val = vm.globals.get("trace").expect("trace should exist");
    if let Value::ObjRef(id) = trace_val {
        if let Ok(whalli::heap::Obj::List(list)) = vm.heap.get(*id) {
            assert_eq!(
                list,
                &[
                    Value::Str(Arc::new("start".to_string())),
                    Value::Str(Arc::new("end".to_string())),
                    Value::Str(Arc::new("deferred_closure".to_string())),
                ]
            );
        } else {
            panic!("Expected list");
        }
    } else {
        panic!("Expected ObjRef");
    }
}

#[test]
fn test_defer_with_try_operator_error() {
    let code = r#"
    import sync

    let mu = sync.Mutex()

    func fails() {
        return (nil, "boom")
    }

    func caller(mu) {
        mu.lock()
        defer mu.unlock()
        let val = fails()?
        return "unreachable"
    }

    let (res, err) = caller(mu)
    let unlocked = mu.try_lock()
    mu.unlock()
    "#;
    let vm = run_code(code);
    assert_eq!(vm.globals.get("res"), Some(&Value::Nil));
    assert_eq!(vm.globals.get("err"), Some(&Value::Str(Arc::new("boom".to_string()))));
    assert_eq!(vm.globals.get("unlocked"), Some(&Value::Bool(true)));
}
