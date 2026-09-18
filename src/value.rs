use std::rc::Rc;

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
}

#[derive(Debug, Clone)]
pub enum Value {
    Nil,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(Rc<String>),
    Function(Rc<FunctionObj>),
    Native(fn(Vec<Value>, &mut crate::heap::Heap) -> NativeResult),
    ObjRef(usize),
    Tuple(Rc<Vec<Value>>),
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Nil, Value::Nil) => true,
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => a == b,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Str(a), Value::Str(b)) => a == b,
            (Value::Function(a), Value::Function(b)) => Rc::ptr_eq(a, b),
            (Value::ObjRef(a), Value::ObjRef(b)) => a == b,
            (Value::Native(a), Value::Native(b)) => (*a as usize) == (*b as usize),
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
            Value::Function(func) => write!(f, "<func {}>", func.name),
            Value::Native(_) => write!(f, "<native>"),
            Value::ObjRef(id) => write!(f, "<object #{}>", id),
            Value::Tuple(elements) => {
                let items: Vec<String> = elements.iter().map(|e| format!("{}", e)).collect();
                write!(f, "({})", items.join(", "))
            }
        }
    }
}

impl Value {
    pub fn call_method(
        &self,
        method_name: &str,
        args: Vec<Value>,
        heap: &mut Heap,
    ) -> Result<Value, String> {
        match self {
            Value::Str(s) => match method_name {
                "len" => Ok(Value::Int(s.len() as i64)),
                _ => Err(format!("Method '{}' not found on string", method_name)),
            },
            Value::ObjRef(id) => {
                let obj = heap.get_mut(*id)?;
                match obj {
                    Obj::List(list) => match method_name {
                        "push" => {
                            if args.len() != 1 {
                                return Err("'push' expects 1 argument".to_string());
                            }
                            list.push(args[0].clone());
                            Ok(Value::Nil)
                        }
                        "pop" => Ok(list.pop().unwrap_or(Value::Nil)),
                        "len" => Ok(Value::Int(list.len() as i64)),
                        _ => Err(format!("Method '{}' not found on list", method_name)),
                    },
                    Obj::Map(map) => match method_name {
                        "len" => Ok(Value::Int(map.len() as i64)),
                        "keys" => {
                            let keys: Vec<Value> =
                                map.keys().map(|k| Value::Str(Rc::new(k.clone()))).collect();
                            let new_id = heap.alloc(Obj::List(keys)); // Выделяем новый список ключей в куче!
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
            Value::Function(func) => format!("<func {}>", func.name),
            Value::Native(_) => "<native>".to_string(),
            Value::Tuple(elements) => {
                let items: Vec<String> = elements.iter().map(|e| e.stringify(heap)).collect();
                format!("({})", items.join(", "))
            }
            Value::ObjRef(id) => {
                if let Ok(obj) = heap.get(*id) {
                    match obj {
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
                    }
                } else {
                    format!("{}", self)
                }
            }
        }
    }
}
