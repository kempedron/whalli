use crate::heap::Obj;
use crate::value::{NativeResult, Value};
use crate::vm::VM;
use std::collections::HashMap;

pub fn register(vm: &mut VM) -> Value {
    let mut math_module = HashMap::new();

    math_module.insert("pi".to_string(), Value::Float(std::f64::consts::PI));
    math_module.insert("e".to_string(), Value::Float(std::f64::consts::E));
    math_module.insert(
        "sqrt".to_string(),
        Value::Native(|args, _vm| {
            if let Some(arg) = args.first() {
                match arg {
                    Value::Float(n) => NativeResult::Return(Value::Float(n.sqrt())),
                    Value::Int(n) => NativeResult::Return(Value::Float((*n as f64).sqrt())),
                    _ => NativeResult::Return(Value::Nil),
                }
            } else {
                NativeResult::Return(Value::Nil)
            }
        }),
    );
    math_module.insert(
        "sin".to_string(),
        Value::Native(|args, _vm| {
            if let Some(Value::Float(n)) = args.first() {
                NativeResult::Return(Value::Float(n.sin()))
            } else {
                NativeResult::Return(Value::Nil)
            }
        }),
    );

    let id = vm.heap.alloc(Obj::Map(math_module));
    Value::ObjRef(id)
}
