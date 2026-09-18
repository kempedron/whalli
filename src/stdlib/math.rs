use crate::heap::{Heap, Obj};
use crate::value::{Value, NativeResult};
use std::collections::HashMap;

pub fn register(heap: &mut Heap) -> Value {
    let mut math_module = HashMap::new();

    math_module.insert("pi".to_string(), Value::Float(std::f64::consts::PI));
    math_module.insert(
        "sin".to_string(),
        Value::Native(|args, _heap| {
            if let Some(Value::Float(n)) = args.first() {
                NativeResult::Return(Value::Float(n.sin()))
            } else {
                NativeResult::Return(Value::Nil)
            }
        }),
    );

    let id = heap.alloc(Obj::Map(math_module));
    Value::ObjRef(id)
}