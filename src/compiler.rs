use crate::{
    ast::{BinaryOp, Expr, Stmt, UnaryOp},
    opcode::{OpCode, UpvalueLoc},
    value::{FunctionObj, Value},
};
use std::rc::Rc;

pub struct Local {
    pub name: String,
    pub depth: usize,
}

pub struct Upvalue {
    pub index: usize,
    pub is_local: bool,
}

pub struct LoopState {
    pub break_jumps: Vec<usize>,
    pub continue_jumps: Vec<usize>,
    pub local_count: usize,
}

pub struct CompilerState {
    pub function: FunctionObj,
    pub locals: Vec<Local>,
    pub upvalues: Vec<Upvalue>,
    pub scope_depth: usize,
    pub loops: Vec<LoopState>,
}

impl CompilerState {
    pub fn new(name: String, arity: usize) -> Self {
        CompilerState {
            function: FunctionObj {
                name,
                arity,
                chunk: Vec::new(),
                param_types: Vec::new(),
            },
            locals: Vec::new(),
            upvalues: Vec::new(),
            scope_depth: 0,
            loops: Vec::new(),
        }
    }
}

pub struct Compiler {
    states: Vec<CompilerState>,
}

impl Compiler {
    pub fn new() -> Self {
        Compiler {
            states: vec![CompilerState::new("main".to_string(), 0)],
        }
    }

    fn current_state(&mut self) -> &mut CompilerState {
        self.states.last_mut().unwrap()
    }

    fn emit(&mut self, opcode: OpCode) {
        self.current_state().function.chunk.push(opcode);
    }

    fn current_ip(&mut self) -> usize {
        self.current_state().function.chunk.len()
    }

    fn emit_jump(&mut self, opcode: OpCode) -> usize {
        self.emit(opcode);
        self.current_ip() - 1
    }

    fn patch_jump(&mut self, offset: usize, target: usize) {
        let chunk = &mut self.current_state().function.chunk;
        match chunk[offset] {
            OpCode::JumpIfFalse(ref mut target_ip) => *target_ip = target,
            OpCode::Jump(ref mut target_ip) => *target_ip = target,
            _ => unreachable!("Compiler error: trying to patch non-jump instruction"),
        }
    }

    fn begin_scope(&mut self) {
        self.current_state().scope_depth += 1;
    }

    fn end_scope(&mut self) {
        let state = self.current_state();
        state.scope_depth -= 1;

        let mut pops = 0;
        while let Some(local) = state.locals.last() {
            if local.depth > state.scope_depth {
                state.locals.pop();
                pops += 1;
            } else {
                break;
            }
        }

        for _ in 0..pops {
            self.emit(OpCode::Pop);
        }
    }

    fn resolve_local(&self, state_idx: usize, name: &str) -> Option<usize> {
        let state = &self.states[state_idx];
        for (i, local) in state.locals.iter().enumerate().rev() {
            if local.name == name {
                return Some(i);
            }
        }
        None
    }

    fn resolve_upvalue(&mut self, state_idx: usize, name: &str) -> Option<usize> {
        if state_idx == 0 {
            return None;
        }

        let parent_idx = state_idx - 1;

        if let Some(local_idx) = self.resolve_local(parent_idx, name) {
            return Some(self.add_upvalue(state_idx, local_idx, true));
        }

        if let Some(upvalue_idx) = self.resolve_upvalue(parent_idx, name) {
            return Some(self.add_upvalue(state_idx, upvalue_idx, false));
        }

        None
    }

    fn add_upvalue(&mut self, state_idx: usize, index: usize, is_local: bool) -> usize {
        let state = &mut self.states[state_idx];

        for (i, upvalue) in state.upvalues.iter().enumerate() {
            if upvalue.index == index && upvalue.is_local == is_local {
                return i;
            }
        }
        state.upvalues.push(Upvalue { index, is_local });
        state.upvalues.len() - 1
    }

    pub fn compile(mut self, stmts: &[Stmt]) -> Vec<OpCode> {
        for stmt in stmts {
            self.compile_stmt(stmt);
        }
        self.states.pop().unwrap().function.chunk
    }

