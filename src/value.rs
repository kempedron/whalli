use std::{cell::RefCell, ops::{Add,Mul,Div,Sub}, rc::Rc};

use crate::opcode::OpCode;

#[derive(Debug, Clone, PartialEq)]

pub struct FunctionObj {
    pub name: String,
    pub arity: usize,
    pub chunk: Vec<OpCode>,
    pub param_types: Vec<String>,
}
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Nil,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(Rc<String>),
    Pointer(Rc<RefCell<Value>>),
    Function(Rc<FunctionObj>),
}

impl Add for Value {
    type Output = Value;

    fn add(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Value::Int(a), Value::Int(b)) => Value::Int(a+b),
            (Value::Float(a), Value::Float(b)) => Value::Float(a+b),
            (Value::Int(a), Value::Float(b)) => Value::Float(a as f64 + b),
            (Value::Float(a), Value::Int(b)) => Value::Float(a + b as f64),
            (Value::Str(a), Value::Str(b)) => {
                let mut combined = (*a).clone();
                combined.push_str(&b);
                Value::Str(Rc::new(combined))
            }
            _ => panic!("Runtime error: invalid types for Add"),

        }
    }
}

impl Sub for Value {
    type Output = Value;

    fn sub(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Value::Int(a), Value::Int(b)) => Value::Int(a+b),
            (Value::Float(a), Value::Float(b)) => Value::Float(a-b),
            (Value::Int(a), Value::Float(b)) => Value::Float(a as f64 - b),
            (Value::Float(a), Value::Int(b)) => Value::Float(a - b as f64),
            _ => panic!("Runtime error: invalid types for Add"),

        }
    }
}

impl Mul for Value {
    type Output = Value;

    fn mul(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Value::Int(a), Value::Int(b)) => Value::Int(a*b),
            (Value::Float(a), Value::Float(b)) => Value::Float(a*b),
            (Value::Int(a), Value::Float(b)) => Value::Float(a as f64 * b),
            (Value::Float(a), Value::Int(b)) => Value::Float(a * b as f64),
            _ => panic!("Runtime error: invalid types for Add"),

        }
    }
}

impl Div for Value {
    type Output = Value;

    fn div(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Value::Int(a), Value::Int(b)) => Value::Int(a/b),
            (Value::Float(a), Value::Float(b)) => Value::Float(a/b),
            (Value::Int(a), Value::Float(b)) => Value::Float(a as f64 / b),
            (Value::Float(a), Value::Int(b)) => Value::Float(a / b as f64),
            _ => panic!("Runtime error: invalid types for Add"),

        }
    }
}


