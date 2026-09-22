mod common;
use common::run_code;
use whalli::value::Value;

#[test]
fn test_multi_file_import() {
    let math_utils = r#"
    func add(a: int, b: int) -> int {
        return a + b
    }
    func multiply(a: int, b: int) -> int {
        return a * b
    }
    struct Point {
        x: int,
        y: int,
    }
    "#;
    std::fs::write("/tmp/test_math_utils_fixture.wh", math_utils).unwrap();

    let main_code = r#"
    import "/tmp/test_math_utils_fixture.wh"

    let sum = add(10, 20)
    let prod = multiply(5, 6)
    let p = Point(1, 2)
    "#;
    let vm = run_code(main_code);
    let sum_val = vm.globals.get("sum").expect("sum should exist");
    assert_eq!(sum_val, &Value::Int(30));

    let prod_val = vm.globals.get("prod").expect("prod should exist");
    assert_eq!(prod_val, &Value::Int(30));

    std::fs::remove_file("/tmp/test_math_utils_fixture.wh").ok();
}
