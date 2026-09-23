mod common;
use common::run_code;
use whalli::value::Value;
use std::sync::Arc;

#[test]
fn test_gc_reclaims_unreachable_memory() {
    // Creates 600 temporary lists in a loop and discards them
    let code = r#"
    for i in range(600) {
        let temp = new(list)
        temp.push(i)
    }
    "#;
    let vm = run_code(code);
    assert!(
        vm.heap.live_count() < 100,
        "Expected live count < 100 after GC, got {}",
        vm.heap.live_count()
    );
}

#[test]
fn test_gc_preserves_reachable_references() {
    // Preserves the surviving list in a global variable
    let code = r#"
    let keeper = new(list)
    keeper.push("I am alive")

    for i in range(500) {
        let temp = new(list)
        temp.push(i)
    }
    "#;
    let vm = run_code(code);
    let keeper_val = vm.globals.get("keeper").expect("keeper should exist");
    if let Value::ObjRef(id) = keeper_val {
        let obj = vm.heap.get(*id).expect("keeper object must survive GC");
        if let whalli::heap::Obj::List(list) = obj {
            assert_eq!(list.len(), 1);
        } else {
            panic!("Expected list");
        }
    } else {
        panic!("Expected ObjRef");
    }
}

#[test]
fn test_collection_sort() {
    let code = r#"
    let numbers = new(list)
    numbers.push(42)
    numbers.push(10)
    numbers.push(99)
    numbers.push(5)
    numbers.sort()

    let words = new(list)
    words.push("cherry")
    words.push("apple")
    words.push("banana")
    words.sort()

    let t = (30, 10, 20)
    let sorted_t = t.sort()
    "#;
    let vm = run_code(code);

    // Check list sort
    let numbers_val = vm.globals.get("numbers").expect("numbers should exist");
    if let Value::ObjRef(id) = numbers_val {
        if let Ok(whalli::heap::Obj::List(list)) = vm.heap.get(*id) {
            assert_eq!(list, &[Value::Int(5), Value::Int(10), Value::Int(42), Value::Int(99)]);
        } else {
            panic!("Expected list");
        }
    }

    let words_val = vm.globals.get("words").expect("words should exist");
    if let Value::ObjRef(id) = words_val {
        if let Ok(whalli::heap::Obj::List(list)) = vm.heap.get(*id) {
            assert_eq!(list[0], Value::Str(Arc::new("apple".to_string())));
            assert_eq!(list[1], Value::Str(Arc::new("banana".to_string())));
            assert_eq!(list[2], Value::Str(Arc::new("cherry".to_string())));
        } else {
            panic!("Expected list");
        }
    }

    // Check tuple sort
    let sorted_t_val = vm.globals.get("sorted_t").expect("sorted_t should exist");
    if let Value::Tuple(elements) = sorted_t_val {
        assert_eq!(elements.as_slice(), &[Value::Int(10), Value::Int(20), Value::Int(30)]);
    } else {
        panic!("Expected Tuple");
    }
}
