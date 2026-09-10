use crate::{value::Value, opcode::OpCode};
use std::collections::HashMap;

pub struct VM{
    instructions: Vec<OpCode>,
    stack: Vec<Value>,
    globals: HashMap<String, Value>,
    ip: usize,
}

impl VM {
    pub fn new(instructions: Vec<OpCode>) -> Self {
        VM { instructions, stack: Vec::new(), globals: HashMap::new(), ip: 0 }
    }

    pub fn run(&mut self){
        while self.ip < self.instructions.len(){
            let instractions = &self.instructions[self.ip];
            self.ip+=1;

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
                        _ => panic!("invalid type: {:?}",val)
                    }
                }
                OpCode::StoreGlobal(name) => {
                    let val = self.stack.pop().expect("Error: stack empty for assignment");
                    self.globals.insert(name.clone(), val);
                }
                OpCode::LoadGlobal(name) => {
                    let val = self.globals.get(name).expect(&format!("Runtime error: undefined variable {}",name));
                    self.stack.push(val.clone());
                }

            }
        }
    }
}