use crate::value::Value;

#[derive(Debug, Clone)]
pub enum OpCode {
    Push(Value),
    Add,
    Print,
}