use crate::value::Value;

#[derive(Debug, Clone)]
pub enum BinaryOp {
    Add,
    Sub,
    Div,
    Mul,
    Equal,
    Less,
    Greater,
}

#[derive(Debug, Clone)]
pub enum Expr {
    Literal(Value),
    Variable(String),
    Binary(Box<Expr>,BinaryOp, Box<Expr>),
    Call(Box<Expr>, Vec<Expr>),
    List(Vec<Expr>),
    Index(Box<Expr>, Box<Expr>),
}

#[derive(Debug)]
pub enum Stmt {
    Print(Vec<Expr>),
    Let(String, Expr),
    Functions(String, Vec<String>, Vec<Stmt>),
    Return(Expr),
    Expr(Expr),
    If {
        condition: Expr,
        then_branch: Vec<Stmt>,
        else_branch: Option<Vec<Stmt>>,
    },
    While { condition: Expr, body: Vec<Stmt> },
    For {item: String, iterable: Expr, body: Vec<Stmt>},
}