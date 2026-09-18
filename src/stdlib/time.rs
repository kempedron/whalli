use crate::heap::{Heap, Obj};
use crate::value::{Value, NativeResult};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn register(heap: &mut Heap) -> Value {
    let mut time_module = HashMap::new();

    time_module.insert(
        "now".to_string(),
        Value::Native(|_args, _heap| {
            let start = SystemTime::now();
            let since_epoch = start.duration_since(UNIX_EPOCH).unwrap();
            NativeResult::Return(Value::Float(since_epoch.as_secs_f64()))
        }),
    );

    time_module.insert(
        "sleep".to_string(),
        Value::Native(|args, _heap| {
            if let Some(val) = args.first() {
                let secs = match val {
                    Value::Float(f) => *f,
                    Value::Int(i) => *i as f64,
                    _ => 0.0,
                };
                // Возвращаем сигнал усыпить текущую ворутину
                return NativeResult::SuspendSleep(secs.max(0.0));
            }
            NativeResult::Return(Value::Nil)
        }),
    );

    let id = heap.alloc(Obj::Map(time_module));
    Value::ObjRef(id)
}