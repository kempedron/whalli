mod common;
use common::run_code;
use whalli::value::Value;
use std::rc::Rc;

#[test]
fn test_match_literals() {
    let code = r#"
    let x = 200
    let res = match x {
        200 => "OK",
        404 => "Not Found",
        _ => "Unknown"
    }

    let is_true = true
    let bool_res = match is_true {
        true => 1,
        false => 0,
        _ => -1
    }
    "#;
    let vm = run_code(code);
    let res = vm.globals.get("res").expect("res should exist");
    assert_eq!(res, &Value::Str(Rc::new("OK".to_string())));

    let bool_res = vm.globals.get("bool_res").expect("bool_res should exist");
    assert_eq!(bool_res, &Value::Int(1));
}

#[test]
fn test_match_ranges() {
    let code = r#"
    let score_a = 95
    let grade_a = match score_a {
        90..=100 => "A",
        80..90 => "B",
        0..80 => "F",
        _ => "Invalid"
    }

    let score_b = 85
    let grade_b = match score_b {
        90..=100 => "A",
        80..90 => "B",
        0..80 => "F",
        _ => "Invalid"
    }

    let score_f = 50
    let grade_f = match score_f {
        90..=100 => "A",
        80..90 => "B",
        0..80 => "F",
        _ => "Invalid"
    }
    "#;
    let vm = run_code(code);
    let grade_a = vm.globals.get("grade_a").expect("grade_a should exist");
    assert_eq!(grade_a, &Value::Str(Rc::new("A".to_string())));

    let grade_b = vm.globals.get("grade_b").expect("grade_b should exist");
    assert_eq!(grade_b, &Value::Str(Rc::new("B".to_string())));

    let grade_f = vm.globals.get("grade_f").expect("grade_f should exist");
    assert_eq!(grade_f, &Value::Str(Rc::new("F".to_string())));
}

#[test]
fn test_match_alternatives() {
    let code = r#"
    let status_2 = 2
    let res_2 = match status_2 {
        1 | 2 | 3 => "small",
        4 | 5 => "medium",
        _ => "large"
    }

    let status_5 = 5
    let res_5 = match status_5 {
        1 | 2 | 3 => "small",
        4 | 5 => "medium",
        _ => "large"
    }

    let status_99 = 99
    let res_99 = match status_99 {
        1 | 2 | 3 => "small",
        4 | 5 => "medium",
        _ => "large"
    }
    "#;
    let vm = run_code(code);
    assert_eq!(vm.globals.get("res_2").unwrap(), &Value::Str(Rc::new("small".to_string())));
    assert_eq!(vm.globals.get("res_5").unwrap(), &Value::Str(Rc::new("medium".to_string())));
    assert_eq!(vm.globals.get("res_99").unwrap(), &Value::Str(Rc::new("large".to_string())));
}

#[test]
fn test_match_tuples_and_variables() {
    let code = r#"
    let pt1 = (0, 0)
    let res1 = match pt1 {
        (0, 0) => "origin",
        (x, y) => "other"
    }

    let pt2 = (10, 20)
    let res2 = match pt2 {
        (0, 0) => "origin",
        (x, y) => f"point: {x},{y}"
    }
    "#;
    let vm = run_code(code);
    assert_eq!(vm.globals.get("res1").unwrap(), &Value::Str(Rc::new("origin".to_string())));
    assert_eq!(vm.globals.get("res2").unwrap(), &Value::Str(Rc::new("point: 10,20".to_string())));
}

#[test]
fn test_match_guards() {
    let code = r#"
    let num_big = 150
    let label_big = match num_big {
        x if x > 100 => "big",
        x => "small"
    }

    let num_small = 42
    let label_small = match num_small {
        x if x > 100 => "big",
        x => "small"
    }
    "#;
    let vm = run_code(code);
    assert_eq!(vm.globals.get("label_big").unwrap(), &Value::Str(Rc::new("big".to_string())));
    assert_eq!(vm.globals.get("label_small").unwrap(), &Value::Str(Rc::new("small".to_string())));
}
