use crate::heap::Obj;
use crate::value::{NativeResult, Value};
use crate::vm::{ChannelTimer, VM};
use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

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

    // time.after(seconds: float | int) -> chan
    time_module.insert(
        "after".to_string(),
        Value::Native(|args, vm| {
            let secs = match args.first() {
                Some(Value::Float(f)) => *f,
                Some(Value::Int(i)) => *i as f64,
                _ => 0.0,
            };

            let chan_id = vm.heap.alloc(Obj::Channel {
                queue: VecDeque::new(),
                capacity: 1,
                closed: false,
            });

            let fire_time = Instant::now() + Duration::from_secs_f64(secs.max(0.0));
            vm.channel_timers.lock().push(ChannelTimer {
                fire_time,
                chan_id,
            });

            NativeResult::Return(Value::ObjRef(chan_id))
        }),
    );

    let id = vm.heap.alloc(Obj::Map(time_module));
    Value::ObjRef(id)
}
