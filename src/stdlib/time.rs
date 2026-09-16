use crate::heap::{Heap, Obj};
use crate::value::Value;
use std::collections::HashMap;
use std::thread;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn register(heap: &mut Heap) -> Value {
    let mut time_module = HashMap::new();

    time_module.insert(
        "now".to_string(),
        Value::Native(|args, heap| {
            let start = SystemTime::now();
            let since_epoch = start.duration_since(UNIX_EPOCH).unwrap();
            Value::Float(since_epoch.as_secs_f64())
        }),
    );

    time_module.insert(
        "sleep".to_string(),
        Value::Native(|args, heap| {
            if let Some(val) = args.first() {
                let secs = match val {
                    Value::Float(f) => *f,
                    Value::Int(i) => *i as f64,
                    _ => 0.0,
                };
                thread::sleep(Duration::from_secs_f64(secs));
            }
            Value::Nil
        }),
    );

    let id = heap.alloc(Obj::Map(time_module));
    Value::ObjRef(id)
}
