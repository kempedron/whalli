use crate::{
    ast::{BinaryOp, Expr, Stmt, UnaryOp},
    opcode::{OpCode, UpvalueLoc},
    value::{FunctionObj, Value},
};
use std::sync::Arc;

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
    pub fn new(name: String, arity: usize, return_type: Option<String>) -> Self {
        CompilerState {
            function: FunctionObj {
                name,
                arity,
                chunk: Vec::new(),
                param_types: Vec::new(),
                return_type,
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
            states: vec![CompilerState::new("main".to_string(), 0, None)],
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

            Stmt::Functions(name, params, return_type, body) => {
                let arity = params.len();
                let mut new_state = CompilerState::new(name.clone(), arity, return_type.clone());
                new_state.scope_depth = 1;

                for (param_name, _param_type) in params {
                    new_state.locals.push(Local {
                        name: param_name.clone(),
                        depth: 1,
                    });
                }

                self.states.push(new_state);

                for (i, (param_name, param_type)) in params.iter().enumerate() {
                    if let Some(t) = param_type {
                        self.emit(OpCode::LoadLocal(i));
                        self.emit(OpCode::LoadGlobal(t.clone()));
                        self.emit(OpCode::CheckIs);
                        self.emit(OpCode::Assert(format!(
                            "TypeError: param {} must implement {}",
                            param_name, t
                        )));
                    }
                }

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

                        let func_value = Arc::new(state.function);

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
                self.begin_scope();

                self.compile_expr(iterable);
                
                let current_depth = self.current_state().scope_depth;
                
                self.current_state().locals.push(Local { 
                    name: "<iterable>".to_string(), 
                    depth: current_depth 
                });

                self.emit(OpCode::Push(Value::Int(0)));
                
                let iter_state_idx = self.current_state().locals.len();
                self.current_state().locals.push(Local { 
                    name: "<iter_state>".to_string(), 
                    depth: current_depth 
                });

                let loop_start = self.current_ip();

                self.emit(OpCode::IterNext(iter_state_idx));
                
                let exit_pos = self.emit_jump(OpCode::JumpIfFalse(999));

                self.begin_scope();
                
                let inner_depth = self.current_state().scope_depth;
                self.current_state().locals.push(Local { name: item.clone(), depth: inner_depth });

                let local_count = self.current_state().locals.len();
                self.current_state().loops.push(LoopState { break_jumps: vec![], continue_jumps: vec![], local_count });

                for s in body {
                    self.compile_stmt(s);
                }

                self.end_scope();

                let continue_target = loop_start;
                self.emit(OpCode::Jump(loop_start)); 

                let end_pos = self.current_ip();
                self.patch_jump(exit_pos, end_pos);
                
                self.emit(OpCode::Pop);

                let loop_state = self.current_state().loops.pop().unwrap();
                for pos in loop_state.break_jumps { self.patch_jump(pos, end_pos); }
                for pos in loop_state.continue_jumps { self.patch_jump(pos, continue_target); }

                self.end_scope();
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
            Stmt::ImportFile(path) => {
                self.emit(OpCode::ImportFile(path.clone()));
            }
            Stmt::Struct(name, fields) => {
                let field_names = fields.iter().map(|(f, _)| f.clone()).collect();

                self.emit(OpCode::BuildStruct(name.clone(), field_names));
                self.emit(OpCode::StoreGlobal(name.clone()));
            }

            Stmt::Impl(target_name, methods) => {
                self.emit(OpCode::LoadGlobal(target_name.clone()));

                for method in methods {
                    if let Stmt::Functions(name, params, return_type, body) = method {
                        let arity = params.len();
                        let mut new_state =
                            CompilerState::new(name.clone(), arity, return_type.clone());
                        new_state.scope_depth = 1;

                        for (param_name, _param_type) in params {
                            new_state.locals.push(Local {
                                name: param_name.clone(),
                                depth: 1,
                            });
                        }

                        self.states.push(new_state);

                        for (i, (param_name, param_type)) in params.iter().enumerate() {
                            if let Some(t) = param_type {
                                self.emit(OpCode::LoadLocal(i));
                                self.emit(OpCode::LoadGlobal(t.clone()));
                                self.emit(OpCode::CheckIs);
                                self.emit(OpCode::Assert(format!(
                                    "TypeError: param {} must implement {}",
                                    param_name, t
                                )));
                            }
                        }

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

                let func_value = Arc::new(state.function);
                        self.emit(OpCode::Closure(func_value, upvalue_locs));

                        self.emit(OpCode::AddMethod(name.clone()));
                    }
                }
                self.emit(OpCode::Pop);
            }
            Stmt::Interface(name, methods) => {
                self.emit(OpCode::BuildInterface(methods.clone()));
                self.emit(OpCode::StoreGlobal(name.clone()));
            }
            Stmt::LetTuple(names, expr) => {
                self.compile_expr(expr);

                self.emit(OpCode::UnpackTuple(names.len()));

                let state = self.current_state();
                if state.scope_depth > 0 {
                    let depth = state.scope_depth;
                    for name in names {
                        self.current_state().locals.push(Local {
                            name: name.clone(),
                            depth,
                        });
                    }
                } else {
                    for name in names.iter().rev() {
                        self.emit(OpCode::StoreGlobal(name.clone()));
                    }
                }
            }
            Stmt::Spawn(callee, args) => {
                self.compile_expr(callee);
                for arg in args {
                    self.compile_expr(arg);
                }
                self.emit(OpCode::Spawn(args.len()));
                self.emit(OpCode::Pop);
            }
            Stmt::Defer(expr) => match &**expr {
                Expr::Call(callee, args) => {
                    self.compile_expr(callee);
                    for arg in args {
                        self.compile_expr(arg);
                    }
                    self.emit(OpCode::DeferCall(args.len()));
                }
                Expr::MethodCall(obj, method_name, args) => {
                    self.compile_expr(obj);
                    for arg in args {
                        self.compile_expr(arg);
                    }
                    self.emit(OpCode::DeferMethodCall(method_name.clone(), args.len()));
                }
                _ => unreachable!(),
            },
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
                    BinaryOp::LessEqual => self.emit(OpCode::LessEqual),
                    BinaryOp::GreaterEqual => self.emit(OpCode::GreaterEqual),
                    BinaryOp::NotEqual => {
                        self.emit(OpCode::Equal);
                        self.emit(OpCode::Not);
                    }
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
            Expr::Tuple(elements) => {
                let len = elements.len();
                for el in elements {
                    self.compile_expr(el);
                }
                self.emit(OpCode::BuildTuple(len));
            }
            Expr::ChanSend(chan, val) => {
                self.compile_expr(chan);
                self.compile_expr(val);
                self.emit(OpCode::ChanSend);
            }
            Expr::ChanRecv(chan) => {
                self.compile_expr(chan);
                self.emit(OpCode::ChanRecv);
            }
            Expr::Try(inner) => {
                self.compile_expr(inner);
                self.emit(OpCode::PropagateError);
            }
            Expr::Match { subject, arms } => {
                self.compile_match(subject, arms);
            }
            Expr::Select(arms) => {
                self.compile_select(arms);
            }
            Expr::Spawn(callee, args) => {
                self.compile_expr(callee);
                for arg in args {
                    self.compile_expr(arg);
                }
                self.emit(OpCode::Spawn(args.len()));
            }
        }
    }

    fn compile_select(&mut self, arms: &[crate::ast::SelectArm]) {
        use crate::ast::SelectArmKind;
        use crate::opcode::SelectCaseOp;

        // 1. Prepare cases and evaluate their channel/value expressions onto stack
        let mut case_ops = Vec::new();

        for arm in arms {
            match &arm.kind {
                SelectArmKind::Default => {
                    case_ops.push(SelectCaseOp::Default);
                }
                SelectArmKind::Recv(_var, chan) => {
                    self.compile_expr(chan);
                    case_ops.push(SelectCaseOp::Recv);
                }
                SelectArmKind::Send(chan, val) => {
                    self.compile_expr(val);
                    self.compile_expr(chan);
                    case_ops.push(SelectCaseOp::Send);
                }
            }
        }

        // Emit Select opcode (it consumes channel/value arguments and leaves exactly:
        // [val_slot, idx_slot]
        self.emit(OpCode::Select(case_ops));

        let current_depth = self.current_state().scope_depth + 1;
        let val_slot = self.current_state().locals.len();
        self.current_state().locals.push(Local {
            name: format!("<select_val_{}>", val_slot),
            depth: current_depth,
        });

        let idx_slot = self.current_state().locals.len();
        self.current_state().locals.push(Local {
            name: format!("<select_idx_{}>", idx_slot),
            depth: current_depth,
        });

        let mut end_jumps = Vec::new();

        for (idx, arm) in arms.iter().enumerate() {
            // Check if selected_case == idx
            self.emit(OpCode::LoadLocal(idx_slot));
            self.emit(OpCode::Push(Value::Int(idx as i64)));
            self.emit(OpCode::Equal);

            let next_case_jump = self.emit_jump(OpCode::JumpIfFalse(0));

            let arm_locals_start = self.current_state().locals.len();

            if let SelectArmKind::Recv(var_name, _) = &arm.kind {
                // Bind var_name from val_slot
                self.emit(OpCode::LoadLocal(val_slot));
                let depth = self.current_state().scope_depth;
                self.current_state().locals.push(Local {
                    name: var_name.clone(),
                    depth,
                });
            }

            self.compile_expr(&arm.body);
            // Result of arm body goes to val_slot (which will be final select result)
            self.emit(OpCode::SetLocal(val_slot));

            let pops = self.current_state().locals.len() - arm_locals_start;
            for _ in 0..pops {
                self.emit(OpCode::Pop);
            }
            self.current_state().locals.truncate(arm_locals_start);

            end_jumps.push(self.emit_jump(OpCode::Jump(0)));

            let next_target = self.current_ip();
            self.patch_jump(next_case_jump, next_target);
        }

        let end_target = self.current_ip();
        for j in end_jumps {
            self.patch_jump(j, end_target);
        }

        // Pop idx_slot from stack, leaving val_slot (the result) on top!
        self.emit(OpCode::Pop);
        self.current_state().locals.pop(); // pop idx_slot from compiler locals
        self.current_state().locals.pop(); // pop val_slot marker so it now holds expression result
    }

    fn compile_match(&mut self, subject: &Expr, arms: &[crate::ast::MatchArm]) {
        // 1. Evaluate subject and place it in a local slot on stack
        self.compile_expr(subject);

        let subj_depth = self.current_state().scope_depth + 1;
        let subj_slot = self.current_state().locals.len();
        self.current_state().locals.push(Local {
            name: format!("<match_subj_{}>", subj_slot),
            depth: subj_depth,
        });

        let mut end_jumps = Vec::new();

        for arm in arms {
            let arm_locals_start = self.current_state().locals.len();

            // 1. Compile pattern check for this arm (leaves 1 bool on stack)
            self.compile_pattern_check(&arm.pattern, subj_slot);

            // If pattern doesn't match, jump directly to next arm (no variables were bound)
            let fail_pattern_jump = self.emit_jump(OpCode::JumpIfFalse(0));

            // 2. Pattern matched! Now bind variables for guards and body
            self.bind_pattern_variables(&arm.pattern, subj_slot);
            let bound_count = self.current_state().locals.len() - arm_locals_start;

            // 3. Evaluate guard if present
            let mut guard_fail_jump = None;
            if let Some(ref guard) = arm.guard {
                self.compile_expr(guard);
                guard_fail_jump = Some(self.emit_jump(OpCode::JumpIfFalse(0)));
            }

            // 4. Evaluate arm body
            self.compile_expr(&arm.body);

            // Store result into subject slot (reusing slot for match result)
            self.emit(OpCode::SetLocal(subj_slot));

            // Pop bound variables from stack
            for _ in 0..bound_count {
                self.emit(OpCode::Pop);
            }
            self.current_state().locals.truncate(arm_locals_start);

            // Jump to end of entire match
            end_jumps.push(self.emit_jump(OpCode::Jump(0)));

            // 5. Failure paths
            if let Some(g_jump) = guard_fail_jump {
                let skip_cleanup = self.emit_jump(OpCode::Jump(0));

                let guard_fail_landing = self.current_ip();
                self.patch_jump(g_jump, guard_fail_landing);

                for _ in 0..bound_count {
                    self.emit(OpCode::Pop);
                }

                let arm_next = self.current_ip();
                self.patch_jump(skip_cleanup, arm_next);
                self.patch_jump(fail_pattern_jump, arm_next);
            } else {
                let next_target = self.current_ip();
                self.patch_jump(fail_pattern_jump, next_target);
            }
        }

        // Default if no arms matched: set nil to subject slot
        self.emit(OpCode::Push(Value::Nil));
        self.emit(OpCode::SetLocal(subj_slot));

        // Patch all successful arm jumps to here
        let end_target = self.current_ip();
        for j in end_jumps {
            self.patch_jump(j, end_target);
        }

        // Pop subject marker from compiler locals so stack slot now holds match result
        self.current_state().locals.pop();
    }

    fn bind_pattern_variables(&mut self, pattern: &crate::ast::Pattern, subj_slot: usize) {
        use crate::ast::Pattern;

        match pattern {
            Pattern::Variable(name) => {
                self.emit(OpCode::LoadLocal(subj_slot));
                let depth = self.current_state().scope_depth;
                self.current_state().locals.push(Local {
                    name: name.clone(),
                    depth,
                });
            }
            Pattern::Type(name, _) => {
                self.emit(OpCode::LoadLocal(subj_slot));
                let depth = self.current_state().scope_depth;
                self.current_state().locals.push(Local {
                    name: name.clone(),
                    depth,
                });
            }
            Pattern::Tuple(elements) => {
                for (idx, elem_pat) in elements.iter().enumerate() {
                    self.emit(OpCode::LoadLocal(subj_slot));
                    self.emit(OpCode::Push(Value::Int(idx as i64)));
                    self.emit(OpCode::IndexGet);

                    let depth = self.current_state().scope_depth;
                    let elem_slot = self.current_state().locals.len();
                    self.current_state().locals.push(Local {
                        name: format!("<tuple_elem_{}>", elem_slot),
                        depth,
                    });
                    self.bind_pattern_variables(elem_pat, elem_slot);
                }
            }
            Pattern::Or(alternatives) => {
                // If alternatives have variables, bind from first alternative
                if let Some(first) = alternatives.first() {
                    self.bind_pattern_variables(first, subj_slot);
                }
            }
            _ => {}
        }
    }

    fn compile_pattern_check(&mut self, pattern: &crate::ast::Pattern, subj_slot: usize) {
        use crate::ast::Pattern;

        match pattern {
            Pattern::Wildcard | Pattern::Variable(_) => {
                self.emit(OpCode::Push(Value::Bool(true)));
            }
            Pattern::Literal(val) => {
                self.emit(OpCode::LoadLocal(subj_slot));
                self.emit(OpCode::Push(val.clone()));
                self.emit(OpCode::Equal);
            }
            Pattern::Range { start, end, inclusive } => {
                // Check subj >= start
                self.emit(OpCode::LoadLocal(subj_slot));
                self.emit(OpCode::Push(Value::Int(*start)));
                self.emit(OpCode::GreaterEqual);

                // Check subj <= end (or < end)
                self.emit(OpCode::LoadLocal(subj_slot));
                self.emit(OpCode::Push(Value::Int(*end)));
                if *inclusive {
                    self.emit(OpCode::LessEqual);
                } else {
                    self.emit(OpCode::Less);
                }

                self.emit(OpCode::And);
            }
            Pattern::Type(_, type_name) => {
                self.emit(OpCode::LoadLocal(subj_slot));
                self.emit(OpCode::LoadGlobal(type_name.clone()));
                self.emit(OpCode::CheckIs);
            }
            Pattern::Tuple(elements) => {
                let expected_len = elements.len();
                if expected_len == 0 {
                    self.emit(OpCode::Push(Value::Bool(true)));
                    return;
                }

                for (idx, elem_pat) in elements.iter().enumerate() {
                    self.emit(OpCode::LoadLocal(subj_slot));
                    self.emit(OpCode::Push(Value::Int(idx as i64)));
                    self.emit(OpCode::IndexGet);

                    match elem_pat {
                        Pattern::Literal(val) => {
                            self.emit(OpCode::Push(val.clone()));
                            self.emit(OpCode::Equal);
                        }
                        Pattern::Range { start, end, inclusive } => {
                            let depth = self.current_state().scope_depth;
                            let elem_slot = self.current_state().locals.len();
                            self.current_state().locals.push(Local {
                                name: format!("<range_elem_{}>", elem_slot),
                                depth,
                            });
                            self.emit(OpCode::LoadLocal(elem_slot));
                            self.emit(OpCode::Push(Value::Int(*start)));
                            self.emit(OpCode::GreaterEqual);

                            self.emit(OpCode::LoadLocal(elem_slot));
                            self.emit(OpCode::Push(Value::Int(*end)));
                            if *inclusive {
                                self.emit(OpCode::LessEqual);
                            } else {
                                self.emit(OpCode::Less);
                            }
                            self.emit(OpCode::And);
                            self.current_state().locals.pop();
                        }
                        _ => {
                            self.emit(OpCode::Pop);
                            self.emit(OpCode::Push(Value::Bool(true)));
                        }
                    }

                    if idx > 0 {
                        self.emit(OpCode::And);
                    }
                }
            }
            Pattern::Or(alternatives) => {
                if alternatives.is_empty() {
                    self.emit(OpCode::Push(Value::Bool(false)));
                    return;
                }
                self.compile_pattern_check(&alternatives[0], subj_slot);
                for alt in &alternatives[1..] {
                    self.compile_pattern_check(alt, subj_slot);
                    self.emit(OpCode::Or);
                }
            }
        }
    }
}
