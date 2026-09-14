use std::collections::HashMap;
use std::rc::Rc;
use std::cell::RefCell;
use crate::value::Value;

pub fn register() -> Value {
    let mut math_module = HashMap::new();
    
    math_module.insert("pi".to_string(), Value::Float(std::f64::consts::PI));
    math_module.insert("sin".to_string(), Value::Native(|args| {
        if let Value::Float(n) = args[0] {
            Value::Float(n.sin())
        } else {
            Value::Nil
        }
    }));

    Value::Map(Rc::new(RefCell::new(math_module)))
}