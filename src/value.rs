use std::{cell::RefCell, collections::HashMap,fmt, rc::Rc};

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
    Map(Rc<RefCell<HashMap<String,  Value>>>),
}

// impl Add for Value {
//     type Output = Value;

//     fn add(self, rhs: Self) -> Self::Output {
//         match (self, rhs) {
//             (Value::Int(a), Value::Int(b)) => Value::Int(a+b),
//             (Value::Float(a), Value::Float(b)) => Value::Float(a+b),
//             (Value::Int(a), Value::Float(b)) => Value::Float(a as f64 + b),
//             (Value::Float(a), Value::Int(b)) => Value::Float(a + b as f64),
//             (Value::Str(a), Value::Str(b)) => {
//                 let mut combined = (*a).clone();
//                 combined.push_str(&b);
//                 Value::Str(Rc::new(combined))
//             }
//             (Value::Str(a), Value::Int(b)) => Value::Str(Rc::new(format!("{}{}",a,b))),
//             (Value::Int(a), Value::Str(b)) => Value::Str(Rc::new(format!("{}{}",a,b))),

//             (Value::Str(x), y) => Value::Str(Rc::new(format!("{}{}", x, y))),
//             (x, Value::Str(y)) => Value::Str(Rc::new(format!("{}{}", x, y))),

//             _ => panic!("Runtime error: invalid types for Add"),

//         }
//     }
// }

// impl Sub for Value {
//     type Output = Value;

//     fn sub(self, rhs: Self) -> Self::Output {
//         match (self, rhs) {
//             (Value::Int(a), Value::Int(b)) => Value::Int(a+b),
//             (Value::Float(a), Value::Float(b)) => Value::Float(a-b),
//             (Value::Int(a), Value::Float(b)) => Value::Float(a as f64 - b),
//             (Value::Float(a), Value::Int(b)) => Value::Float(a - b as f64),
//             _ => panic!("Runtime error: invalid types for Add"),

//         }
//     }
// }

// impl Mul for Value {
//     type Output = Value;

//     fn mul(self, rhs: Self) -> Self::Output {
//         match (self, rhs) {
//             (Value::Int(a), Value::Int(b)) => Value::Int(a*b),
//             (Value::Float(a), Value::Float(b)) => Value::Float(a*b),
//             (Value::Int(a), Value::Float(b)) => Value::Float(a as f64 * b),
//             (Value::Float(a), Value::Int(b)) => Value::Float(a * b as f64),
//             _ => panic!("Runtime error: invalid types for Add"),

//         }
//     }
// }

// impl Div for Value {
//     type Output = Value;

//     fn div(self, rhs: Self) -> Self::Output {
//         match (self, rhs) {
//             (Value::Int(a), Value::Int(b)) => {
//                 if b == 0 { panic!("Runtime error: div by zero"); }
//                 Value::Int(a / b)
//             }
//             (Value::Float(a), Value::Float(b)) => {
//                 if b == 0.0 { panic!("Runtime error: div by zero"); }
//                 Value::Float(a / b)
//             },
//             (Value::Int(a), Value::Float(b)) => {
//                 if b == 0.0 { panic!("Runtime error: div by zero"); }
//                 Value::Float(a as f64 / b)
//             }
//             (Value::Float(a), Value::Int(b)) => {
//                 if b == 0 { panic!("Runtime error: div by zero"); }
//                 Value::Float(a / b as f64)
//             }
//             _ => panic!("Runtime error: invalid types for Add"),

//         }
//     }
// }

// impl Rem for Value {
//     type Output = Value;
//     fn rem(self, rhs: Self) -> Self::Output {
//         match (self, rhs) {
//             (Value::Int(a), Value::Int(b)) => {
//                 if b == 0 { panic!("Runtime error: modulo by zero"); }
//                 Value::Int(a % b)
//             }
//             (Value::Float(a), Value::Float(b)) => {
//                 if b == 0.0 { panic!("Runtime error: modulo by zero"); }
//                 Value::Float(a % b)
//             },
//             (Value::Int(a), Value::Float(b)) => {
//                 if b == 0.0 { panic!("Runtime error: modulo by zero"); }
//                 Value::Float(a as f64 % b)
//             }
//             (Value::Float(a), Value::Int(b)) => {
//                 if b == 0 { panic!("Runtime error: modulo by zero"); }
//                 Value::Float(a % b as f64)
//             }
//             _ => panic!("Runtime error: invalid types for "),

//         }
//     }
// }

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
            },
            Value::Map(map) => {
                let borrowed = map.borrow();
                let items: Vec<String> = borrowed.iter()
                    .map(|(k,v)| {
                        if let Value::Str(s) = v {
                            format!("\"{}\": \"{}\"",k, s)
                        } else {
                            format!("\"{}\": {}",k, v)
                        }
                    })
                    .collect();
                write!(f, "{{{}}}", items.join(", "))
            }

        }    
    }
}

impl Value {
    pub fn call_method(&self, method_name: &str, args: Vec<Value>) -> Result<Value, String> {
        match (self, method_name) {
            
            // Arrays methods
            (Value::List(list), "push") => {
                if args.len() != 1 { return Err("'push' expects 1 argument".to_string()); }
                list.borrow_mut().push(args[0].clone());
                Ok(Value::Nil)
            }
            (Value::List(list), "pop") => {
                if args.len() != 0 { return Err("'pop' expects 0 arguments".to_string()); }
                let val = list.borrow_mut().pop().unwrap_or(Value::Nil);
                Ok(val)
            }
            (Value::List(list), "len") => {
                Ok(Value::Int(list.borrow().len() as i64))
            }


            // Map  methods
            (Value::Map(map), "keys") => {
                let keys: Vec<Value> = map.borrow().keys()
                    .map(|k| Value::Str(Rc::new(k.clone())))
                    .collect();
                Ok(Value::List(Rc::new(std::cell::RefCell::new(keys))))
            }
            (Value::Map(map), "len") => {
                Ok(Value::Int(map.borrow().len() as i64))
            }

            // Strings methods
            (Value::Str(s), "len") => {
                Ok(Value::Int(s.len() as i64))
            }

            _ => Err(format!("Method '{}' not found on this type", method_name))
        }
    }
    
}