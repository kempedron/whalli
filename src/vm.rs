use crate::{value::Value, opcode::OpCode};

pub struct VM{
    instructions: Vec<OpCode>,
    stack: Vec<Value>,
    ip: usize,
}

impl VM {
    pub fn new(instructions: Vec<OpCode>) -> Self {
        VM { instructions, stack: Vec::new(), ip: 0 }
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

                OpCode::Print => {
                    let val = self.stack.pop().expect("Error: stack empty");
                    match val {
                        Value::Num(n) => print!("{}",n),
                    }
                }

            }
        }
    }
}