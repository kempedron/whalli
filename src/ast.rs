use crate::value::Value;

#[derive(Debug, Clone)]
pub enum BinaryOp {
    Add,
    Sub,
    Div,
    Mul,
    Mod,
    Equal,
    NotEqual,
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
pub enum Pattern {
    Wildcard,
    Literal(Value),
    Range { start: i64, end: i64, inclusive: bool },
    Variable(String),
    Type(String, String), // name: type (e.g. n: int)
    Tuple(Vec<Pattern>),
    Or(Vec<Pattern>),
}

#[derive(Debug, Clone)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub guard: Option<Expr>,
    pub body: Expr,
}

#[derive(Debug, Clone)]
pub enum SelectArmKind {
    Recv(String, Expr), // var <- chan
    Send(Expr, Expr),   // chan <- val
    Default,
}

#[derive(Debug, Clone)]
pub struct SelectArm {
    pub kind: SelectArmKind,
    pub body: Expr,
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
    ChanRecv(Box<Expr>),            // <- ch
    Try(Box<Expr>),                 // expr?
    Match {
        subject: Box<Expr>,
        arms: Vec<MatchArm>,
    },
    Select(Vec<SelectArm>),
    Spawn(Box<Expr>, Vec<Expr>),
}

#[derive(Debug)]
pub enum Stmt {
    Let(String, Expr, bool), // name, expr, is_pub
    LetTuple(Vec<String>, Expr),
    Assign(String, Expr),
    IndexAssign(Expr, Expr, Expr),
    Functions(
        String,
        Vec<(String, Option<String>)>,
        Option<String>,
        Vec<Stmt>,
        bool, // is_pub
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
    Import {
        name: String,
        alias: Option<String>,
    },
    ImportFile {
        path: String,
        alias: Option<String>,
    },
    Struct(String, Vec<(String, String)>, bool), // name, fields, is_pub
    Impl(String, Vec<Stmt>),
    Line(usize),
    Interface(String, Vec<String>, bool), // name, methods, is_pub
    Spawn(Box<Expr>, Vec<Expr>),
    Defer(Box<Expr>),
}
