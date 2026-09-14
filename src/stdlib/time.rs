use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use std::thread;
use std::time::Duration;
use std::rc::Rc;
use std::cell::RefCell;
use crate::value::Value;

pub fn register() -> Value {
    let mut time_module = HashMap::new();
    
    time_module.insert("now".to_string(), Value::Native(|_| {
        let start = SystemTime::now();
        let since_epoch = start.duration_since(UNIX_EPOCH).unwrap();
        Value::Float(since_epoch.as_secs_f64())
    }));

    time_module.insert("sleep".to_string(), Value::Native(|args| {
        if let Some(val) = args.first() {
            let secs = match val {
                Value::Float(f) => *f,
                Value::Int(i) => *i as f64,
                _ => 0.0,
            };
            thread::sleep(Duration::from_secs_f64(secs));
        }
        Value::Nil
    }));

    Value::Map(Rc::new(RefCell::new(time_module)))
}