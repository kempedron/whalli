use std::rc::Rc;

use crate::{ast::{BinaryOp, Expr, Stmt}, opcode::OpCode, value::{FunctionObj, Value}};

pub struct Compiler {
    bytecode: Vec<OpCode>,
    locals: Vec<String>,
}

impl Compiler {
    pub fn new() -> Self{
        Compiler {
            bytecode: Vec::new(),
            locals: Vec::new(),
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
            Stmt::Functions(name,params, body ) => {
                let mut fn_compiler = Compiler::new();
                
                for param in params {
                    fn_compiler.locals.push(param.clone());
                }

                for stmt in body {
                    fn_compiler.compile_stmt(stmt);
                }
                fn_compiler.bytecode.push(OpCode::Push(Value::Nil));
                fn_compiler.bytecode.push(OpCode::Return);

                let func_obj = FunctionObj {
                    name: name.clone(),
                    arity: params.len(),
                    chunk: fn_compiler.bytecode,
                    param_types: params.clone(),
                };

                let func_value = Value::Function(Rc::new(func_obj));

                self.bytecode.push(OpCode::Push(func_value));
                self.bytecode.push(OpCode::StoreGlobal(name.clone()));

            }
            Stmt::Return(expr) => {
                self.compile_expr(expr);
                self.bytecode.push(OpCode::Return);
            }
            Stmt::Expr(expr) => {
                self.compile_expr(expr);
            }
            Stmt::If { condition, then_branch, else_branch } => {
                self.compile_expr(condition);

                let jump_if_false_pos = self.bytecode.len();
                self.bytecode.push(OpCode::JumpIfFalse(999));

                for stmt in then_branch {
                    self.compile_stmt(stmt);
                }
                if let Some(else_stmts) = else_branch {
                    let jump_over_else_pos = self.bytecode.len();
                    self.bytecode.push(OpCode::Jump(999));
                
                    let else_start = self.bytecode.len();
                    self.bytecode[jump_if_false_pos] = OpCode::JumpIfFalse(else_start);

                    for stmt in else_stmts {
                        self.compile_stmt(stmt);
                    }

                    let end_pos = self.bytecode.len();
                    self.bytecode[jump_over_else_pos] = OpCode::Jump(end_pos);
                } else {
                    let end_pos = self.bytecode.len();
                    self.bytecode[jump_if_false_pos] = OpCode::JumpIfFalse(end_pos);
                }

            }
            Stmt::While { condition, body } => {
                let loop_start = self.bytecode.len();
                self.compile_expr(condition);
                let exit_jump_pos = self.bytecode.len();
                self.bytecode.push(OpCode::JumpIfFalse(999));

                for stmt in body {
                    self.compile_stmt(stmt);
                }

                self.bytecode.push(OpCode::Jump(loop_start));
                
                let end_pos = self.bytecode.len();
                self.bytecode[exit_jump_pos] = OpCode::JumpIfFalse(end_pos);
            }
            Stmt::For { item, iterable, body } => {
                let loop_id = self.bytecode.len();
                let arr_var = format!("__iter_arr{}",loop_id);
                let idx_var = format!("__iter_idx{}",loop_id);

                self.compile_expr(iterable);
                self.bytecode.push(OpCode::StoreGlobal(arr_var.clone()));
                
                self.bytecode.push(OpCode::Push(Value::Int(0)));
                self.bytecode.push(OpCode::StoreGlobal(idx_var.clone()));

                let loop_start = self.bytecode.len();

                self.bytecode.push(OpCode::LoadGlobal(idx_var.clone()));
                self.bytecode.push(OpCode::LoadGlobal(arr_var.clone()));
                self.bytecode.push(OpCode::ListLen);
                self.bytecode.push(OpCode::Less);

                let exit_pos = self.bytecode.len();
                self.bytecode.push(OpCode::JumpIfFalse(999));

                self.bytecode.push(OpCode::LoadGlobal(arr_var.clone()));
                self.bytecode.push(OpCode::LoadGlobal(idx_var.clone()));
                self.bytecode.push(OpCode::IndexGet);
                self.bytecode.push(OpCode::StoreGlobal(item.clone()));


                for stmt in body {
                    self.compile_stmt(stmt);
                }

                self.bytecode.push(OpCode::LoadGlobal(idx_var.clone()));
                self.bytecode.push(OpCode::Push(Value::Int(1)));
                self.bytecode.push(OpCode::Add);
                self.bytecode.push(OpCode::StoreGlobal(idx_var.clone()));

                self.bytecode.push(OpCode::Jump(loop_start));
                
                let end_pos = self.bytecode.len();
                self.bytecode[exit_pos] = OpCode::JumpIfFalse(end_pos);

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
                    BinaryOp::Equal => self.bytecode.push(OpCode::Equal),
                    BinaryOp::Greater => self.bytecode.push(OpCode::Greater),
                    BinaryOp::Less => self.bytecode.push(OpCode::Less),
                }
            }
            Expr::Variable(name) => {
                if let Some(idx) = self.locals.iter().position(|l| l == name){
                    self.bytecode.push(OpCode::LoadLocal(idx));
                } else {
                    self.bytecode.push(OpCode::LoadGlobal(name.clone()));                    
                }
            }
            Expr::Call(callee, args) => {
                self.compile_expr(callee);
                for arg in args {
                    self.compile_expr(arg);
                }
                self.bytecode.push(OpCode::Call(args.len()));
            }
            Expr::List(elements) => {
                let len = elements.len();
                for el in elements {
                    self.compile_expr(el);
                }
                self.bytecode.push(OpCode::BuildList(len));
            }
            Expr::Index(array, index) => {
                self.compile_expr(array);
                self.compile_expr(index);
                self.bytecode.push(OpCode::IndexGet);
            }
         }
    }
}