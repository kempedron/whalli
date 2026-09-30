mod common;
use common::run_code;
use whalli::value::Value;

#[test]
fn test_tail_call_deep_recursion() {
    let code = r#"
    func countdown(n: int) -> int {
        if n == 0 {
            return 42
        }
        return countdown(n - 1)
    }

    let res = countdown(50000)
    "#;
    let vm = run_code(code);
    assert_eq!(vm.globals.get("res"), Some(&Value::Int(42)));
}

#[test]
fn test_tail_call_with_accumulator() {
    let code = r#"
    func sum_acc(n: int, acc: int) -> int {
        if n <= 0 {
            return acc
        }
        return sum_acc(n - 1, acc + n)
    }

    let sum = sum_acc(1000, 0)
    "#;
    let vm = run_code(code);
    assert_eq!(vm.globals.get("sum"), Some(&Value::Int(500500)));
}
