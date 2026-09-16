mod fs;
mod math;
mod time;

use crate::{heap::Heap, value::Value};
use std::collections::HashMap;

pub fn register_natives(heap: &mut Heap) -> (HashMap<String, Value>, HashMap<String, Value>) {
    let mut globals = HashMap::new();
    let mut modules = HashMap::new();

    globals.insert(
        "print".to_string(),
        Value::Native(|args, heap| {
            let strings: Vec<String> = args.iter().map(|a| a.stringify(heap)).collect();
            print!("{}", strings.join(" "));
            Value::Nil
        }),
    );

    globals.insert(
        "println".to_string(),
        Value::Native(|args, heap| {
            let strings: Vec<String> = args.iter().map(|a| a.stringify(heap)).collect();
            println!("{}", strings.join(" "));
            Value::Nil
        }),
    );

    // Type casting
    globals.insert(
        "int".to_string(),
        Value::Native(|args, heap| match args.first() {
            Some(Value::Int(n)) => Value::Int(*n),
            Some(Value::Float(f)) => Value::Int(*f as i64),
            Some(Value::Str(s)) => s.parse::<i64>().map(Value::Int).unwrap_or(Value::Int(0)),
            Some(Value::Bool(b)) => Value::Int(if *b { 1 } else { 0 }),
            _ => Value::Int(0),
        }),
    );

    globals.insert(
        "float".to_string(),
        Value::Native(|args, heap| match args.first() {
            Some(Value::Float(f)) => Value::Float(*f),
            Some(Value::Int(n)) => Value::Float(*n as f64),
            Some(Value::Str(s)) => s
                .parse::<f64>()
                .map(Value::Float)
                .unwrap_or(Value::Float(0.0)),
            _ => Value::Float(0.0),
        }),
    );

    globals.insert(
        "str".to_string(),
        Value::Native(|args, heap| match args.first() {
            Some(val) => Value::Str(std::rc::Rc::new(val.stringify(heap))), // <--- ИСПОЛЬЗУЕМ stringify
            None => Value::Str(std::rc::Rc::new("".to_string())),
        }),
    );

    globals.insert(
        "bool".to_string(),
        Value::Native(|args, heap| match args.first() {
            Some(Value::Bool(b)) => Value::Bool(*b),
            Some(Value::Int(n)) => Value::Bool(*n != 0),
            Some(Value::Float(f)) => Value::Bool(*f != 0.0),
            Some(Value::Str(s)) => Value::Bool(!s.is_empty()),
            Some(Value::Nil) => Value::Bool(false),
            Some(_) => Value::Bool(true),
            None => Value::Bool(false),
        }),
    );

    // modules register
    modules.insert("math".to_string(), math::register(heap));
    modules.insert("fs".to_string(), fs::register(heap));
    modules.insert("time".to_string(), time::register(heap));

    (globals, modules)
}
