use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use crate::value::Value;

pub fn register_natives() -> HashMap<String, Value> {
    let mut globals = HashMap::new();

    globals.insert("clock".to_string(), Value::Native(|_| {
        let start = SystemTime::now();
        let since_the_epoch = start.duration_since(UNIX_EPOCH).unwrap();
        Value::Float(since_the_epoch.as_secs_f64())
    }));

    globals.insert("push".to_string(), Value::Native(|args| {
        if args.len() != 2 {
            panic!("Runtime error: push() expects exactly 2 arguments");
        }
        if let Value::List(list) = &args[0] {
            list.borrow_mut().push(args[1].clone());
            return Value::Nil;
        }
        panic!("Runtime error: First argument to push() must be a list");
    }));

    globals.insert("len".to_string(), Value::Native(|args| {
        if args.len() != 1 {
            panic!("Runtime error: len() expects exactly 1 argument");
        }
        match &args[0] {
            Value::List(list) => Value::Int(list.borrow().len() as i64),
            Value::Str(s) => Value::Int(s.len() as i64),
            _ => panic!("Runtime error: len() unsupported type"),
        }
    }));

    globals.insert("print".to_string(), Value::Native(|args| {
        for arg in args {
            print!("{}", arg);
        }
        Value::Nil
    }));

    globals.insert("println".to_string(), Value::Native(|args| {
        for arg in args {
            print!("{}", arg);
        }
        println!();
        Value::Nil
    }));

    globals
}