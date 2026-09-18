use crate::heap::Obj;
use crate::value::{NativeResult, Value};
use crate::vm::VM;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn register(vm: &mut VM) -> Value {
    let mut time_module = HashMap::new();

    time_module.insert(
        "now".to_string(),
        Value::Native(|_args, _vm| {
            let start = SystemTime::now();
            let since_epoch = start.duration_since(UNIX_EPOCH).unwrap();
            NativeResult::Return(Value::Float(since_epoch.as_secs_f64()))
        }),
    );

    time_module.insert(
        "sleep".to_string(),
        Value::Native(|args, _vm| {
            if let Some(val) = args.first() {
                let secs = match val {
                    Value::Float(f) => *f,
                    Value::Int(i) => *i as f64,
                    _ => 0.0,
                };
                return NativeResult::SuspendSleep(secs.max(0.0));
            }
            NativeResult::Return(Value::Nil)
        }),
    );

    let id = vm.heap.alloc(Obj::Map(time_module));
    Value::ObjRef(id)
}
