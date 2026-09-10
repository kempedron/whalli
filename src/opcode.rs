use crate::value::Value;

#[derive(Debug, Clone, PartialEq)]
pub enum OpCode {
    Push(Value),
    Add,
    Sub,
    Div,
    Mul,
    Print,
    StoreGlobal(String),
    LoadGlobal(String),
}