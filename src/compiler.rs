use std::rc::Rc;

use crate::{ast::{BinaryOp, Expr, Stmt, UnaryOp}, opcode::OpCode, value::{FunctionObj, Value}};

pub struct Local {
    pub name: String,
    pub depth: usize,
}

pub struct LoopState {
    pub break_jumps: Vec<usize>,
    pub continue_jumps: Vec<usize>,
    pub local_count: usize,
}
pub struct Compiler {
    bytecode: Vec<OpCode>,
    // lines: Vec<usize>,
    locals: Vec<Local>,
    scope_depth: usize,
    loops: Vec<LoopState>,
}

impl Compiler {
    pub fn new() -> Self{
        Compiler {
            bytecode: Vec::new(),
            locals: Vec::new(),
            scope_depth: 0,
            loops: Vec::new(),
        }
    }

    fn begin_scope(&mut self) {
        self.scope_depth += 1;
    }

    fn end_scope(&mut self) {
        self.scope_depth -= 1;

        while let Some(local) = self.locals.last() {
            if local.depth > self.scope_depth {
                self.bytecode.push(OpCode::Pop);
                self.locals.pop();
            } else {
                break;
            }
        }
    }

    fn resolve_local(&self, name: &str) -> Option<usize>{
        for (i, local) in self.locals.iter().enumerate().rev() {
            if local.name == name {
                return Some(i)
            }
        }
        None
    }

    pub fn compile(mut self, stmts: &[Stmt]) -> Vec<OpCode>{
        for stmt in stmts {
            self.compile_stmt(stmt)
        }
        self.bytecode
    }

    fn compile_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Let(name,expr) => {
                self.compile_expr(expr);
                if self.scope_depth > 0 {
                    self.locals.push(Local { name: name.clone(), depth: self.scope_depth });
                } else {
                    self.bytecode.push(OpCode::StoreGlobal(name.clone()));
                }
            }
            Stmt::Functions(name,params, body ) => {
                let mut fn_compiler = Compiler::new();

                fn_compiler.scope_depth = 1;
                
                for param in params {
                    fn_compiler.locals.push(Local { name: param.clone(), depth: 1 });
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
                self.bytecode.push(OpCode::Pop);
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

                self.loops.push(LoopState { break_jumps: vec![], continue_jumps: vec![], local_count: self.locals.len() });

                self.begin_scope();
                for stmt in body {
                    self.compile_stmt(stmt);
                }
                self.end_scope();

                let continue_target = self.bytecode.len();
                self.bytecode.push(OpCode::Jump(loop_start));
                
                let end_pos = self.bytecode.len();
                self.bytecode[exit_jump_pos] = OpCode::JumpIfFalse(end_pos);
            
                let loop_state = self.loops.pop().unwrap();
                for pos in loop_state.break_jumps { self.bytecode[pos] = OpCode::Jump(end_pos); }
                for pos in loop_state.continue_jumps { self.bytecode[pos] = OpCode::Jump(continue_target); }
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

                self.loops.push(LoopState { break_jumps: vec![], continue_jumps: vec![], local_count: self.locals.len() });

                self.begin_scope();

                self.bytecode.push(OpCode::LoadGlobal(arr_var.clone()));
                self.bytecode.push(OpCode::LoadGlobal(idx_var.clone()));
                self.bytecode.push(OpCode::IndexGet);
                self.locals.push(Local { name: item.clone(), depth: self.scope_depth });

                for stmt in body {
                    self.compile_stmt(stmt);
                }
                self.end_scope();

                let continue_target = self.bytecode.len();

                self.bytecode.push(OpCode::LoadGlobal(idx_var.clone()));
                self.bytecode.push(OpCode::Push(Value::Int(1)));
                self.bytecode.push(OpCode::Add);
                self.bytecode.push(OpCode::StoreGlobal(idx_var.clone()));

                self.bytecode.push(OpCode::Jump(loop_start));
                
                let end_pos = self.bytecode.len();
                self.bytecode[exit_pos] = OpCode::JumpIfFalse(end_pos);

                let loop_state = self.loops.pop().unwrap();
                for pos in loop_state.break_jumps { self.bytecode[pos] = OpCode::Jump(end_pos); }
                for pos in loop_state.continue_jumps { self.bytecode[pos] = OpCode::Jump(continue_target); }
                
            }
            Stmt::Assign(name, expr) => {
                self.compile_expr(expr);
                if let Some(idx) = self.resolve_local(name) {
                    self.bytecode.push(OpCode::SetLocal(idx));
                } else {
                    self.bytecode.push(OpCode::StoreGlobal(name.clone()));
                }
            }
            Stmt::IndexAssign(array, index, value ) => {
                self.compile_expr(array);
                self.compile_expr(index);
                self.compile_expr(value);
                self.bytecode.push(OpCode::IndexSet);
            }
            Stmt::Break => {
                if let Some(loop_state) = self.loops.last_mut(){
                    let pops = self.locals.len() - loop_state.local_count;
                    for _ in 0..pops { self.bytecode.push(OpCode::Pop); }
                    let jump_pos = self.bytecode.len();
                    self.bytecode.push(OpCode::Jump(999));
                    loop_state.break_jumps.push(jump_pos);
                } else {
                    panic!("Compiler error: 'break' outside of loop");
                }
            }
            Stmt::Continue => {
                if let Some(loop_state) = self.loops.last_mut(){
                    let pops = self.locals.len() - loop_state.local_count;
                    for _ in 0..pops { self.bytecode.push(OpCode::Pop); }
                    let jump_pos = self.bytecode.len();
                    self.bytecode.push(OpCode::Jump(999));
                    loop_state.continue_jumps.push(jump_pos);
                } else {
                    panic!("Compiler error: 'continue' outside of loop");
                }
            }
            Stmt::Block(stmts) => {
                self.begin_scope();
                for stmt in stmts {
                    self.compile_stmt(stmt);
                }
                self.end_scope();
            }
            Stmt::Line(line) => {
                self.bytecode.push(OpCode::SetLine(*line));
            }
            Stmt::Import(name) => {
                self.bytecode.push(OpCode::Import(name.clone()));
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
                    BinaryOp::Mod => self.bytecode.push(OpCode::Mod),
                    BinaryOp::Equal => self.bytecode.push(OpCode::Equal),
                    BinaryOp::Greater => self.bytecode.push(OpCode::Greater),
                    BinaryOp::Less => self.bytecode.push(OpCode::Less),
                    BinaryOp::And => self.bytecode.push(OpCode::And),
                    BinaryOp::Or => self.bytecode.push(OpCode::Or),
                }
            }
            Expr::Variable(name) => {
                if let Some(idx) = self.resolve_local(name) {
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
            Expr::MethodCall(obj, method_name, args) => {
                self.compile_expr(obj);
                for arg in args {
                    self.compile_expr(arg);
                }
                self.bytecode.push(OpCode::MethodCall(method_name.clone(), args.len()));
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
            Expr::Unary(op, right) => {
                self.compile_expr(right);
                match op {
                    UnaryOp::Not => self.bytecode.push(OpCode::Not),
                }
            }
            Expr::Map(entries) => {
                let len = entries.len();
                for (k,v) in entries {
                    self.compile_expr(k);
                    self.compile_expr(v);
                }
                self.bytecode.push(OpCode::BuildMap(len));
            }
         }
    }
}