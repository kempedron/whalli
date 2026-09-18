mod fs;
mod math;
mod time;

use crate::{heap::Heap, value::{Value, NativeResult}};
use std::collections::HashMap;
use std::rc::Rc;

pub fn register_natives(heap: &mut Heap) -> (HashMap<String, Value>, HashMap<String, Value>) {
    let mut globals = HashMap::new();
    let mut modules = HashMap::new();

    globals.insert(
        "print".to_string(),
        Value::Native(|args, heap| {
            let strings: Vec<String> = args.iter().map(|a| a.stringify(heap)).collect();
            print!("{}", strings.join(" "));
            NativeResult::Return(Value::Nil)
        }),
    );

    globals.insert(
        "println".to_string(),
        Value::Native(|args, heap| {
            let strings: Vec<String> = args.iter().map(|a| a.stringify(heap)).collect();
            println!("{}", strings.join(" "));
            NativeResult::Return(Value::Nil)
        }),
    );

    globals.insert(
        "int".to_string(),
        Value::Native(|args, _heap| {
            let val = match args.first() {
                Some(Value::Int(n)) => Value::Int(*n),
                Some(Value::Float(f)) => Value::Int(*f as i64),
                Some(Value::Str(s)) => s.parse::<i64>().map(Value::Int).unwrap_or(Value::Int(0)),
                Some(Value::Bool(b)) => Value::Int(if *b { 1 } else { 0 }),
                _ => Value::Int(0),
            };
            NativeResult::Return(val)
        }),
    );

    globals.insert(
        "float".to_string(),
        Value::Native(|args, _heap| {
            let val = match args.first() {
                Some(Value::Float(f)) => Value::Float(*f),
                Some(Value::Int(n)) => Value::Float(*n as f64),
                Some(Value::Str(s)) => s.parse::<f64>().map(Value::Float).unwrap_or(Value::Float(0.0)),
                _ => Value::Float(0.0),
            };
            NativeResult::Return(val)
        }),
    );

    globals.insert(
        "str".to_string(),
        Value::Native(|args, heap| {
            let val = match args.first() {
                Some(val) => Value::Str(Rc::new(val.stringify(heap))),
                None => Value::Str(Rc::new("".to_string())),
            };
            NativeResult::Return(val)
        }),
    );

    globals.insert(
        "bool".to_string(),
        Value::Native(|args, _heap| {
            let val = match args.first() {
                Some(Value::Bool(b)) => Value::Bool(*b),
                Some(Value::Int(n)) => Value::Bool(*n != 0),
                Some(Value::Float(f)) => Value::Bool(*f != 0.0),
                Some(Value::Str(s)) => Value::Bool(!s.is_empty()),
                Some(Value::Nil) => Value::Bool(false),
                Some(_) => Value::Bool(true),
                None => Value::Bool(false),
            };
            NativeResult::Return(val)
        }),
    );

    globals.insert(
        // new(<type>, <length>, <capacity>)
        // If capacity is not transferred - capacity = lengtha
        "new".to_string(),
        Value::Native(|args, heap| {
            if args.is_empty() {
                return crate::value::NativeResult::Return(Value::Nil);
            }

            let type_name = match &args[0] {
                Value::Str(s) => s.as_str(),
                _ => return crate::value::NativeResult::Return(Value::Nil),
            };

            let arg_len = if args.len() > 1 {
                match &args[1] { Value::Int(n) => *n as usize, _ => 0 }
            } else { 0 };

            let arg_cap = if args.len() > 2 {
                match &args[2] { Value::Int(n) => *n as usize, _ => arg_len }
            } else { arg_len };

            match type_name {
                "list" => {
                    let mut elements = Vec::with_capacity(arg_cap);
                    for _ in 0..arg_len {
                        elements.push(Value::Nil);
                    }
                    let id = heap.alloc(crate::heap::Obj::List(elements));
                    crate::value::NativeResult::Return(Value::ObjRef(id))
                }
                "map" => {
                    let id = heap.alloc(crate::heap::Obj::Map(std::collections::HashMap::with_capacity(arg_cap)));
                    crate::value::NativeResult::Return(Value::ObjRef(id))
                }
                "chan" => {
                    let id = heap.alloc(crate::heap::Obj::Channel(std::collections::VecDeque::with_capacity(arg_len)));
                    crate::value::NativeResult::Return(Value::ObjRef(id))
                }
                "tuple" => {
                    let mut elements = Vec::with_capacity(arg_len);
                    for _ in 0..arg_len { elements.push(Value::Nil); }
                    crate::value::NativeResult::Return(Value::Tuple(Rc::new(elements)))
                }
                _ => crate::value::NativeResult::Return(Value::Nil),
            }
        }),
    );

    modules.insert("math".to_string(), math::register(heap));
    modules.insert("fs".to_string(), fs::register(heap));
    modules.insert("time".to_string(), time::register(heap));

    (globals, modules)
}