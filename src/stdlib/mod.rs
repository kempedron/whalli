mod math;
mod fs;
mod time;

use std::collections::HashMap;
use crate::value::Value;

pub fn register_natives() -> (HashMap<String, Value>, HashMap<String, Value>) {
    let mut globals = HashMap::new();
    let mut modules = HashMap::new();

    globals.insert("print".to_string(), Value::Native(|args| {
        for (i, arg) in args.iter().enumerate() {
            if i > 0 { print!(" "); }
            print!("{}", arg);
        }
        Value::Nil
    }));

    globals.insert("println".to_string(), Value::Native(|args| {
        for (i, arg) in args.iter().enumerate() {
            if i > 0 { print!(" "); }
            print!("{}", arg);
        }
        println!();
        Value::Nil
    }));

    // Type casting
    globals.insert("int".to_string(), Value::Native(|args| {
        match args.first() {
            Some(Value::Int(n)) => Value::Int(*n),
            Some(Value::Float(f)) => Value::Int(*f as i64),
            Some(Value::Str(s)) => s.parse::<i64>().map(Value::Int).unwrap_or(Value::Int(0)),
            Some(Value::Bool(b)) => Value::Int(if *b { 1 } else { 0 }),
            _ => Value::Int(0),
        }
    }));

    globals.insert("float".to_string(), Value::Native(|args| {
        match args.first() {
            Some(Value::Float(f)) => Value::Float(*f),
            Some(Value::Int(n)) => Value::Float(*n as f64),
            Some(Value::Str(s)) => s.parse::<f64>().map(Value::Float).unwrap_or(Value::Float(0.0)),
            _ => Value::Float(0.0),
        }
    }));

    globals.insert("str".to_string(), Value::Native(|args| {
        match args.first() {
            Some(val) => Value::Str(std::rc::Rc::new(format!("{}", val))),
            None => Value::Str(std::rc::Rc::new("".to_string())),
        }
    }));

    globals.insert("bool".to_string(), Value::Native(|args| {
        match args.first() {
            Some(Value::Bool(b)) => Value::Bool(*b),
            Some(Value::Int(n)) => Value::Bool(*n != 0),
            Some(Value::Float(f)) => Value::Bool(*f != 0.0),
            Some(Value::Str(s)) => Value::Bool(!s.is_empty()),
            Some(Value::Nil) => Value::Bool(false),
            Some(_) => Value::Bool(true),
            None => Value::Bool(false),
        }
    }));

    // modules register
    modules.insert("math".to_string(), math::register());
    modules.insert("fs".to_string(), fs::register());
    modules.insert("time".to_string(), time::register());

    (globals, modules)
}