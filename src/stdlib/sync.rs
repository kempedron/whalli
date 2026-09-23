use crate::heap::Obj;
use crate::value::{NativeResult, Value};
use crate::vm::VM;
use std::collections::HashMap;

pub fn register(vm: &mut VM) -> Value {
    let mut sync_module = HashMap::new();

    // sync.WaitGroup() -> WaitGroup object
    sync_module.insert(
        "WaitGroup".to_string(),
        Value::Native(|_args, vm| {
            let id = vm.heap.alloc(Obj::WaitGroup {
                count: 0,
                waiting_tasks: Vec::new(),
            });
            NativeResult::Return(Value::ObjRef(id))
        }),
    );

    // sync.Mutex() -> Mutex object
    sync_module.insert(
        "Mutex".to_string(),
        Value::Native(|_args, vm| {
            let id = vm.heap.alloc(Obj::Mutex {
                locked: false,
                waiting_tasks: Vec::new(),
            });
            NativeResult::Return(Value::ObjRef(id))
        }),
    );

    let id = vm.heap.alloc(Obj::Map(sync_module));
    Value::ObjRef(id)
}
