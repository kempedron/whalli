use std::{cell::RefCell, fmt, ops::{Add, Div, Mul, Sub}, rc::Rc};

use crate::opcode::OpCode;

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionObj {
    pub name: String,
    pub arity: usize,
    pub chunk: Vec<OpCode>,
    pub param_types: Vec<String>,
}
#[derive(Debug, Clone)]
pub enum Value {
    Nil,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(Rc<String>),
    List(Rc<RefCell<Vec<Value>>>),
    Function(Rc<FunctionObj>),
    Native(fn(Vec<Value>) -> Value),
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
            (Value::Str(a), Value::Int(b)) => Value::Str(Rc::new(format!("{}{}",a,b))),
            (Value::Int(a), Value::Str(b)) => Value::Str(Rc::new(format!("{}{}",a,b))),

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

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Nil, Value::Nil) => true,
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => a == b,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Str(a), Value::Str(b)) => a == b,
            // Для функций и списков сравниваем просто их адреса (ссылки Rc)
            (Value::Function(a), Value::Function(b)) => Rc::ptr_eq(a, b),
            (Value::List(a), Value::List(b)) => Rc::ptr_eq(a, b),
            // Сравниваем нативные функции приведением их адресов к числам
            (Value::Native(a), Value::Native(b)) => (*a as usize) == (*b as usize),
            _ => false,
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Nil => write!(f, "nil"),
            Value::Int(n) => write!(f,"{}",n),
            Value::Float(n) => write!(f,"{}",n),
            Value::Str(n) => write!(f,"{}",n),
            Value::Bool(n) => write!(f,"{}",n),
            Value::Function(func) => write!(f,"<func{}>",func.name),
            Value::Native(_) => write!(f, "<native func>"),
            Value::List(list) => {
                let borrowed = list.borrow();
                write!(f,"[")?;
                for (i,val) in borrowed.iter().enumerate() {
                    if i > 0{
                        write!(f, ", ")?;
                    }
                    write!(f, "{}",val)?;
                }
                write!(f, "]")?;
                Ok(())
            }

        }    
    }
}
