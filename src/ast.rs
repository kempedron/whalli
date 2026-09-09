use crate::value::Value;

#[derive(Debug)]
pub enum BinaryOp {
    Add,
}

#[derive(Debug)]
pub enum Expr {
    Literal(Value),
    Binary(Box<Expr>,BinaryOp, Box<Expr>),
}

#[derive(Debug)]
pub enum Stmt {
    Print(Expr),
}