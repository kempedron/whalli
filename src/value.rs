use mio::Token;
use std::sync::Arc;

use crate::{
    heap::{Heap, Obj},
    opcode::OpCode,
};

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionObj {
    pub name: String,
    pub arity: usize,
    pub chunk: Vec<OpCode>,
    pub param_types: Vec<String>,
    pub return_type: Option<String>,
}

#[derive(Debug, Clone)]
pub enum NativeResult {
    Return(Value),
    SuspendSleep(f64),
    SuspendIO(Token),
}

#[derive(Debug, Clone)]
pub enum Value {
    Nil,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(Arc<String>),
    Type(String),
    Function(Arc<FunctionObj>),
    Native(fn(Vec<Value>, &mut crate::vm::VM) -> NativeResult),
    ObjRef(usize),
    Tuple(Arc<Vec<Value>>),
    Range(i64, i64, i64),
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Nil, Value::Nil) => true,
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => a == b,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Str(a), Value::Str(b)) => a == b,
            (Value::Type(a), Value::Type(b)) => a == b,
            (Value::Function(a), Value::Function(b)) => Arc::ptr_eq(a, b),
            (Value::ObjRef(a), Value::ObjRef(b)) => a == b,
            (Value::Native(a), Value::Native(b)) => (*a as usize) == (*b as usize),
            (Value::Range(s1, e1, st1), Value::Range(s2, e2, st2)) => s1 == s2 && e1 == e2 && st1 == st2,
            _ => false,
        }
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Nil => write!(f, "nil"),
            Value::Int(n) => write!(f, "{}", n),
            Value::Float(n) => write!(f, "{}", n),
            Value::Bool(b) => write!(f, "{}", b),
            Value::Str(s) => write!(f, "{}", s),
            Value::Type(t) => write!(f, "<type {}>", t),
            Value::Function(func) => write!(f, "<func {}>", func.name),
            Value::Native(_) => write!(f, "<native>"),
            Value::ObjRef(id) => write!(f, "<object #{}>", id),
            Value::Tuple(elements) => {
                let items: Vec<String> = elements.iter().map(|e| format!("{}", e)).collect();
                write!(f, "({})", items.join(", "))
            }
            Value::Range(s, e, step) => write!(f, "range({}, {}, {})", s, e, step),
        }
    }
}

