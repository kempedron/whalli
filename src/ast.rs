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
    And,
    Or,
}
#[derive(Debug, Clone)]
pub enum UnaryOp {
    Not,
}

#[derive(Debug, Clone)]
pub enum Expr {
    Literal(Value),
    Variable(String),
    Binary(Box<Expr>,BinaryOp, Box<Expr>),
    Call(Box<Expr>, Vec<Expr>),
    List(Vec<Expr>),
    Index(Box<Expr>, Box<Expr>),
    Unary(UnaryOp, Box<Expr>),
}

#[derive(Debug)]
pub enum Stmt {
    Let(String, Expr),
    Assign(String, Expr),
    IndexAssign(Expr, Expr, Expr),
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
    Break,
    Continue,
    Block(Vec<Stmt>),
}