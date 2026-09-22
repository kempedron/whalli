use crate::heap::Obj;
use crate::value::{NativeResult, Value};
use crate::vm::VM;
use std::collections::HashMap;
use std::rc::Rc;

pub fn register(vm: &mut VM) -> Value {
    let mut os_module = HashMap::new();

    // os.args() -> list of strings
    os_module.insert(
        "args".to_string(),
        Value::Native(|_args, vm| {
            let args_list: Vec<Value> = std::env::args()
                .map(|a| Value::Str(Rc::new(a)))
                .collect();
            let id = vm.heap.alloc(Obj::List(args_list));
            NativeResult::Return(Value::ObjRef(id))
        }),
    );

    // os.env(key: str) -> str or nil
    os_module.insert(
        "env".to_string(),
        Value::Native(|args, _vm| {
            if let Some(Value::Str(key)) = args.first() {
                match std::env::var(key.as_str()) {
                    Ok(val) => NativeResult::Return(Value::Str(Rc::new(val))),
                    Err(_) => NativeResult::Return(Value::Nil),
                }
            } else {
                NativeResult::Return(Value::Nil)
            }
        }),
    );

    // os.set_env(key: str, val: str) -> bool
    os_module.insert(
        "set_env".to_string(),
        Value::Native(|args, _vm| {
            if args.len() >= 2 {
                if let (Value::Str(key), Value::Str(val)) = (&args[0], &args[1]) {
                    // Safe set_var
                    unsafe {
                        std::env::set_var(key.as_str(), val.as_str());
                    }
                    return NativeResult::Return(Value::Bool(true));
                }
            }
            NativeResult::Return(Value::Bool(false))
        }),
    );

    // os.cwd() -> str
    os_module.insert(
        "cwd".to_string(),
        Value::Native(|_args, _vm| {
            let cwd = std::env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| ".".to_string());
            NativeResult::Return(Value::Str(Rc::new(cwd)))
        }),
    );

    // os.exit(code: int)
    os_module.insert(
        "exit".to_string(),
        Value::Native(|args, _vm| {
            let code = match args.first() {
                Some(Value::Int(c)) => *c as i32,
                _ => 0,
            };
            std::process::exit(code);
        }),
    );

    let id = vm.heap.alloc(Obj::Map(os_module));
    Value::ObjRef(id)
}
