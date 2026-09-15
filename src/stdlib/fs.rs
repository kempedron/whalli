use std::collections::HashMap;
use std::rc::Rc;
use std::fs;
use crate::value::Value;
use crate::heap::{Heap, Obj};

pub fn register(heap: &mut Heap) -> Value {
    let mut fs_module = HashMap::new();
    
    fs_module.insert("read".to_string(), Value::Native(|args| {
        if let Some(Value::Str(path)) = args.first() {
            match fs::read_to_string(&**path) {
                Ok(content) => Value::Str(Rc::new(content)),
                Err(e) => Value::Str(Rc::new(format!("Error: {}", e))),
            }
        } else {
            Value::Nil
        }
    }));

    fs_module.insert("write".to_string(), Value::Native(|args| {
        if args.len() >= 2 {
            if let (Value::Str(path), Value::Str(content)) = (&args[0], &args[1]) {
                match fs::write(&**path, &**content) {
                    Ok(_) => return Value::Bool(true),
                    Err(_) => return Value::Bool(false),
                }
            }
        }
        Value::Bool(false)
    }));


    let id = heap.alloc(Obj::Map(fs_module));
    Value::ObjRef(id)
}