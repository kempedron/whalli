use crate::{
    heap,
    opcode::{OpCode, UpvalueLoc},
    value::{FunctionObj, Value},
};
use std::{collections::HashMap, rc::Rc};

pub struct CallFrame {
    pub closure_id: usize,
    pub function: Rc<FunctionObj>,
    pub ip: usize,
    pub stack_offset: usize,
}

pub struct RuntimeError {
    pub message: String,
    pub line: usize,
}

pub struct VM {
    stack: Vec<Value>,
    frames: Vec<CallFrame>,
    globals: HashMap<String, Value>,
    modules: HashMap<String, Value>,
    pub heap: heap::Heap,
    pub current_line: usize,
    pub gc_threshold: usize,
}

impl VM {
    pub fn new(instructions: Vec<OpCode>) -> Self {
        let main_func = Rc::new(FunctionObj {
            name: "main".to_string(),
            arity: 0,
            chunk: instructions,
            param_types: vec![],
        });

        let mut heap = heap::Heap::new();

        let main_closure_id = heap.alloc(heap::Obj::Closure(main_func.clone(), vec![]));

        let initial_frame = CallFrame {
            closure_id: main_closure_id,
            function: main_func,
            ip: 0,
            stack_offset: 0,
        };

        let (globals, modules) = crate::stdlib::register_natives(&mut heap);

        VM {
            stack: Vec::new(),
            frames: vec![initial_frame],
            globals,
            modules,
            current_line: 1,
            heap,
            gc_threshold: 1024,
        }
    }

    pub fn collect_garbage(&mut self) {
        for val in &self.stack {
            if let Value::ObjRef(id) = val {
                self.heap.mark(*id);
            }
        }

        for val in self.globals.values() {
            if let Value::ObjRef(id) = val {
                self.heap.mark(*id);
            }
        }

        for val in self.modules.values() {
            if let Value::ObjRef(id) = val {
                self.heap.mark(*id);
            }
        }

        // let freed = self.heap.sweep();
    }

