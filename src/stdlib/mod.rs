mod fs;
mod json;
mod math;
mod net;
mod os;
mod time;

use crate::value::{NativeResult, Value};
use crate::vm::VM;
use std::collections::HashMap;
use std::rc::Rc;

pub fn register_natives(vm: &mut VM) -> (HashMap<String, Value>, HashMap<String, Value>) {
    let mut globals = HashMap::new();
    let mut modules = HashMap::new();

    globals.insert(
        "print".to_string(),
        Value::Native(|args, vm| {
            let strings: Vec<String> = args.iter().map(|a| a.stringify(&vm.heap)).collect();
            print!("{}", strings.join(" "));
            NativeResult::Return(Value::Nil)
        }),
    );

    globals.insert(
        "println".to_string(),
        Value::Native(|args, vm| {
            let strings: Vec<String> = args.iter().map(|a| a.stringify(&vm.heap)).collect();
            println!("{}", strings.join(" "));
            NativeResult::Return(Value::Nil)
        }),
    );

    globals.insert(
        "int".to_string(),
        Value::Native(|args, _vm| {
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
        Value::Native(|args, _vm| {
            let val = match args.first() {
                Some(Value::Float(f)) => Value::Float(*f),
                Some(Value::Int(n)) => Value::Float(*n as f64),
                Some(Value::Str(s)) => s
                    .parse::<f64>()
                    .map(Value::Float)
                    .unwrap_or(Value::Float(0.0)),
                _ => Value::Float(0.0),
            };
            NativeResult::Return(val)
        }),
    );

    globals.insert(
        "str".to_string(),
        Value::Native(|args, vm| {
            let val = match args.first() {
                Some(val) => Value::Str(Rc::new(val.stringify(&vm.heap))),
                None => Value::Str(Rc::new("".to_string())),
            };
            NativeResult::Return(val)
        }),
    );

    globals.insert(
        "bool".to_string(),
        Value::Native(|args, _vm| {
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

    // new(<type>, <length>, <capacity>)
    // If capacity is not transferred - capacity = lengtha
    globals.insert(
        "new".to_string(),
        Value::Native(|args, vm| {
            if args.is_empty() {
                return crate::value::NativeResult::Return(Value::Nil);
            }

            let type_name = match &args[0] {
                Value::Str(s) => s.as_str(),
                _ => return crate::value::NativeResult::Return(Value::Nil),
            };

            let arg_len = if args.len() > 1 {
                match &args[1] {
                    Value::Int(n) => *n as usize,
                    _ => 0,
                }
            } else {
                0
            };

            let arg_cap = if args.len() > 2 {
                match &args[2] {
                    Value::Int(n) => *n as usize,
                    _ => arg_len,
                }
            } else {
                arg_len
            };

            match type_name {
                "list" => {
                    let mut elements = Vec::with_capacity(arg_cap);
                    for _ in 0..arg_len {
                        elements.push(Value::Nil);
                    }
                    let id = vm.heap.alloc(crate::heap::Obj::List(elements));
                    crate::value::NativeResult::Return(Value::ObjRef(id))
                }
                "map" => {
                    let id = vm.heap.alloc(crate::heap::Obj::Map(
                        std::collections::HashMap::with_capacity(arg_cap),
                    ));
                    crate::value::NativeResult::Return(Value::ObjRef(id))
                }
                "chan" => {
                    let id = vm.heap.alloc(crate::heap::Obj::Channel(
                        std::collections::VecDeque::with_capacity(arg_len),
                    ));
                    crate::value::NativeResult::Return(Value::ObjRef(id))
                }
                "tuple" => {
                    let mut elements = Vec::with_capacity(arg_len);
                    for _ in 0..arg_len {
                        elements.push(Value::Nil);
                    }
                    crate::value::NativeResult::Return(Value::Tuple(Rc::new(elements)))
                }
                _ => crate::value::NativeResult::Return(Value::Nil),
            }
        }),
    );

    globals.insert(
        "range".to_string(),
        Value::Native(|args, _vm| {
            match args.len() {
                1 => {
                    // range(end) -> 0..end, step=1
                    if let Value::Int(end) = args[0] {
                        NativeResult::Return(Value::Range(0, end, 1))
                    } else {
                        NativeResult::Return(Value::Nil)
                    }
                }
                2 => {
                    // range(start, end)
                    if let (Value::Int(start), Value::Int(end)) = (&args[0], &args[1]) {
                        NativeResult::Return(Value::Range(*start, *end, 1))
                    } else {
                        NativeResult::Return(Value::Nil)
                    }
                }
                3 => {
                    // range(start, end, step)
                    if let (Value::Int(start), Value::Int(end), Value::Int(step)) = (&args[0], &args[1], &args[2]) {
                        if *step == 0 { return NativeResult::Return(Value::Nil); }
                        NativeResult::Return(Value::Range(*start, *end, *step))
                    } else {
                        NativeResult::Return(Value::Nil)
                    }
                }
                _ => NativeResult::Return(Value::Nil),
            }
        }),
    );

    modules.insert("math".to_string(), math::register(vm));
    modules.insert("fs".to_string(), fs::register(vm));
    modules.insert("time".to_string(), time::register(vm));
    modules.insert("net".to_string(), net::register(vm));
    modules.insert("json".to_string(), json::register(vm));
    modules.insert("os".to_string(), os::register(vm));

    (globals, modules)
}
