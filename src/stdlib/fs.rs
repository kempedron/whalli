use crate::heap::{Heap, Obj};
use crate::value::{Value, NativeResult};
use std::collections::HashMap;
use std::fs;
use std::rc::Rc;

pub fn register(heap: &mut Heap) -> Value {
    let mut fs_module = HashMap::new();

    fs_module.insert(
        "read".to_string(),
        Value::Native(|args, _heap| {
            if let Some(Value::Str(path)) = args.first() {
                match fs::read_to_string(&**path) {
                    Ok(content) => NativeResult::Return(Value::Str(Rc::new(content))),
                    Err(e) => NativeResult::Return(Value::Str(Rc::new(format!("Error: {}", e)))),
                }
            } else {
                NativeResult::Return(Value::Nil)
            }
        }),
    );

    fs_module.insert(
        "write".to_string(),
        Value::Native(|args, _heap| {
            if args.len() >= 2 {
                if let (Value::Str(path), Value::Str(content)) = (&args[0], &args[1]) {
                    return match fs::write(&**path, &**content) {
                        Ok(_) => NativeResult::Return(Value::Bool(true)),
                        Err(_) => NativeResult::Return(Value::Bool(false)),
                    };
                }
            }
            NativeResult::Return(Value::Bool(false))
        }),
    );

    let id = heap.alloc(Obj::Map(fs_module));
    Value::ObjRef(id)
}