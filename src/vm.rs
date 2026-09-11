use crate::{opcode::OpCode, value::{FunctionObj, Value}};
use std::{collections::HashMap, rc::Rc};


pub struct CallFrame {
    pub function: Rc<FunctionObj>,
    pub ip: usize,
    pub stack_offset: usize
}

pub struct VM{
    stack: Vec<Value>,
    frames: Vec<CallFrame>,
    globals: HashMap<String, Value>,
}


impl VM {
    pub fn new(instructions: Vec<OpCode>) -> Self {
        let main_func = FunctionObj {
            name: "main".to_string(),
            arity: 0,
            chunk: instructions,
            param_types: vec![],
        };

        let initial_frame = CallFrame {
            function: Rc::new(main_func),
            ip: 0,
            stack_offset: 0,
        };

        VM {
            stack: Vec::new(),
            frames: vec![initial_frame],
            globals: HashMap::new(),
        }
    }

    pub fn run(&mut self){
        
        while !self.frames.is_empty() {
            let frame_idx = self.frames.len() - 1;
            let frame = &mut self.frames[frame_idx];

            if frame.ip >= frame.function.chunk.len(){
                self.frames.pop();
                continue;
            }

            let instractions = frame.function.chunk[frame.ip].clone();
            frame.ip+=1;

            match instractions {
                OpCode::Push(val) => {
                    self.stack.push(val.clone());
                }

                OpCode::Add => {
                    let b = self.stack.pop().expect("Error: stack empty");
                    let a = self.stack.pop().expect("Error: stack empty");
                    self.stack.push(a+b);
                }
                OpCode::Sub => {
                    let b = self.stack.pop().expect("Error: stack empty");
                    let a = self.stack.pop().expect("Error: stack empty");
                    self.stack.push(a-b);
                }
                OpCode::Mul => {
                    let b = self.stack.pop().expect("Error: stack empty");
                    let a = self.stack.pop().expect("Error: stack empty");
                    self.stack.push(a*b);
                }
                OpCode::Div => {
                    let b = self.stack.pop().expect("Error: stack empty");
                    let a = self.stack.pop().expect("Error: stack empty");
                    self.stack.push(a/b);
                }

                OpCode::Print => {
                    let val = self.stack.pop().expect("Error: stack empty");
                    match val {
                        Value::Int(n) => print!("{}",n),
                        Value::Float(n) => print!("{}",n),
                        Value::Bool(n) => print!("{}",n),
                        Value::Str(n) => print!("{}",n),
                        Value::List(n) => print!("{:?}",n),
                        _ => panic!("invalid type: {:?}",val),
                    }
                }
                OpCode::StoreGlobal(name) => {
                    let val = self.stack.pop().expect("Error: stack empty for assignment");
                    self.globals.insert(name.clone(), val);
                }
                OpCode::LoadGlobal(name) => {
                    let val = self.globals.get(&name.to_string()).expect(&format!("Runtime error: undefined variable {}",name));
                    self.stack.push(val.clone());
                }
                OpCode::LoadLocal(idx) => {
                    let val = self.stack[frame.stack_offset + idx].clone();
                    self.stack.push(val);
                }
                OpCode::Call(arg_count) => {
                    let callee_index = self.stack.len() - arg_count - 1;
                    let callee = self.stack[callee_index].clone();

                    match callee {
                        Value::Function(func) => {
                            if func.arity != arg_count {
                                panic!("Runtime error: function '{}' expects {} arguments, got {}",func.name, func.arity, arg_count);
                            }

                            let new_frame = CallFrame {
                                function: func,
                                ip: 0,
                                stack_offset: callee_index + 1,
                            };
                            self.frames.push(new_frame);
                        }
                        _ => panic!("Runtime error: attempt to call a non-function value")
                    }
                }
                OpCode::Return => {
                    let result = self.stack.pop().unwrap_or(Value::Nil);

                    let frame = self.frames.pop().unwrap();
                    
                    if frame.stack_offset > 0 {
                        self.stack.truncate(frame.stack_offset - 1);
                    }
                    if !self.frames.is_empty() {
                        self.stack.push(result);
                    }
                }
                OpCode::JumpIfFalse(target_ip) => {
                    let condition = self.stack.pop().expect("Stack empty");
                    if let Value::Bool(false) = condition {
                        frame.ip = target_ip;
                    }
                }
                OpCode::Jump(target_ip) => {
                    frame.ip = target_ip;
                }
                OpCode::Equal => {
                    let b = self.stack.pop().expect("Error: stack empty");
                    let a = self.stack.pop().expect("Error: stack empty");
                    self.stack.push(Value::Bool(a == b));
                }
                OpCode::Less => {
                    let b = self.stack.pop().expect("Error: stack empty");
                    let a = self.stack.pop().expect("Error: stack empty");
                    let res = match(a, b) {
                        (Value::Int(x), Value::Int(y)) => x < y,
                        (Value::Float(x), Value::Float(y)) => x < y,
                        (Value::Int(x), Value::Float(y)) => (x as f64) < y,
                        (Value::Float(x), Value::Int(y)) => x < (y as f64),
                        _ => panic!("Runtime error: invalid types for '<' operation")
                    };
                    self.stack.push(Value::Bool(res));
                }
                OpCode::Greater => {
                    let b = self.stack.pop().expect("Error: stack empty");
                    let a = self.stack.pop().expect("Error: stack empty");
                    let res = match(a, b) {
                        (Value::Int(x), Value::Int(y)) => x > y,
                        (Value::Float(x), Value::Float(y)) => x > y,
                        (Value::Int(x), Value::Float(y)) => (x as f64) > y,
                        (Value::Float(x), Value::Int(y)) => x > (y as f64),
                        _ => panic!("Runtime error: invalid types for '>' operation")
                    };
                    self.stack.push(Value::Bool(res));
                }
                OpCode::BuildList(size) => {
                    let start = self.stack.len() - size;
                    let elements: Vec<Value> = self.stack.drain(start..).collect();
                    self.stack.push(Value::List(Rc::new(std::cell::RefCell::new(elements))));
                }
                OpCode::ListLen => {
                    let val = self.stack.pop().expect("Stack empty");
                    if let Value::List(list) = val {
                        let len = list.borrow().len() as i64;
                        self.stack.push(Value::Int(len));
                    } else {
                        panic!("Runtime error: Attempt to get length of a non-list");
                    }
                }
                OpCode::IndexGet => {
                    let index = self.stack.pop().expect("Stack empty");
                    let array = self.stack.pop().expect("Stack empty");

                    match (array, index) {
                        (Value::List(list), Value::Int(idx)) => {
                            let borrowed = list.borrow();
                            if idx < 0 || idx >= borrowed.len() as i64 {
                                panic!("Runtime error: Index {} out of bounds", idx);
                            }
                            self.stack.push(borrowed[idx as usize].clone());
                        }
                        _ => panic!("Runtime error: Invalid array or index"),
                    }
                }


            }
        }
    }
}