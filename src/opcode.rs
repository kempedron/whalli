use crate::value::Value;

#[derive(Debug, Clone, PartialEq)]
pub enum OpCode {
    Push(Value),
    Pop,
    Add,
    Sub,
    Div,
    Mul,
    StoreGlobal(String),
    LoadGlobal(String),
    LoadLocal(usize),
    SetLocal(usize),
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
    IndexSet,
    And,
    Or,
    Not,
    
}