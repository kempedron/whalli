use mio::Token;
use std::collections::HashMap;
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
                "json" => {
                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(s.as_str()) {
                        Ok(crate::stdlib::json::json_to_value(parsed, heap))
                    } else {
                        Ok(Value::Nil)
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
                        "header" => {
                            if args.is_empty() {
                                return Err("'header' expects header name string".to_string());
                            }
                            let key = args[0].stringify(heap).to_lowercase();
                            let default_val = args.get(1).cloned().unwrap_or(Value::Nil);
                            heap.with_read(*id, |obj| {
                                if let Obj::Map(m) = obj {
                                    if let Some(Value::ObjRef(h_id)) = m.get("headers") {
                                        if let Ok(Obj::Map(h_map)) = heap.get(*h_id) {
                                            return Ok(h_map.get(&key).cloned().unwrap_or(default_val));
                                        }
                                    }
                                }
                                Ok(default_val)
                            })?
                        }
                        "cookie" => {
                            if args.is_empty() {
                                return Err("'cookie' expects cookie name string".to_string());
                            }
                            let key = match &args[0] {
                                Value::Str(s) => s.as_str(),
                                _ => return Err("Cookie name must be a string".to_string()),
                            };
                            let default_val = args.get(1).cloned().unwrap_or(Value::Nil);
                            heap.with_read(*id, |obj| {
                                if let Obj::Map(m) = obj {
                                    if let Some(Value::ObjRef(c_id)) = m.get("cookies") {
                                        if let Ok(Obj::Map(c_map)) = heap.get(*c_id) {
                                            return Ok(c_map.get(key).cloned().unwrap_or(default_val));
                                        }
                                    }
                                }
                                Ok(default_val)
                            })?
                        }
                        "close" => {
                            let db_id_opt = heap.with_read(*id, |obj| {
                                if let Obj::Map(map) = obj {
                                    if let Some(Value::Int(db_id)) = map.get("db_id") {
                                        return Some(*db_id as usize);
                                    }
                                }
                                None
                            })?;

                            if let Some(conn_id) = db_id_opt {
                                let ok = crate::stdlib::sql::db_manager().close(conn_id);
                                Ok(Value::Bool(ok))
                            } else {
                                Err("Method 'close' not found on map".to_string())
                            }
                        }
                        "exec" => {
                            let db_id = heap.with_read(*id, |obj| {
                                if let Obj::Map(map) = obj {
                                    if let Some(Value::Int(db_id)) = map.get("db_id") {
                                        return Ok(*db_id as usize);
                                    }
                                }
                                Err("Method 'exec' only valid on database objects".to_string())
                            })??;

                            if args.is_empty() {
                                return Err("'exec' expects SQL query string".to_string());
                            }
                            let query = match &args[0] {
                                Value::Str(s) => s.as_str(),
                                _ => return Err("SQL query must be a string".to_string()),
                            };
                            let params = crate::stdlib::sql::convert_sql_args(args.get(1), heap);
                            match crate::stdlib::sql::db_exec_internal(db_id, query, params, heap) {
                                Ok(res_val) => Ok(Value::Tuple(Arc::new(vec![res_val, Value::Nil]))),
                                Err(e) => Ok(Value::Tuple(Arc::new(vec![Value::Nil, Value::Str(Arc::new(e))]))),
                            }
                        }
                        "query" => {
                            let is_db = heap.with_read(*id, |obj| {
                                if let Obj::Map(map) = obj {
                                    map.contains_key("db_id")
                                } else {
                                    false
                                }
                            }).unwrap_or(false);

                            if is_db {
                                let db_id = heap.with_read(*id, |obj| {
                                    if let Obj::Map(map) = obj {
                                        if let Some(Value::Int(db_id)) = map.get("db_id") {
                                            return Ok(*db_id as usize);
                                        }
                                    }
                                    Err("Database object missing db_id".to_string())
                                })??;

                                if args.is_empty() {
                                    return Err("'query' expects SQL query string".to_string());
                                }
                                let query = match &args[0] {
                                    Value::Str(s) => s.as_str(),
                                    _ => return Err("SQL query must be a string".to_string()),
                                };
                                let params = crate::stdlib::sql::convert_sql_args(args.get(1), heap);
                                match crate::stdlib::sql::db_query_internal(db_id, query, params, heap) {
                                    Ok(rows_val) => Ok(Value::Tuple(Arc::new(vec![rows_val, Value::Nil]))),
                                    Err(e) => Ok(Value::Tuple(Arc::new(vec![Value::Nil, Value::Str(Arc::new(e))]))),
                                }
                            } else {
                                if args.is_empty() {
                                    return Err("'query' expects parameter name string".to_string());
                                }
                                let key = match &args[0] {
                                    Value::Str(s) => s.as_str(),
                                    _ => return Err("Query parameter name must be a string".to_string()),
                                };
                                let default_val = args.get(1).cloned().unwrap_or(Value::Nil);
                                heap.with_read(*id, |obj| {
                                    if let Obj::Map(m) = obj {
                                        if let Some(Value::ObjRef(q_id)) = m.get("query") {
                                            if let Ok(Obj::Map(q_map)) = heap.get(*q_id) {
                                                return Ok(q_map.get(key).cloned().unwrap_or(default_val));
                                            }
                                        }
                                    }
                                    Ok(default_val)
                                })?
                            }
                        }
                        "query_row" => {
                            let db_id = heap.with_read(*id, |obj| {
                                if let Obj::Map(map) = obj {
                                    if let Some(Value::Int(db_id)) = map.get("db_id") {
                                        return Ok(*db_id as usize);
                                    }
                                }
                                Err("Method 'query_row' only valid on database objects".to_string())
                            })??;

                            if args.is_empty() {
                                return Err("'query_row' expects SQL query string".to_string());
                            }
                            let query = match &args[0] {
                                Value::Str(s) => s.as_str(),
                                _ => return Err("SQL query must be a string".to_string()),
                            };
                            let params = crate::stdlib::sql::convert_sql_args(args.get(1), heap);
                            match crate::stdlib::sql::db_query_row_internal(db_id, query, params, heap) {
                                Ok(row_val) => Ok(Value::Tuple(Arc::new(vec![row_val, Value::Nil]))),
                                Err(e) => Ok(Value::Tuple(Arc::new(vec![Value::Nil, Value::Str(Arc::new(e))]))),
                            }
                        }
                        "param" => {
                            if args.is_empty() {
                                return Err("'param' expects route parameter name string".to_string());
                            }
                            let key = match &args[0] {
                                Value::Str(s) => s.as_str(),
                                _ => return Err("Param name must be a string".to_string()),
                            };
                            let default_val = args.get(1).cloned().unwrap_or(Value::Nil);
                            heap.with_read(*id, |obj| {
                                if let Obj::Map(m) = obj {
                                    if let Some(Value::ObjRef(p_id)) = m.get("params") {
                                        if let Ok(Obj::Map(p_map)) = heap.get(*p_id) {
                                            return Ok(p_map.get(key).cloned().unwrap_or(default_val));
                                        }
                                    }
                                }
                                Ok(default_val)
                            })?
                        }
                        "form" => {
                            if args.is_empty() {
                                let (existing_form_id, body_str) = heap.with_read(*id, |obj| {
                                    if let Obj::Map(m) = obj {
                                        let f_id = match m.get("form") {
                                            Some(Value::ObjRef(id)) => Some(*id),
                                            _ => None,
                                        };
                                        let b = match m.get("body") {
                                            Some(Value::Str(s)) => Some(s.as_str().to_string()),
                                            _ => None,
                                        };
                                        (f_id, b)
                                    } else {
                                        (None, None)
                                    }
                                })?;

                                if let Some(f_id) = existing_form_id {
                                    let is_empty = heap.with_read(f_id, |obj| {
                                        if let Obj::Map(m) = obj { m.is_empty() } else { true }
                                    }).unwrap_or(true);

                                    if !is_empty {
                                        return Ok(Value::ObjRef(f_id));
                                    }

                                    if let Some(body) = body_str {
                                        if !body.is_empty() {
                                            let parsed = crate::stdlib::http::parse_query_pairs(&body);
                                            let _ = heap.with_write(f_id, |obj| {
                                                if let Obj::Map(m) = obj {
                                                    *m = parsed;
                                                }
                                                Ok::<(), String>(())
                                            });
                                        }
                                    }
                                    return Ok(Value::ObjRef(f_id));
                                }
                                return Ok(Value::Nil);
                            }
                            let key = match &args[0] {
                                Value::Str(s) => s.as_str().to_string(),
                                _ => return Err("Form field name must be a string".to_string()),
                            };
                            let default_val = args.get(1).cloned().unwrap_or(Value::Nil);
                            let val = heap.with_read(*id, |obj| {
                                if let Obj::Map(m) = obj {
                                    if let Some(Value::ObjRef(f_id)) = m.get("form") {
                                        if let Ok(Obj::Map(f_map)) = heap.get(*f_id) {
                                            if let Some(v) = f_map.get(&key) {
                                                return Some(v.clone());
                                            }
                                        }
                                    }
                                    if let Some(Value::Str(b)) = m.get("body") {
                                        let parsed = crate::stdlib::http::parse_query_pairs(b.as_str());
                                        if let Some(v) = parsed.get(&key) {
                                            return Some(v.clone());
                                        }
                                    }
                                }
                                None
                            })?;
                            Ok(val.unwrap_or(default_val))
                        }
                        "form_value" => {
                            if args.is_empty() {
                                return Err("'form_value' expects field name string".to_string());
                            }
                            let key = match &args[0] {
                                Value::Str(s) => s.as_str().to_string(),
                                _ => return Err("Form field name must be a string".to_string()),
                            };
                            let default_val = args.get(1).cloned().unwrap_or(Value::Nil);
                            let val = heap.with_read(*id, |obj| {
                                if let Obj::Map(m) = obj {
                                    if let Some(Value::ObjRef(f_id)) = m.get("form") {
                                        if let Ok(Obj::Map(f_map)) = heap.get(*f_id) {
                                            if let Some(v) = f_map.get(&key) {
                                                return Some(v.clone());
                                            }
                                        }
                                    }
                                    if let Some(Value::Str(b)) = m.get("body") {
                                        let parsed = crate::stdlib::http::parse_query_pairs(b.as_str());
                                        if let Some(v) = parsed.get(&key) {
                                            return Some(v.clone());
                                        }
                                    }
                                }
                                None
                            })?;
                            Ok(val.unwrap_or(default_val))
                        }
                        "use" => {
                            if args.is_empty() {
                                return Err("'use' expects at least 1 middleware function".to_string());
                            }
                            heap.with_write(*id, |obj| {
                                if let Obj::Map(map) = obj {
                                    let mws_val = map.entry("middlewares".to_string()).or_insert_with(|| {
                                        let list_id = heap.alloc(Obj::List(Vec::new()));
                                        Value::ObjRef(list_id)
                                    });
                                    if let Value::ObjRef(list_id) = mws_val {
                                        let list_id = *list_id;
                                        heap.with_write(list_id, |l_obj| {
                                            if let Obj::List(list) = l_obj {
                                                for arg in &args {
                                                    if let Value::ObjRef(arg_id) = arg {
                                                        if let Ok(Obj::List(sub_list)) = heap.get(*arg_id) {
                                                            list.extend(sub_list.clone());
                                                            continue;
                                                        }
                                                    }
                                                    list.push(arg.clone());
                                                }
                                                Ok(Value::Nil)
                                            } else {
                                                Err("Invalid middlewares list".to_string())
                                            }
                                        })?
                                    } else {
                                        Err("Invalid middlewares reference".to_string())
                                    }
                                } else {
                                    Err("Expected map object".to_string())
                                }
                            })?
                        }
                        "group" => {
                            if args.is_empty() {
                                return Err("'group' expects prefix string argument".to_string());
                            }
                            let sub_prefix = match &args[0] {
                                Value::Str(s) => s.as_str().to_string(),
                                _ => return Err("Group prefix must be a string".to_string()),
                            };

                            let (root_id, full_prefix, inherited_mws) = heap.with_read(*id, |obj| {
                                if let Obj::Map(m) = obj {
                                    let parent_id = match m.get("parent") {
                                        Some(Value::ObjRef(p_id)) => *p_id,
                                        _ => *id,
                                    };
                                    let prefix = match m.get("prefix") {
                                        Some(Value::Str(p)) => {
                                            let p1 = p.trim_end_matches('/');
                                            let p2 = sub_prefix.trim_start_matches('/');
                                            if p1.is_empty() {
                                                format!("/{}", p2)
                                            } else if p2.is_empty() {
                                                p1.to_string()
                                            } else {
                                                format!("{}/{}", p1, p2)
                                            }
                                        }
                                        _ => {
                                            if sub_prefix.starts_with('/') {
                                                sub_prefix.clone()
                                            } else {
                                                format!("/{}", sub_prefix)
                                            }
                                        }
                                    };
                                    let mut mws = Vec::new();
                                    if let Some(Value::ObjRef(mws_id)) = m.get("middlewares") {
                                        if let Ok(Obj::List(list)) = heap.get(*mws_id) {
                                            mws = list.clone();
                                        }
                                    }
                                    (parent_id, prefix, mws)
                                } else {
                                    (*id, sub_prefix.clone(), Vec::new())
                                }
                            })?;

                            let mut grp_map = HashMap::new();
                            grp_map.insert("prefix".to_string(), Value::Str(Arc::new(full_prefix)));
                            grp_map.insert("parent".to_string(), Value::ObjRef(root_id));
                            let mws_id = heap.alloc(Obj::List(inherited_mws));
                            grp_map.insert("middlewares".to_string(), Value::ObjRef(mws_id));
                            let grp_id = heap.alloc(Obj::Map(grp_map));
                            Ok(Value::ObjRef(grp_id))
                        }
                        "not_found" => {
                            if args.is_empty() {
                                return Err("'not_found' expects a handler function".to_string());
                            }
                            let target_id = heap.with_read(*id, |obj| {
                                if let Obj::Map(m) = obj {
                                    if let Some(Value::ObjRef(p_id)) = m.get("parent") {
                                        *p_id
                                    } else {
                                        *id
                                    }
                                } else {
                                    *id
                                }
                            })?;
                            heap.with_write(target_id, |obj| {
                                if let Obj::Map(map) = obj {
                                    map.insert("not_found".to_string(), args[0].clone());
                                    Ok(Value::Nil)
                                } else {
                                    Err("Expected map object".to_string())
                                }
                            })?
                        }
                        "method_not_allowed" => {
                            if args.is_empty() {
                                return Err("'method_not_allowed' expects a handler function".to_string());
                            }
                            let target_id = heap.with_read(*id, |obj| {
                                if let Obj::Map(m) = obj {
                                    if let Some(Value::ObjRef(p_id)) = m.get("parent") {
                                        *p_id
                                    } else {
                                        *id
                                    }
                                } else {
                                    *id
                                }
                            })?;
                            heap.with_write(target_id, |obj| {
                                if let Obj::Map(map) = obj {
                                    map.insert("method_not_allowed".to_string(), args[0].clone());
                                    Ok(Value::Nil)
                                } else {
                                    Err("Expected map object".to_string())
                                }
                            })?
                        }
                        "static" => {
                            if args.len() < 2 {
                                return Err("'static' expects 2 arguments: (prefix: str, dir: str)".to_string());
                            }
                            register_static_route(*id, &args[0], &args[1], heap)
                        }
                        "json" => {
                            let text_str = heap.with_read(*id, |obj| {
                                if let Obj::Map(map) = obj {
                                    if let Some(Value::Str(s)) = map.get("text").or_else(|| map.get("body")) {
                                        Some((**s).clone())
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                }
                            })?;

                            if let Some(text) = text_str {
                                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&text) {
                                    Ok(crate::stdlib::json::json_to_value(parsed, heap))
                                } else {
                                    Ok(Value::Nil)
                                }
                            } else {
                                Ok(Value::Nil)
                            }
                        }
                        "get" => {
                            let is_router_like = heap.with_read(*id, |obj| {
                                if let Obj::Map(map) = obj {
                                    map.contains_key("routes") || map.contains_key("parent")
                                } else {
                                    false
                                }
                            }).unwrap_or(false);

                            if args.len() == 1 {
                                if let Value::Str(key) = &args[0] {
                                    heap.with_read(*id, |obj| {
                                        if let Obj::Map(map) = obj {
                                            Ok(map.get(&**key).cloned().unwrap_or(Value::Nil))
                                        } else {
                                            unreachable!()
                                        }
                                    })?
                                } else {
                                    Err("Map key must be string".to_string())
                                }
                            } else if args.len() >= 2 {
                                if is_router_like {
                                    register_router_route(*id, "GET", &args[0], &args[1], heap)
                                } else {
                                    if let Value::Str(key) = &args[0] {
                                        heap.with_read(*id, |obj| {
                                            if let Obj::Map(map) = obj {
                                                Ok(map.get(&**key).cloned().unwrap_or_else(|| args[1].clone()))
                                            } else {
                                                unreachable!()
                                            }
                                        })?
                                    } else {
                                        Err("Map key must be string".to_string())
                                    }
                                }
                            } else {
                                Err("'get' expects 1 argument (key: str) or 2 arguments (key: str, default: any)".to_string())
                            }
                        }
                        "post" => {
                            if args.len() < 2 { return Err("'post' expects 2 arguments: (path: str, handler: func)".to_string()); }
                            register_router_route(*id, "POST", &args[0], &args[1], heap)
                        }
                        "put" => {
                            if args.len() < 2 { return Err("'put' expects 2 arguments: (path: str, handler: func)".to_string()); }
                            register_router_route(*id, "PUT", &args[0], &args[1], heap)
                        }
                        "delete" => {
                            if args.len() < 2 { return Err("'delete' expects 2 arguments: (path: str, handler: func)".to_string()); }
                            register_router_route(*id, "DELETE", &args[0], &args[1], heap)
                        }
                        "patch" => {
                            if args.len() < 2 { return Err("'patch' expects 2 arguments: (path: str, handler: func)".to_string()); }
                            register_router_route(*id, "PATCH", &args[0], &args[1], heap)
                        }
                        "head" => {
                            if args.len() < 2 { return Err("'head' expects 2 arguments: (path: str, handler: func)".to_string()); }
                            register_router_route(*id, "HEAD", &args[0], &args[1], heap)
                        }
                        "options" => {
                            if args.len() < 2 { return Err("'options' expects 2 arguments: (path: str, handler: func)".to_string()); }
                            register_router_route(*id, "OPTIONS", &args[0], &args[1], heap)
                        }
                        "handle" => {
                            if args.len() < 3 {
                                return Err("'handle' expects 3 arguments: (method: str, path: str, handler: func)".to_string());
                            }
                            let m = match &args[0] {
                                Value::Str(s) => s.as_str().to_uppercase(),
                                _ => return Err("HTTP method must be a string".to_string()),
                            };
                            register_router_route(*id, &m, &args[1], &args[2], heap)
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
}

fn find_root_router_and_path(
    mut cur_id: usize,
    raw_path: &str,
    heap: &Heap,
) -> Result<(usize, String, Vec<Value>), String> {
    let mut parts = vec![raw_path.trim_matches('/').to_string()];
    let mut all_mws = Vec::new();

    loop {
        let (parent_opt, prefix_opt, mws_opt) = heap.with_read(cur_id, |obj| {
            if let Obj::Map(m) = obj {
                let p = m.get("parent").and_then(|v| if let Value::ObjRef(id) = v { Some(*id) } else { None });
                let pref = m.get("prefix").and_then(|v| if let Value::Str(s) = v { Some(s.as_str().to_string()) } else { None });
                let mws = m.get("middlewares").and_then(|v| if let Value::ObjRef(id) = v { Some(*id) } else { None });
                (p, pref, mws)
            } else {
                (None, None, None)
            }
        })?;

        if let Some(mws_id) = mws_opt {
            let mws = heap.with_read(mws_id, |obj| {
                if let Obj::List(l) = obj { l.clone() } else { Vec::new() }
            }).unwrap_or_default();
            let mut combined = mws;
            combined.extend(all_mws);
            all_mws = combined;
        }

        if let Some(prefix) = prefix_opt {
            let clean = prefix.trim_matches('/').to_string();
            if !clean.is_empty() {
                parts.insert(0, clean);
            }
        }

        if let Some(parent_id) = parent_opt {
            cur_id = parent_id;
        } else {
            break;
        }
    }

    let joined = parts.into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join("/");
    let full_path = format!("/{}", joined);
    Ok((cur_id, full_path, all_mws))
}

fn register_router_route(
    router_id: usize,
    method: &str,
    path_val: &Value,
    handler_val: &Value,
    heap: &Heap,
) -> Result<Value, String> {
    let raw_path = match path_val {
        Value::Str(s) => s.as_str().to_string(),
        _ => return Err("Route path must be a string".to_string()),
    };

    let (root_id, full_path, group_mws) = find_root_router_and_path(router_id, &raw_path, heap)?;

    // Get or create routes map ID
    let routes_id_opt = heap.with_read(root_id, |obj| {
        if let Obj::Map(m) = obj {
            m.get("routes").and_then(|v| if let Value::ObjRef(id) = v { Some(*id) } else { None })
        } else {
            None
        }
    })?;

    let routes_id = match routes_id_opt {
        Some(id) => id,
        None => {
            let new_routes_id = heap.alloc(Obj::Map(HashMap::new()));
            let _ = heap.with_write(root_id, |obj| {
                if let Obj::Map(m) = obj {
                    m.insert("routes".to_string(), Value::ObjRef(new_routes_id));
                }
            });
            new_routes_id
        }
    };

    // Get or create method routes map ID
    let method_routes_id_opt = heap.with_read(routes_id, |obj| {
        if let Obj::Map(m) = obj {
            m.get(method).and_then(|v| if let Value::ObjRef(id) = v { Some(*id) } else { None })
        } else {
            None
        }
    })?;

    let method_routes_id = match method_routes_id_opt {
        Some(id) => id,
        None => {
            let new_m_id = heap.alloc(Obj::Map(HashMap::new()));
            let _ = heap.with_write(routes_id, |obj| {
                if let Obj::Map(m) = obj {
                    m.insert(method.to_string(), Value::ObjRef(new_m_id));
                }
            });
            new_m_id
        }
    };

    // Insert route into method routes
    let _ = heap.with_write(method_routes_id, |obj| {
        if let Obj::Map(m) = obj {
            m.insert(full_path.clone(), handler_val.clone());
        }
    });

    // If group has middlewares, attach them to route_middlewares
    if !group_mws.is_empty() {
        let rmws_id_opt = heap.with_read(root_id, |obj| {
            if let Obj::Map(m) = obj {
                m.get("route_middlewares").and_then(|v| if let Value::ObjRef(id) = v { Some(*id) } else { None })
            } else {
                None
            }
        })?;

        let rmws_id = match rmws_id_opt {
            Some(id) => id,
            None => {
                let new_rm_id = heap.alloc(Obj::Map(HashMap::new()));
                let _ = heap.with_write(root_id, |obj| {
                    if let Obj::Map(m) = obj {
                        m.insert("route_middlewares".to_string(), Value::ObjRef(new_rm_id));
                    }
                });
                new_rm_id
            }
        };

        let list_id = heap.alloc(Obj::List(group_mws));
        let _ = heap.with_write(rmws_id, |obj| {
            if let Obj::Map(m) = obj {
                m.insert(full_path, Value::ObjRef(list_id));
            }
        });
    }

    Ok(Value::Nil)
}

fn register_static_route(
    router_id: usize,
    prefix_val: &Value,
    dir_val: &Value,
    heap: &Heap,
) -> Result<Value, String> {
    let prefix = match prefix_val {
        Value::Str(s) => s.as_str().to_string(),
        _ => return Err("Static prefix must be a string".to_string()),
    };
    let dir = match dir_val {
        Value::Str(s) => s.as_str().to_string(),
        _ => return Err("Static dir must be a string".to_string()),
    };

    let p = prefix.trim_end_matches('/');
    let normalized = if p.starts_with('/') { p.to_string() } else { format!("/{}", p) };
    let wildcard = format!("{}/*filepath", normalized);

    let mut static_info = std::collections::HashMap::new();
    static_info.insert("static_dir".to_string(), Value::Str(Arc::new(dir)));
    static_info.insert("prefix".to_string(), Value::Str(Arc::new(normalized.clone())));
    let static_id = heap.alloc(Obj::Map(static_info));
    let static_val = Value::ObjRef(static_id);

    register_router_route(router_id, "GET", &Value::Str(Arc::new(wildcard.clone())), &static_val, heap)?;
    register_router_route(router_id, "HEAD", &Value::Str(Arc::new(wildcard)), &static_val, heap)?;

    if !normalized.is_empty() && normalized != "/" {
        register_router_route(router_id, "GET", &Value::Str(Arc::new(normalized.clone())), &static_val, heap)?;
        register_router_route(router_id, "HEAD", &Value::Str(Arc::new(normalized)), &static_val, heap)?;
    }

    Ok(Value::Nil)
}

impl Value {
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
