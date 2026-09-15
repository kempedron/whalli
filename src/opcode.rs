use std::rc::Rc;

use crate::value::{FunctionObj, Value};

#[derive(Debug, Clone, PartialEq)]
pub enum UpvalueLoc {
    Local(usize),
    Upvalue(usize),
}

#[derive(Debug, Clone, PartialEq)]
pub enum OpCode {
    Push(Value),
    Pop,
    Add,
    Sub,
    Div,
    Mul,
    Mod,
    StoreGlobal(String),
    LoadGlobal(String),
    LoadLocal(usize),
    SetLocal(usize),
    Call(usize),
    MethodCall(String, usize),
    Return,
    Equal,
    Less,
    Greater,
    JumpIfFalse(usize),
    Jump(usize),
    BuildList(usize),
    BuildMap(usize),
    IndexGet,
    ListLen,
    IndexSet,
    And,
    Or,
    Not,
    SetLine(usize),
    Import(String),
    GetUpvalue(usize),
    SetUpvalue(usize),
    Closure(Rc<FunctionObj>, Vec<UpvalueLoc>),
}