    pub fn run(&mut self) -> Result<(), RuntimeError> {
        macro_rules! runtime_error {
            ($msg:expr) => {
                return Err(RuntimeError { message: $msg.to_string(), line: self.current_line })
            };
            ($fmt:expr, $($arg:expr),*) => {
                return Err(RuntimeError { message: format!($fmt, $($arg),*), line: self.current_line })
            };
        }

        macro_rules! pop {
            () => {
                self.stack.pop().ok_or_else(|| RuntimeError {
                    message: "Stack underflow (internal VM error)".to_string(),
                    line: self.current_line,
                })?
            };
        }

        while !self.frames.is_empty() {
            if self.heap.live_count() >= self.gc_threshold {
                self.collect_garbage();

                self.gc_threshold = std::cmp::max(self.heap.live_count() * 2, 1024)
            }

            let frame_idx = self.frames.len() - 1;

            if self.frames[frame_idx].ip >= self.frames[frame_idx].function.chunk.len() {
                self.frames.pop();
                continue;
            }

            let instruction =
                self.frames[frame_idx].function.chunk[self.frames[frame_idx].ip].clone();
            self.frames[frame_idx].ip += 1;

            match instruction {
                OpCode::Push(val) => {
                    self.stack.push(val);
                }

                OpCode::Add => {
                    let b = pop!();
                    let a = pop!();
                    match (a, b) {
                        (Value::Int(x), Value::Int(y)) => self.stack.push(Value::Int(x + y)),
                        (Value::Float(x), Value::Float(y)) => self.stack.push(Value::Float(x + y)),
                        (Value::Int(x), Value::Float(y)) => {
                            self.stack.push(Value::Float((x as f64) + y))
                        }
                        (Value::Float(x), Value::Int(y)) => {
                            self.stack.push(Value::Float(x + (y as f64)))
                        }
                        (Value::Str(x), Value::Str(y)) => {
                            self.stack.push(Value::Str(Rc::new(format!("{}{}", x, y))))
                        }
                        (Value::Str(x), y) => {
                            self.stack.push(Value::Str(Rc::new(format!("{}{}", x, y))))
                        }
                        (x, Value::Str(y)) => {
                            self.stack.push(Value::Str(Rc::new(format!("{}{}", x, y))))
                        }
                        _ => runtime_error!("Invalid types for '+' operation"),
                    }
                }
                OpCode::Sub => {
                    let b = pop!();
                    let a = pop!();
                    match (a, b) {
                        (Value::Int(x), Value::Int(y)) => self.stack.push(Value::Int(x - y)),
                        (Value::Float(x), Value::Float(y)) => self.stack.push(Value::Float(x - y)),
                        (Value::Int(x), Value::Float(y)) => {
                            self.stack.push(Value::Float((x as f64) - y))
                        }
                        (Value::Float(x), Value::Int(y)) => {
                            self.stack.push(Value::Float(x - (y as f64)))
                        }
                        _ => runtime_error!("Invalid types for '-' operation"),
                    }
                }
                OpCode::Mul => {
                    let b = pop!();
                    let a = pop!();
                    match (a, b) {
                        (Value::Int(x), Value::Int(y)) => self.stack.push(Value::Int(x * y)),
                        (Value::Float(x), Value::Float(y)) => self.stack.push(Value::Float(x * y)),
                        (Value::Int(x), Value::Float(y)) => {
                            self.stack.push(Value::Float((x as f64) * y))
                        }
                        (Value::Float(x), Value::Int(y)) => {
                            self.stack.push(Value::Float(x * (y as f64)))
                        }
                        _ => runtime_error!("Invalid types for '*' operation"),
                    }
                }
                OpCode::Div => {
                    let b = pop!();
                    let a = pop!();
                    match (a, b) {
                        (Value::Int(x), Value::Int(y)) => {
                            if y == 0 {
                                runtime_error!("Division by zero");
                            }
                            self.stack.push(Value::Int(x / y));
                        }
                        (Value::Float(x), Value::Float(y)) => self.stack.push(Value::Float(x / y)),
                        (Value::Int(x), Value::Float(y)) => {
                            self.stack.push(Value::Float((x as f64) / y))
                        }
                        (Value::Float(x), Value::Int(y)) => {
                            if y == 0 {
                                runtime_error!("Division by zero");
                            }
                            self.stack.push(Value::Float(x / (y as f64)));
                        }
                        _ => runtime_error!("Invalid types for '/' operation"),
                    }
                }
                OpCode::Mod => {
                    let b = pop!();
                    let a = pop!();
                    match (a, b) {
                        (Value::Int(x), Value::Int(y)) => {
                            if y == 0 {
                                runtime_error!("Modulo by zero");
                            }
                            self.stack.push(Value::Int(x % y));
                        }
                        (Value::Float(x), Value::Float(y)) => self.stack.push(Value::Float(x % y)),
                        (Value::Int(x), Value::Float(y)) => {
                            self.stack.push(Value::Float((x as f64) % y))
                        }
                        (Value::Float(x), Value::Int(y)) => {
                            if y == 0 {
                                runtime_error!("Modulo by zero");
                            }
                            self.stack.push(Value::Float(x % (y as f64)));
                        }
                        _ => runtime_error!("Invalid types for '%' operation"),
                    }
                }

                OpCode::StoreGlobal(name) => {
                    let val = pop!();
                    self.globals.insert(name.clone(), val);
                }
                OpCode::LoadGlobal(name) => {
                    if let Some(val) = self.globals.get(&name) {
                        self.stack.push(val.clone());
                    } else {
                        runtime_error!("Undefined variable '{}'", name);
                    }
                }
                OpCode::LoadLocal(idx) => {
                    let offset = self.frames[frame_idx].stack_offset;
                    let val = self.stack[offset + idx].clone();
                    self.stack.push(val);
                }
                OpCode::SetLocal(idx) => {
                    let val = pop!();
                    let offset = self.frames[frame_idx].stack_offset;
                    self.stack[offset + idx] = val;
                }

                OpCode::Call(arg_count) => {
                    let callee_index = self.stack.len() - arg_count - 1;
                    let callee = self.stack[callee_index].clone();

                    match callee {
                        Value::ObjRef(id) => {
                            let obj = self.heap.get(id).map_err(|e| RuntimeError { message: e, line: self.current_line })?;
                            match obj {
                                heap::Obj::Closure(func, _) => {
                                    if func.arity != arg_count { runtime_error!("Function '{}' expects {} arguments, got {}", func.name, func.arity, arg_count); }
                                    let new_frame = CallFrame { closure_id: id, function: func.clone(), ip: 0, stack_offset: callee_index + 1 };
                                    self.frames.push(new_frame);
                                }
                                heap::Obj::StructDef { name, fields, .. } => {
                                    if fields.len() != arg_count { runtime_error!("Struct '{}' expects {} arguments, got {}", name, fields.len(), arg_count); }
                                    let mut instance_fields = HashMap::new();
                                    
                                    let mut args = Vec::with_capacity(arg_count);
                                    for _ in 0..arg_count { args.push(pop!()); }
                                    args.reverse();
                                    pop!();
                                    
                                    for (i, field_name) in fields.iter().enumerate() {
                                        instance_fields.insert(field_name.clone(), args[i].clone());
                                    }
                                    
                                    let instance_id = self.heap.alloc(heap::Obj::Instance { struct_id: id, fields: instance_fields });
                                    self.stack.push(Value::ObjRef(instance_id));
                                }
                                _ => runtime_error!("Attempt to call a non-callable object"),
                            }
                        
                        }
                        Value::Native(native_fn) => {
                            let mut args = Vec::with_capacity(arg_count);
                            for _ in 0..arg_count {
                                args.push(pop!());
                            }
                            args.reverse();
                            pop!();
                            let result = native_fn(args);
                            self.stack.push(result);
                        }
                        _ => runtime_error!("Attempt to call a non-function value"),
                    }
                }

                OpCode::MethodCall(method_name, arg_count) => {
                    let mut args = Vec::with_capacity(arg_count);
                    for _ in 0..arg_count { args.push(pop!()); }
                    args.reverse();
                    let obj = pop!(); 
                    
                    let mut handled = false;
                    
                    if let Value::ObjRef(id) = &obj {
                        if let Ok(heap::Obj::Instance { struct_id, .. }) = self.heap.get(*id) {
                            if let Ok(heap::Obj::StructDef { methods, .. }) = self.heap.get(*struct_id) {
                                if let Some(method_val) = methods.get(&method_name) {
                                    handled = true;
                                    
                                    self.stack.push(obj.clone()); 
                                    for arg in &args { self.stack.push(arg.clone()); } 
                                    
                                    let callee_index = self.stack.len() - arg_count - 1;
                                    
                                    if let Value::ObjRef(closure_id) = method_val {
                                        if let Ok(heap::Obj::Closure(func, _)) = self.heap.get(*closure_id) {
                                            if func.arity != arg_count + 1 { runtime_error!("Method '{}' expects {} arguments (including self), got {}", method_name, func.arity, arg_count + 1); }
                                            let new_frame = CallFrame { 
                                                closure_id: *closure_id, 
                                                function: func.clone(), 
                                                ip: 0, 
                                                stack_offset: callee_index 
                                            };
                                            self.frames.push(new_frame);
                                        }
                                    }
                                }
                            }
                        }
                        
                        if !handled {
                            if let Ok(heap::Obj::Map(map)) = self.heap.get(*id) {
                                if let Some(val) = map.get(&method_name) {
                                    handled = true;
                                    self.stack.push(val.clone()); 
                                    for arg in &args { self.stack.push(arg.clone()); } 
                                    
                                    let callee_index = self.stack.len() - arg_count - 1;
                                    match self.stack[callee_index].clone() {
                                        Value::Native(native_fn) => {
                                            let mut n_args = Vec::new();
                                            for _ in 0..arg_count { n_args.push(pop!()); }
                                            n_args.reverse();
                                            pop!(); 
                                            let res = native_fn(n_args);
                                            self.stack.push(res);
                                        }
                                        Value::ObjRef(cid) => {
                                            if let Ok(heap::Obj::Closure(func, _)) = self.heap.get(cid) {
                                                if func.arity != arg_count { runtime_error!("Wrong arity"); }
                                                let new_frame = CallFrame { closure_id: cid, function: func.clone(), ip: 0, stack_offset: callee_index + 1 };
                                                self.frames.push(new_frame);
                                            }
                                        }
                                        _ => runtime_error!("Property is not callable"),
                                    }
                                }
                            }
                        }
                    }
                    
                    if !handled {
                        match obj.call_method(&method_name, args, &mut self.heap) {
                            Ok(result) => self.stack.push(result),
                            Err(err_msg) => runtime_error!("{}", err_msg),
                        }
                    }
                }

                OpCode::BuildList(size) => {
                    let start = self.stack.len() - size;
                    let elements: Vec<Value> = self.stack.drain(start..).collect();
                    let id = self.heap.alloc(heap::Obj::List(elements));
                    self.stack.push(Value::ObjRef(id));
                }

                OpCode::ListLen => {
                    let val = pop!();
                    if let Value::ObjRef(id) = val {
                        if let Ok(heap::Obj::List(list)) = self.heap.get(id) {
                            self.stack.push(Value::Int(list.len() as i64));
                        }
                    } else {
                        runtime_error!("Attempt to get length of a non-list");
                    }
                }

                OpCode::BuildMap(size) => {
                    let mut map = HashMap::new();
                    for _ in 0..size {
                        let val = pop!();
                        let key = pop!();
                        let key_str = match key {
                            Value::Str(s) => (*s).clone(),
                            _ => runtime_error!("Map keys must be strings"),
                        };
                        map.insert(key_str, val);
                    }
                    let id = self.heap.alloc(heap::Obj::Map(map));
                    self.stack.push(Value::ObjRef(id));
                }

                OpCode::IndexGet => {
                    let index = pop!();
                    let collection = pop!();

                    match collection {
                        Value::ObjRef(id) => {
                            let obj = self.heap.get(id).map_err(|e| RuntimeError {
                                message: e,
                                line: self.current_line,
                            })?;
                            match (obj, index) {
                                (crate::heap::Obj::List(list), Value::Int(idx)) => {
                                    if idx < 0 || idx >= list.len() as i64 {
                                        runtime_error!("Index {} out of bounds", idx);
                                    }
                                    self.stack.push(list[idx as usize].clone());
                                }
                                (crate::heap::Obj::Map(map), Value::Str(key)) => {
                                    let val = map.get(&*key).unwrap_or(&Value::Nil);
                                    self.stack.push(val.clone());
                                }
                                (crate::heap::Obj::Instance { fields, .. }, Value::Str(key)) => {
                                    let val = fields.get(&*key).unwrap_or(&Value::Nil);
                                    self.stack.push(val.clone());
                                }

                                _ => runtime_error!("Invalid index type for collection"),
                            }
                        }
                        Value::Str(s) => {
                            if let Value::Int(idx) = index {
                                if idx < 0 || idx >= s.len() as i64 {
                                    runtime_error!("String index out of bounds");
                                }
                                let ch = s.chars().nth(idx as usize).unwrap().to_string();
                                self.stack.push(Value::Str(std::rc::Rc::new(ch)));
                            } else {
                                runtime_error!("String index must be integer");
                            }
                        }
                        
                        _ => runtime_error!("Invalid target for reading index"),
                    }
                }

                OpCode::IndexSet => {
                    let value = pop!();
                    let index = pop!();
                    let collection = pop!();

                    match collection {
                        Value::ObjRef(id) => {
                            let obj = self.heap.get_mut(id).map_err(|e| RuntimeError {
                                message: e,
                                line: self.current_line,
                            })?;
                            match (obj, index) {
                                (crate::heap::Obj::List(list), Value::Int(idx)) => {
                                    if idx < 0 || idx >= list.len() as i64 {
                                        runtime_error!("Index {} out of bounds", idx);
                                    }
                                    list[idx as usize] = value;
                                }
                                (crate::heap::Obj::Map(map), Value::Str(key)) => {
                                    map.insert((*key).clone(), value);
                                }
                                (crate::heap::Obj::Instance { fields, .. }, Value::Str(key)) => {
                                    fields.insert((*key).clone(), value);
                                }
                                _ => runtime_error!("Invalid index type for collection"),
                            }
                        }
                        _ => runtime_error!(
                            "Invalid target for assignment (only lists and maps are mutable)"
                        ),
                    }
                }

                OpCode::Import(module_name) => {
                    if let Some(module_val) = self.modules.get(&module_name) {
                        self.globals.insert(module_name.clone(), module_val.clone());
                    } else {
                        runtime_error!("Module '{}' not found", module_name)
                    }
                }

                OpCode::Return => {
                    let result = self.stack.pop().unwrap_or(Value::Nil);
                    let frame = self.frames.pop().unwrap();

                    if frame.stack_offset > 0 {
                        self.stack.truncate(frame.stack_offset - 1);
                    }
                    if !self.frames.is_empty() {
                        self.stack.push(result);
                    }
                }

                OpCode::JumpIfFalse(target_ip) => {
                    let condition = pop!();
                    if let Value::Bool(false) = condition {
                        self.frames[frame_idx].ip = target_ip;
                    }
                }
                OpCode::Jump(target_ip) => {
                    self.frames[frame_idx].ip = target_ip;
                }
                OpCode::Equal => {
                    let b = pop!();
                    let a = pop!();
                    self.stack.push(Value::Bool(a == b));
                }
                OpCode::Less => {
                    let b = pop!();
                    let a = pop!();
                    let res = match (a, b) {
                        (Value::Int(x), Value::Int(y)) => x < y,
                        (Value::Float(x), Value::Float(y)) => x < y,
                        (Value::Int(x), Value::Float(y)) => (x as f64) < y,
                        (Value::Float(x), Value::Int(y)) => x < (y as f64),
                        _ => runtime_error!("Invalid types for '<' operation"),
                    };
                    self.stack.push(Value::Bool(res));
                }
                OpCode::Greater => {
                    let b = pop!();
                    let a = pop!();
                    let res = match (a, b) {
                        (Value::Int(x), Value::Int(y)) => x > y,
                        (Value::Float(x), Value::Float(y)) => x > y,
                        (Value::Int(x), Value::Float(y)) => (x as f64) > y,
                        (Value::Float(x), Value::Int(y)) => x > (y as f64),
                        _ => runtime_error!("Invalid types for '>' operation"),
                    };
                    self.stack.push(Value::Bool(res));
                }
                OpCode::And => {
                    let b = pop!();
                    let a = pop!();
                    if let (Value::Bool(x), Value::Bool(y)) = (a, b) {
                        self.stack.push(Value::Bool(x && y));
                    } else {
                        runtime_error!("'and' expects booleans");
                    }
                }
                OpCode::Or => {
                    let b = pop!();
                    let a = pop!();
                    if let (Value::Bool(x), Value::Bool(y)) = (a, b) {
                        self.stack.push(Value::Bool(x || y));
                    } else {
                        runtime_error!("'or' expects booleans");
                    }
                }
                OpCode::Not => {
                    let a = pop!();
                    if let Value::Bool(x) = a {
                        self.stack.push(Value::Bool(!x));
                    } else {
                        runtime_error!("'not' expects a boolean");
                    }
                }
                OpCode::Pop => {
                    pop!();
                }
                OpCode::SetLine(line) => {
                    self.current_line = line;
                }
                OpCode::Closure(func, upvalues) => {
                    let mut captured = Vec::new();
                    for loc in upvalues {
                        match loc {
                            UpvalueLoc::Local(idx) => {
                                let offset = self.frames[frame_idx].stack_offset;
                                let val = self.stack[offset + idx].clone();
                                let upvalue_id = self.heap.alloc(heap::Obj::Upvalue(val));
                                captured.push(upvalue_id);
                            }
                            UpvalueLoc::Upvalue(idx) => {
                                let current_closure_id = self.frames[frame_idx].closure_id;
                                if let Ok(heap::Obj::Closure(_, upvs)) =
                                    self.heap.get(current_closure_id)
                                {
                                    captured.push(upvs[idx]);
                                }
                            }
                        }
                    }

                    let closure_id = self.heap.alloc(heap::Obj::Closure(func, captured));
                    self.stack.push(Value::ObjRef(closure_id));
                }
                OpCode::GetUpvalue(idx) => {
                    let closure_id = self.frames[frame_idx].closure_id;
                    if let Ok(heap::Obj::Closure(_, upvs)) = self.heap.get(closure_id) {
                        let upvalue_id = upvs[idx];
                        if let Ok(heap::Obj::Upvalue(val)) = self.heap.get(upvalue_id) {
                            self.stack.push(val.clone());
                        }
                    }
                }

                OpCode::SetUpvalue(idx) => {
                    let val = pop!();
                    let closure_id = self.frames[frame_idx].closure_id;

                    let upvalue_id = {
                        if let Ok(heap::Obj::Closure(_, upvs)) = self.heap.get(closure_id) {
                            upvs[idx]
                        } else {
                            unreachable!()
                        }
                    };

                    if let Ok(heap::Obj::Upvalue(inner)) = self.heap.get_mut(upvalue_id) {
                        *inner = val;
                    }
                }
                OpCode::BuildStruct(name, fields) => {
                    let id = self.heap.alloc(heap::Obj::StructDef {
                        name,
                        fields,
                        methods: HashMap::new(), 
                    });
                    self.stack.push(Value::ObjRef(id));
                }
                OpCode::AddMethod(name) => {
                    let method = pop!();
                    let struct_val = self.stack.last().unwrap().clone();
                    if let Value::ObjRef(id) = struct_val {
                        if let Ok(heap::Obj::StructDef { methods, .. }) = self.heap.get_mut(id) {
                            methods.insert(name, method);
                        }
                    }
                }
                OpCode::BuildInterface(methods) => {
                    let id = self.heap.alloc(heap::Obj::Interface(methods));
                    self.stack.push(Value::ObjRef(id));
                }
                OpCode::CheckIs => {
                    let right = pop!();
                    let left = pop!();
                    
                    let mut is_match = false;
                    
                    if let Value::ObjRef(iface_id) = right {
                        if let Ok(heap::Obj::Interface(required_methods)) = self.heap.get(iface_id) {
                            
                            if let Value::ObjRef(inst_id) = left {
                                if let Ok(heap::Obj::Instance { struct_id, .. }) = self.heap.get(inst_id) {
                                    
                                    if let Ok(heap::Obj::StructDef { methods, .. }) = self.heap.get(*struct_id) {
                                        
                                        is_match = required_methods.iter().all(|m| methods.contains_key(m));
                                    }
                                }
                            }
                        }
                    }
                    
                    self.stack.push(Value::Bool(is_match));
                }
            }
        }
        Ok(())
    }
}
