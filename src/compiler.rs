use crate::{ast::{Expr,Stmt,BinaryOp},opcode::OpCode};

pub struct Compiler {
    bytecode: Vec<OpCode>,
}

impl Compiler {
    pub fn new() -> Self{
        Compiler {
            bytecode: Vec::new(),
        }
    }

    pub fn compile(mut self, stmts: &[Stmt]) -> Vec<OpCode>{
        for stmt in stmts {
            self.compile_stmt(stmt)
        }
        self.bytecode
    }

    fn compile_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Print(exprs) => {
                for expr in exprs {
                    self.compile_expr(expr);
                    self.bytecode.push(OpCode::Print);
                }
            }
            Stmt::Let(name,expr) => {
                self.compile_expr(expr);
                self.bytecode.push(OpCode::StoreGlobal(name.clone()));
            }
        }
    }

    fn compile_expr(&mut self,expr: &Expr) {
        match expr {
            Expr::Literal(val) => {
                self.bytecode.push(OpCode::Push(val.clone()));
            }
            Expr::Binary(left,op ,right) => {
                self.compile_expr(left);
                self.compile_expr(right);

                match op {
                    BinaryOp::Add => self.bytecode.push(OpCode::Add),
                    BinaryOp::Sub => self.bytecode.push(OpCode::Sub),
                    BinaryOp::Div => self.bytecode.push(OpCode::Div),
                    BinaryOp::Mul => self.bytecode.push(OpCode::Mul),

                }
            }
            Expr::Variable(name) => {
                self.bytecode.push(OpCode::LoadGlobal(name.clone()));
            }
         }
    }
}