impl Value {
    pub fn compare(&self, other: &Value) -> std::cmp::Ordering {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => a.cmp(b),
            (Value::Float(a), Value::Float(b)) => a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal),
            (Value::Int(a), Value::Float(b)) => (*a as f64).partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal),
            (Value::Float(a), Value::Int(b)) => a.partial_cmp(&(*b as f64)).unwrap_or(std::cmp::Ordering::Equal),
            (Value::Str(a), Value::Str(b)) => a.cmp(b),
            (Value::Bool(a), Value::Bool(b)) => a.cmp(b),
            _ => std::cmp::Ordering::Equal,
        }
    }

    pub fn call_method(
        &self,
        method_name: &str,
        args: Vec<Value>,
        heap: &Heap,
    ) -> Result<Value, String> {
        match self {
            Value::Str(s) => match method_name {
                "len" => Ok(Value::Int(s.len() as i64)),
                "trim" => Ok(Value::Str(Arc::new(s.trim().to_string()))),
                "to_lower" => Ok(Value::Str(Arc::new(s.to_lowercase()))),
                "to_upper" => Ok(Value::Str(Arc::new(s.to_uppercase()))),
                "contains" => {
                    if args.len() != 1 {
                        return Err("'contains' expects 1 argument (substring)".to_string());
                    }
                    match &args[0] {
                        Value::Str(sub) => Ok(Value::Bool(s.contains(sub.as_str()))),
                        _ => Err("'contains' expects a string argument".to_string()),
                    }
                }
                "starts_with" => {
                    if args.len() != 1 {
                        return Err("'starts_with' expects 1 argument (prefix)".to_string());
                    }
                    match &args[0] {
                        Value::Str(prefix) => Ok(Value::Bool(s.starts_with(prefix.as_str()))),
                        _ => Err("'starts_with' expects a string argument".to_string()),
                    }
                }
                "ends_with" => {
                    if args.len() != 1 {
                        return Err("'ends_with' expects 1 argument (suffix)".to_string());
                    }
                    match &args[0] {
                        Value::Str(suffix) => Ok(Value::Bool(s.ends_with(suffix.as_str()))),
                        _ => Err("'ends_with' expects a string argument".to_string()),
                    }
                }
                "replace" => {
                    if args.len() != 2 {
                        return Err("'replace' expects 2 arguments (from, to)".to_string());
                    }
                    if let (Value::Str(from), Value::Str(to)) = (&args[0], &args[1]) {
                        Ok(Value::Str(Arc::new(s.replace(from.as_str(), to.as_str()))))
                    } else {
                        Err("'replace' expects 2 string arguments".to_string())
                    }
                }
                "split" => {
                    if args.len() != 1 {
                        return Err("'split' expects 1 argument (delimiter)".to_string());
                    }
                    match &args[0] {
                        Value::Str(delimiter) => {
                            let parts: Vec<Value> = s
                                .split(delimiter.as_str())
                                .map(|p| Value::Str(Arc::new(p.to_string())))
                                .collect();
                            let list_id = heap.alloc(Obj::List(parts));
                            Ok(Value::ObjRef(list_id))
                        }
                        _ => Err("'split' expects a string delimiter".to_string()),
                    }
                }
                _ => Err(format!("Method '{}' not found on string", method_name)),
            },
            Value::Tuple(elements) => match method_name {
                "len" => Ok(Value::Int(elements.len() as i64)),
                "sort" => {
                    let mut sorted = (**elements).clone();
                    sorted.sort_by(|a, b| a.compare(b));
                    Ok(Value::Tuple(Arc::new(sorted)))
                }
                _ => Err(format!("Method '{}' not found on tuple", method_name)),
            },
            Value::ObjRef(id) => {
                let obj_type = heap.with_read(*id, |obj| match obj {
                    Obj::List(_) => "list",
                    Obj::Map(_) => "map",
                    _ => "other",
                })?;

                match obj_type {
                    "list" => match method_name {
                        "push" => {
                            if args.len() != 1 {
                                return Err("'push' expects 1 argument".to_string());
                            }
                            heap.with_write(*id, |obj| {
                                if let Obj::List(list) = obj {
                                    list.push(args[0].clone());
                                    Ok(Value::Nil)
                                } else {
                                    unreachable!()
                                }
                            })?
                        }
                        "pop" => {
                            heap.with_write(*id, |obj| {
                                if let Obj::List(list) = obj {
                                    Ok(list.pop().unwrap_or(Value::Nil))
                                } else {
                                    unreachable!()
                                }
                            })?
                        }
                        "len" => {
                            heap.with_read(*id, |obj| {
                                if let Obj::List(list) = obj {
                                    Ok(Value::Int(list.len() as i64))
                                } else {
                                    unreachable!()
                                }
                            })?
                        }
                        "sort" => {
                            heap.with_write(*id, |obj| {
                                if let Obj::List(list) = obj {
                                    list.sort_by(|a, b| a.compare(b));
                                    Ok(Value::Nil)
                                } else {
                                    unreachable!()
                                }
                            })?
                        }
                        "reverse" => {
                            heap.with_write(*id, |obj| {
                                if let Obj::List(list) = obj {
                                    list.reverse();
                                    Ok(Value::Nil)
                                } else {
                                    unreachable!()
                                }
                            })?
                        }
                        "clear" => {
                            heap.with_write(*id, |obj| {
                                if let Obj::List(list) = obj {
                                    list.clear();
                                    Ok(Value::Nil)
                                } else {
                                    unreachable!()
                                }
                            })?
                        }
                        "contains" => {
                            if args.len() != 1 {
                                return Err("'contains' expects 1 argument".to_string());
                            }
                            heap.with_read(*id, |obj| {
                                if let Obj::List(list) = obj {
                                    Ok(Value::Bool(list.contains(&args[0])))
                                } else {
                                    unreachable!()
                                }
                            })?
                        }
                        "join" => {
                            let sep = if args.is_empty() {
                                ""
                            } else if let Value::Str(ref s) = args[0] {
                                s.as_str()
                            } else {
                                return Err("'join' expects a string separator".to_string());
                            };
                            let parts: Vec<String> = heap.with_read(*id, |obj| {
                                if let Obj::List(list) = obj {
                                    list.iter().map(|item| item.stringify(heap)).collect()
                                } else {
                                    Vec::new()
                                }
                            })?;
                            Ok(Value::Str(Arc::new(parts.join(sep))))
                        }
                        _ => Err(format!("Method '{}' not found on list", method_name)),
                    },
                    "map" => match method_name {
                        "len" => {
                            heap.with_read(*id, |obj| {
                                if let Obj::Map(map) = obj {
                                    Ok(Value::Int(map.len() as i64))
                                } else {
                                    unreachable!()
                                }
                            })?
                        }
                        "remove" => {
                            if args.len() != 1 {
                                return Err("'remove' expects 1 argument".to_string());
                            }
                            if let Value::Str(key) = &args[0] {
                                heap.with_write(*id, |obj| {
                                    if let Obj::Map(map) = obj {
                                        Ok(map.remove(&**key).unwrap_or(Value::Nil))
                                    } else {
                                        unreachable!()
                                    }
                                })?
                            } else {
                                Err("Map key must be string".to_string())
                            }
                        }
                        "keys" => {
                            let keys: Vec<Value> = heap.with_read(*id, |obj| {
                                if let Obj::Map(map) = obj {
                                    map.keys().map(|k| Value::Str(Arc::new(k.clone()))).collect()
                                } else {
                                    Vec::new()
                                }
                            })?;
                            let new_id = heap.alloc(Obj::List(keys));
                            Ok(Value::ObjRef(new_id))
                        }
                        _ => Err(format!("Method '{}' not found on map", method_name)),
                    },
                    _ => Err(format!(
                        "Method '{}' not found on this heap object",
                        method_name
                    )),
                }
            }
            _ => Err(format!("Method '{}' not found on this type", method_name)),
        }
    }

    pub fn stringify(&self, heap: &crate::heap::Heap) -> String {
        match self {
            Value::Nil => "nil".to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Int(n) => n.to_string(),
            Value::Float(n) => n.to_string(),
            Value::Str(s) => s.to_string(),
            Value::Type(t) => format!("<type {}>", t),
            Value::Function(func) => format!("<func {}>", func.name),
            Value::Native(_) => "<native>".to_string(),
            Value::Range(s, e, step) => format!("range({}, {}, {})", s, e, step),
            Value::Tuple(elements) => {
                let items: Vec<String> = elements.iter().map(|e| e.stringify(heap)).collect();
                format!("({})", items.join(", "))
            }
            Value::ObjRef(id) => {
                heap.with_read(*id, |obj| match obj {
                    crate::heap::Obj::List(list) => {
                        let items: Vec<String> =
                            list.iter().map(|e| e.stringify(heap)).collect();
                        format!("[{}]", items.join(", "))
                    }
                    crate::heap::Obj::Map(map) => {
                        let items: Vec<String> = map
                            .iter()
                            .map(|(k, v)| format!("'{}': {}", k, v.stringify(heap)))
                            .collect();
                        format!("{{{}}}", items.join(", "))
                    }
                    crate::heap::Obj::Instance { struct_id, fields } => {
                        let mut items: Vec<String> = fields
                            .iter()
                            .map(|(k, v)| format!("{}: {}", k, v.stringify(heap)))
                            .collect();
                        items.sort();
                        if let Ok(crate::heap::Obj::StructDef { name, .. }) =
                            heap.get(*struct_id)
                        {
                            format!("{} {{{}}}", name, items.join(", "))
                        } else {
                            format!("Instance {{{}}}", items.join(", "))
                        }
                    }
                    _ => format!("{}", self),
                }).unwrap_or_else(|_| format!("{}", self))
            }
        }
    }
}
