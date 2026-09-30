mod common;
use common::run_code;
use whalli::value::Value;
use std::sync::Arc;

#[test]
fn test_destructuring_list() {
    let code = r#"
    let arr = [10, 20, 30]
    let [a, b, c] = arr
    "#;
    let vm = run_code(code);
    assert_eq!(vm.globals.get("a"), Some(&Value::Int(10)));
    assert_eq!(vm.globals.get("b"), Some(&Value::Int(20)));
    assert_eq!(vm.globals.get("c"), Some(&Value::Int(30)));
}

#[test]
fn test_destructuring_map() {
    let code = r#"
    let user = {
        "name": "Alice",
        "age": 28,
        "city": "Rome"
    }
    let { name, age, city: user_city } = user
    "#;
    let vm = run_code(code);
    assert_eq!(vm.globals.get("name"), Some(&Value::Str(Arc::new("Alice".to_string()))));
    assert_eq!(vm.globals.get("age"), Some(&Value::Int(28)));
    assert_eq!(vm.globals.get("user_city"), Some(&Value::Str(Arc::new("Rome".to_string()))));
}

#[test]
fn test_destructuring_struct_instance() {
    let code = r#"
    struct Point {
        x: int,
        y: int
    }

    let p = Point(100, 200)
    let { x, y: y_coord } = p
    "#;
    let vm = run_code(code);
    assert_eq!(vm.globals.get("x"), Some(&Value::Int(100)));
    assert_eq!(vm.globals.get("y_coord"), Some(&Value::Int(200)));
}

#[test]
fn test_destructuring_local_scope() {
    let code = r#"
    func get_coords() -> int {
        let coords = [5, 15]
        let [x, y] = coords
        return x + y
    }

    let total = get_coords()
    "#;
    let vm = run_code(code);
    assert_eq!(vm.globals.get("total"), Some(&Value::Int(20)));
}
