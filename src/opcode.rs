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
    // StorageLocal(String),
    LoadLocal(usize),
    Call(usize),
    Return,
    Equal,
    Less,
    Greater,
    JumpIfFalse(usize),
    Jump(usize),
    BuildList(usize),
    IndexGet,
    ListLen,
}