use crate::heap::Obj;
use crate::value::{NativeResult, Value};
use crate::vm::VM;
use std::collections::HashMap;
use std::fs;
use std::rc::Rc;

pub fn register(vm: &mut VM) -> Value {
    let mut fs_module = HashMap::new();

    fs_module.insert(
        "read".to_string(),
        Value::Native(|args, _vm| {
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
        Value::Native(|args, _vm| {
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

    let id = vm.heap.alloc(Obj::Map(fs_module));
    Value::ObjRef(id)
}