    fn compile_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Let(name, expr) => {
                self.compile_expr(expr);

                let state = self.current_state();
                if state.scope_depth > 0 {
                    let depth = state.scope_depth;
                    state.locals.push(Local {
                        name: name.clone(),
                        depth,
                    });
                } else {
                    self.emit(OpCode::StoreGlobal(name.clone()));
                }
            }

            Stmt::Functions(name, params, body) => {
                let arity = params.len();
                let mut new_state = CompilerState::new(name.clone(), arity);
                new_state.scope_depth = 1;

                for param in params {
                    new_state.locals.push(Local {
                        name: param.clone(),
                        depth: 1,
                    });
                }

                self.states.push(new_state);

                for s in body {
                    self.compile_stmt(s);
                }

                self.emit(OpCode::Push(Value::Nil));
                self.emit(OpCode::Return);

                let state = self.states.pop().unwrap();

                let mut upvalue_locs = Vec::new();
                for upvalue in state.upvalues {
                    if upvalue.is_local {
                        upvalue_locs.push(UpvalueLoc::Local(upvalue.index));
                    } else {
                        upvalue_locs.push(UpvalueLoc::Upvalue(upvalue.index));
                    }
                }

                let func_value = Rc::new(state.function);

                self.emit(OpCode::Closure(func_value, upvalue_locs));
                self.emit(OpCode::StoreGlobal(name.clone()));
            }

