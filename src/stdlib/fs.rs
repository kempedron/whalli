use crate::heap::Obj;
use crate::value::{NativeResult, Value};
use crate::vm::VM;
use std::collections::HashMap;
use std::fs;
use std::rc::Rc;

pub fn register(vm: &mut VM) -> Value {
    let mut fs_module = HashMap::new();

    // fs.read(path: str) -> (content: str, err: str | nil)
    fs_module.insert(
        "read".to_string(),
        Value::Native(|args, _vm| {
            if let Some(Value::Str(path)) = args.first() {
                match fs::read_to_string(&**path) {
                    Ok(content) => {
                        let res = Value::Tuple(Rc::new(vec![Value::Str(Rc::new(content)), Value::Nil]));
                        NativeResult::Return(res)
                    }
                    Err(e) => {
                        let res = Value::Tuple(Rc::new(vec![Value::Nil, Value::Str(Rc::new(e.to_string()))]));
                        NativeResult::Return(res)
                    }
                }
            } else {
                let res = Value::Tuple(Rc::new(vec![Value::Nil, Value::Str(Rc::new("Expected file path string".to_string()))]));
                NativeResult::Return(res)
            }
        }),
    );

    // fs.write(path: str, data: str) -> (ok: bool, err: str | nil)
    fs_module.insert(
        "write".to_string(),
        Value::Native(|args, _vm| {
            if args.len() >= 2 {
                if let (Value::Str(path), Value::Str(content)) = (&args[0], &args[1]) {
                    return match fs::write(&**path, &**content) {
                        Ok(_) => {
                            let res = Value::Tuple(Rc::new(vec![Value::Bool(true), Value::Nil]));
                            NativeResult::Return(res)
                        }
                        Err(e) => {
                            let res = Value::Tuple(Rc::new(vec![Value::Bool(false), Value::Str(Rc::new(e.to_string()))]));
                            NativeResult::Return(res)
                        }
                    };
                }
            }
            let res = Value::Tuple(Rc::new(vec![Value::Bool(false), Value::Str(Rc::new("Expected path and data strings".to_string()))]));
            NativeResult::Return(res)
        }),
    );

    let id = vm.heap.alloc(Obj::Map(fs_module));
    Value::ObjRef(id)
}
