use crate::heap::{Heap, Obj};
use crate::value::{NativeResult, Value};
use crate::vm::VM;
use std::collections::HashMap;
use std::sync::Arc;

fn value_to_json(val: &Value, heap: &Heap) -> serde_json::Value {
    match val {
        Value::Nil => serde_json::Value::Null,
        Value::Bool(b) => serde_json::Value::Bool(*b),
        Value::Int(i) => serde_json::json!(*i),
        Value::Float(f) => serde_json::json!(*f),
        Value::Str(s) => serde_json::Value::String((**s).clone()),
        Value::Tuple(elements) => {
            let arr: Vec<serde_json::Value> = elements.iter().map(|e| value_to_json(e, heap)).collect();
            serde_json::Value::Array(arr)
        }
        Value::Range(start, end, step) => {
            serde_json::json!({
                "start": start,
                "end": end,
                "step": step
            })
        }
        Value::ObjRef(id) => {
            if let Ok(obj) = heap.get(*id) {
                match obj {
                    Obj::List(list) => {
                        let arr: Vec<serde_json::Value> = list.iter().map(|e| value_to_json(e, heap)).collect();
                        serde_json::Value::Array(arr)
                    }
                    Obj::Map(map) => {
                        let mut json_obj = serde_json::Map::new();
                        for (k, v) in &map {
                            json_obj.insert(k.clone(), value_to_json(v, heap));
                        }
                        serde_json::Value::Object(json_obj)
                    }
                    Obj::Instance { fields, .. } => {
                        let mut json_obj = serde_json::Map::new();
                        for (k, v) in &fields {
                            json_obj.insert(k.clone(), value_to_json(v, heap));
                        }
                        serde_json::Value::Object(json_obj)
                    }
                    _ => serde_json::Value::Null,
                }
            } else {
                serde_json::Value::Null
            }
        }
        _ => serde_json::Value::Null,
    }
}

fn json_to_value(jv: serde_json::Value, heap: &mut Heap) -> Value {
    match jv {
        serde_json::Value::Null => Value::Nil,
        serde_json::Value::Bool(b) => Value::Bool(b),
        serde_json::Value::Number(num) => {
            if let Some(i) = num.as_i64() {
                Value::Int(i)
            } else {
                Value::Float(num.as_f64().unwrap_or(0.0))
            }
        }
        serde_json::Value::String(s) => Value::Str(Arc::new(s)),
        serde_json::Value::Array(arr) => {
            let elements: Vec<Value> = arr.into_iter().map(|v| json_to_value(v, heap)).collect();
            let id = heap.alloc(Obj::List(elements));
            Value::ObjRef(id)
        }
        serde_json::Value::Object(map) => {
            let mut wh_map = HashMap::new();
            for (k, v) in map {
                wh_map.insert(k, json_to_value(v, heap));
            }
            let id = heap.alloc(Obj::Map(wh_map));
            Value::ObjRef(id)
        }
    }
}

pub fn register(vm: &mut VM) -> Value {
    let mut json_module = HashMap::new();

    // json.encode(data) -> str
    json_module.insert(
        "encode".to_string(),
        Value::Native(|args, vm| {
            if let Some(val) = args.first() {
                let json_val = value_to_json(val, &vm.heap);
                let json_str = serde_json::to_string(&json_val).unwrap_or_else(|_| "null".to_string());
                NativeResult::Return(Value::Str(Arc::new(json_str)))
            } else {
                NativeResult::Return(Value::Str(Arc::new("null".to_string())))
            }
        }),
    );

    // json.decode(str) -> (value: any, err: str | nil)
    json_module.insert(
        "decode".to_string(),
        Value::Native(|args, vm| {
            if let Some(Value::Str(json_str)) = args.first() {
                match serde_json::from_str::<serde_json::Value>(json_str.as_str()) {
                    Ok(parsed) => {
                        let val = json_to_value(parsed, &mut vm.heap);
                        let res = Value::Tuple(Arc::new(vec![val, Value::Nil]));
                        NativeResult::Return(res)
                    }
                    Err(e) => {
                        let res = Value::Tuple(Arc::new(vec![Value::Nil, Value::Str(Arc::new(e.to_string()))]));
                        NativeResult::Return(res)
                    }
                }
            } else {
                let res = Value::Tuple(Arc::new(vec![Value::Nil, Value::Str(Arc::new("Expected JSON string".to_string()))]));
                NativeResult::Return(res)
            }
        }),
    );

    let id = vm.heap.alloc(Obj::Map(json_module));
    Value::ObjRef(id)
}