            Stmt::Return(expr) => {
                self.compile_expr(expr);
                self.emit(OpCode::Return);
            }
            Stmt::Expr(expr) => {
                self.compile_expr(expr);
                self.emit(OpCode::Pop);
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.compile_expr(condition);

                let jump_if_false_pos = self.emit_jump(OpCode::JumpIfFalse(999));

                for s in then_branch {
                    self.compile_stmt(s);
                }

                if let Some(else_stmts) = else_branch {
                    let jump_over_else_pos = self.emit_jump(OpCode::Jump(999));

                    let else_start = self.current_ip();
                    self.patch_jump(jump_if_false_pos, else_start);

                    for s in else_stmts {
                        self.compile_stmt(s);
                    }

                    let end_pos = self.current_ip();
                    self.patch_jump(jump_over_else_pos, end_pos);
                } else {
                    let end_pos = self.current_ip();
                    self.patch_jump(jump_if_false_pos, end_pos);
                }
            }
            Stmt::While { condition, body } => {
                let loop_start = self.current_ip();
                self.compile_expr(condition);
                let exit_jump_pos = self.emit_jump(OpCode::JumpIfFalse(999));

                let local_count = self.current_state().locals.len();
                self.current_state().loops.push(LoopState {
                    break_jumps: vec![],
                    continue_jumps: vec![],
                    local_count,
                });

                self.begin_scope();
                for s in body {
                    self.compile_stmt(s);
                }
                self.end_scope();

                let continue_target = loop_start;
                self.emit(OpCode::Jump(loop_start));

                let end_pos = self.current_ip();
                self.patch_jump(exit_jump_pos, end_pos);

                let loop_state = self.current_state().loops.pop().unwrap();
                for pos in loop_state.break_jumps {
                    self.patch_jump(pos, end_pos);
                }
                for pos in loop_state.continue_jumps {
                    self.patch_jump(pos, continue_target);
                }
            }
            Stmt::For {
                item,
                iterable,
                body,
            } => {
                let loop_id = self.current_ip();
                let arr_var = format!("__iter_arr{}", loop_id);
                let idx_var = format!("__iter_idx{}", loop_id);

                self.compile_expr(iterable);
                self.emit(OpCode::StoreGlobal(arr_var.clone()));

                self.emit(OpCode::Push(Value::Int(0)));
                self.emit(OpCode::StoreGlobal(idx_var.clone()));

                let loop_start = self.current_ip();

                self.emit(OpCode::LoadGlobal(idx_var.clone()));
                self.emit(OpCode::LoadGlobal(arr_var.clone()));
                self.emit(OpCode::ListLen);
                self.emit(OpCode::Less);

                let exit_pos = self.emit_jump(OpCode::JumpIfFalse(999));

                self.emit(OpCode::LoadGlobal(arr_var.clone()));
                self.emit(OpCode::LoadGlobal(idx_var.clone()));
                self.emit(OpCode::IndexGet);
                self.emit(OpCode::StoreGlobal(item.clone()));

                let local_count = self.current_state().locals.len();
                self.current_state().loops.push(LoopState {
                    break_jumps: vec![],
                    continue_jumps: vec![],
                    local_count,
                });

                self.begin_scope();

                self.emit(OpCode::LoadGlobal(arr_var.clone()));
                self.emit(OpCode::LoadGlobal(idx_var.clone()));
                self.emit(OpCode::IndexGet);

                let depth = self.current_state().scope_depth;
                self.current_state().locals.push(Local {
                    name: item.clone(),
                    depth,
                });

                for s in body {
                    self.compile_stmt(s);
                }
                self.end_scope();

                let continue_target = self.current_ip();

                self.emit(OpCode::LoadGlobal(idx_var.clone()));
                self.emit(OpCode::Push(Value::Int(1)));
                self.emit(OpCode::Add);
                self.emit(OpCode::StoreGlobal(idx_var.clone()));

                self.emit(OpCode::Jump(loop_start));

                let end_pos = self.current_ip();
                self.patch_jump(exit_pos, end_pos);

                let loop_state = self.current_state().loops.pop().unwrap();
                for pos in loop_state.break_jumps {
                    self.patch_jump(pos, end_pos);
                }
                for pos in loop_state.continue_jumps {
                    self.patch_jump(pos, continue_target);
                }
            }
            Stmt::Assign(name, expr) => {
                self.compile_expr(expr);

                let current_idx = self.states.len() - 1;
                if let Some(idx) = self.resolve_local(current_idx, name) {
                    self.emit(OpCode::SetLocal(idx));
                } else if let Some(upvalue_idx) = self.resolve_upvalue(current_idx, name) {
                    self.emit(OpCode::SetUpvalue(upvalue_idx));
                } else {
                    self.emit(OpCode::StoreGlobal(name.clone()));
                }
            }
            Stmt::IndexAssign(array, index, value) => {
                self.compile_expr(array);
                self.compile_expr(index);
                self.compile_expr(value);
                self.emit(OpCode::IndexSet);
            }
            Stmt::Break => {
                let local_count = {
                    let state = self.current_state();
                    if state.loops.is_empty() {
                        panic!("Compiler error: 'break' outside of loop");
                    }
                    state.loops.last().unwrap().local_count
                };

                let pops = self.current_state().locals.len() - local_count;
                for _ in 0..pops {
                    self.emit(OpCode::Pop);
                }

                let jump_pos = self.emit_jump(OpCode::Jump(999));
                self.current_state()
                    .loops
                    .last_mut()
                    .unwrap()
                    .break_jumps
                    .push(jump_pos);
            }
            Stmt::Continue => {
                let local_count = {
                    let state = self.current_state();
                    if state.loops.is_empty() {
                        panic!("Compiler error: 'continue' outside of loop");
                    }
                    state.loops.last().unwrap().local_count
                };

                let pops = self.current_state().locals.len() - local_count;
                for _ in 0..pops {
                    self.emit(OpCode::Pop);
                }

                let jump_pos = self.emit_jump(OpCode::Jump(999));
                self.current_state()
                    .loops
                    .last_mut()
                    .unwrap()
                    .continue_jumps
                    .push(jump_pos);
            }
            Stmt::Block(stmts) => {
                self.begin_scope();
                for s in stmts {
                    self.compile_stmt(s);
                }
                self.end_scope();
            }
            Stmt::Line(line) => {
                self.emit(OpCode::SetLine(*line));
            }
            Stmt::Import(name) => {
                self.emit(OpCode::Import(name.clone()));
            }
            Stmt::Struct(name, fields) => {
                let field_names = fields.iter().map(|(f, _)| f.clone()).collect();

                self.emit(OpCode::BuildStruct(name.clone(), field_names));
                self.emit(OpCode::StoreGlobal(name.clone()));
            }

