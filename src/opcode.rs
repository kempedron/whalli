use std::sync::Arc;

use crate::value::{FunctionObj, Value};

#[derive(Debug, Clone, PartialEq)]
pub enum UpvalueLoc {
    Local(usize),
    Upvalue(usize),
}

#[derive(Debug, Clone, PartialEq)]
pub enum SelectCaseOp {
    Recv,
    Send,
    Default,
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
    StoreGlobal(Arc<String>),
    LoadGlobal(Arc<String>),
    LoadLocal(usize),
    SetLocal(usize),
    Call(usize),
    MethodCall(Arc<String>, usize),
    Return,
    Equal,
    Less,
    Greater,
    LessEqual,
    GreaterEqual,
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
    Import {
        name: Arc<String>,
        alias: Arc<String>,
    },
    ImportFile {
        path: Arc<String>,
        alias: Arc<String>,
    },
    GetUpvalue(usize),
    SetUpvalue(usize),
    Closure(Arc<FunctionObj>, Arc<Vec<UpvalueLoc>>),
    BuildStruct(Arc<String>, Arc<Vec<String>>),
    AddMethod(Arc<String>),
    BuildInterface(Arc<Vec<String>>),
    CheckIs,
    Assert(Arc<String>),
    BuildTuple(usize),
    UnpackTuple(usize),
    Spawn(usize),
    ChanSend,
    ChanRecv,
    IterNext(usize),
    PropagateError,
    Select(Arc<Vec<SelectCaseOp>>),
    DeferCall(usize),
    DeferMethodCall(Arc<String>, usize),
    Export(Arc<String>),
    BuildModule(usize),
}
