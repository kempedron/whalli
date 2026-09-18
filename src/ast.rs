use crate::value::Value;

#[derive(Debug, Clone)]
pub enum BinaryOp {
    Add,
    Sub,
    Div,
    Mul,
    Mod,
    Equal,
    Less,
    Greater,
    LessEqual,
    GreaterEqual,
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
    Binary(Box<Expr>, BinaryOp, Box<Expr>),
    Call(Box<Expr>, Vec<Expr>),
    List(Vec<Expr>),
    Map(Vec<(Expr, Expr)>),
    Index(Box<Expr>, Box<Expr>),
    Unary(UnaryOp, Box<Expr>),
    MethodCall(Box<Expr>, String, Vec<Expr>),
    Is(Box<Expr>, Box<Expr>), // interface implementation check
    Tuple(Vec<Expr>),
    ChanSend(Box<Expr>, Box<Expr>), // ch <- val
    ChanRecv(Box<Expr>), // <- ch
}

#[derive(Debug)]
pub enum Stmt {
    Let(String, Expr),
    LetTuple(Vec<String>, Expr),
    Assign(String, Expr),
    IndexAssign(Expr, Expr, Expr),
    Functions(
        String,
        Vec<(String, Option<String>)>,
        Option<String>,
        Vec<Stmt>,
    ),
    Return(Expr),
    Expr(Expr),
    If {
        condition: Expr,
        then_branch: Vec<Stmt>,
        else_branch: Option<Vec<Stmt>>,
    },
    While {
        condition: Expr,
        body: Vec<Stmt>,
    },
    For {
        item: String,
        iterable: Expr,
        body: Vec<Stmt>,
    },
    Break,
    Continue,
    Block(Vec<Stmt>),
    Import(String),
    Struct(String, Vec<(String, String)>),
    Impl(String, Vec<Stmt>),
    Line(usize),
    Interface(String, Vec<String>),
    Spawn(Box<Expr>, Vec<Expr>),
}