            Stmt::Impl(target_name, methods) => {
                self.emit(OpCode::LoadGlobal(target_name.clone()));
                
                for method in methods {
                    if let Stmt::Functions(name, params, body) = method {
                        let arity = params.len();
                        let mut new_state = CompilerState::new(name.clone(), arity);
                        new_state.scope_depth = 1;

                        for param in params {
                            new_state.locals.push(Local { name: param.clone(), depth: 1 });
                        }

                        self.states.push(new_state);
                        for s in body { self.compile_stmt(s); }
                        self.emit(OpCode::Push(Value::Nil));
                        self.emit(OpCode::Return);
                    
                        let state = self.states.pop().unwrap();
                        let mut upvalue_locs = Vec::new();
                        
                        for upvalue in state.upvalues {
                            if upvalue.is_local { upvalue_locs.push(UpvalueLoc::Local(upvalue.index)); }
                            else { upvalue_locs.push(UpvalueLoc::Upvalue(upvalue.index)); }
                        }

                        let func_value = Rc::new(state.function);
                        self.emit(OpCode::Closure(func_value, upvalue_locs));
                        
                        self.emit(OpCode::AddMethod(name.clone()));
                    }
                }
                self.emit(OpCode::Pop);
            }
            // Stmt::Interface(, )
        Stmt::Interface(name, methods) => {
                // Создаем интерфейс в памяти и кладем в глобальную переменную (как структуру)
                self.emit(OpCode::BuildInterface(methods.clone()));
                self.emit(OpCode::StoreGlobal(name.clone()));
            }
        }
    }

    fn compile_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Literal(val) => {
                self.emit(OpCode::Push(val.clone()));
            }
            Expr::Binary(left, op, right) => {
                self.compile_expr(left);
                self.compile_expr(right);

                match op {
                    BinaryOp::Add => self.emit(OpCode::Add),
                    BinaryOp::Sub => self.emit(OpCode::Sub),
                    BinaryOp::Div => self.emit(OpCode::Div),
                    BinaryOp::Mul => self.emit(OpCode::Mul),
                    BinaryOp::Mod => self.emit(OpCode::Mod),
                    BinaryOp::Equal => self.emit(OpCode::Equal),
                    BinaryOp::Greater => self.emit(OpCode::Greater),
                    BinaryOp::Less => self.emit(OpCode::Less),
                    BinaryOp::And => self.emit(OpCode::And),
                    BinaryOp::Or => self.emit(OpCode::Or),
                }
            }
            Expr::Variable(name) => {
                let current_idx = self.states.len() - 1;
                if let Some(idx) = self.resolve_local(current_idx, name) {
                    self.emit(OpCode::LoadLocal(idx));
                } else if let Some(upvalue_idx) = self.resolve_upvalue(current_idx, name) {
                    self.emit(OpCode::GetUpvalue(upvalue_idx));
                } else {
                    self.emit(OpCode::LoadGlobal(name.clone()));
                }
            }
            Expr::Call(callee, args) => {
                self.compile_expr(callee);
                for arg in args {
                    self.compile_expr(arg);
                }
                self.emit(OpCode::Call(args.len()));
            }
            Expr::MethodCall(obj, method_name, args) => {
                self.compile_expr(obj);
                for arg in args {
                    self.compile_expr(arg);
                }
                self.emit(OpCode::MethodCall(method_name.clone(), args.len()));
            }
            Expr::List(elements) => {
                let len = elements.len();
                for el in elements {
                    self.compile_expr(el);
                }
                self.emit(OpCode::BuildList(len));
            }
            Expr::Index(array, index) => {
                self.compile_expr(array);
                self.compile_expr(index);
                self.emit(OpCode::IndexGet);
            }
            Expr::Unary(op, right) => {
                self.compile_expr(right);
                match op {
                    UnaryOp::Not => self.emit(OpCode::Not),
                }
            }
            Expr::Map(entries) => {
                let len = entries.len();
                for (k, v) in entries {
                    self.compile_expr(k);
                    self.compile_expr(v);
                }
                self.emit(OpCode::BuildMap(len));
            }
            Expr::Is(left, right) => {
                self.compile_expr(left);
                self.compile_expr(right);
                self.emit(OpCode::CheckIs);
            }
        }
    }
}
