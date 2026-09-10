use crate::value::Value;

#[derive(Debug, Clone)]
pub enum BinaryOp {
    Add,
    Sub,
    Div,
    Mul
}

#[derive(Debug, Clone)]
pub enum Expr {
    Literal(Value),
    Variable(String),
    Binary(Box<Expr>,BinaryOp, Box<Expr>),
}

#[derive(Debug)]
pub enum Stmt {
    Print(Vec<Expr>),
    Let(String, Expr),
}