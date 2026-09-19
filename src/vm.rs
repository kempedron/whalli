use crate::{
    heap,
    opcode::{OpCode, UpvalueLoc},
    value::{FunctionObj, Value},
};
use std::{
    collections::{HashMap, VecDeque},
    rc::Rc,
    time::{Duration, SystemTime},
};

use mio::net::{TcpListener, TcpStream};
use mio::{Events, Poll, Token};

pub struct CallFrame {
    pub closure_id: usize,
    pub function: Rc<FunctionObj>,
    pub ip: usize,
    pub stack_offset: usize,
    pub base_offset: usize,
}

pub struct RuntimeError {
    pub message: String,
    pub line: usize,
}

pub enum TaskState {
    Runnable,
    Sleeping(SystemTime),
    Waiting,
    WaitingIO(Token),
}

pub struct Task {
    pub stack: Vec<Value>,
    pub frames: Vec<CallFrame>,
    pub state: TaskState,
}

pub struct VM {
    tasks: VecDeque<Task>,
    globals: HashMap<String, Value>,
    modules: HashMap<String, Value>,
    pub heap: heap::Heap,
    pub current_line: usize,
    pub gc_threshold: usize,

    pub poll: Poll,
    pub listeners: HashMap<usize, TcpListener>,
    pub streams: HashMap<usize, TcpStream>,
    pub next_token: usize,
}

impl VM {
    pub fn new(instructions: Vec<OpCode>) -> Self {
        let main_func = Rc::new(FunctionObj {
            name: "main".to_string(),
            arity: 0,
            chunk: instructions,
            param_types: vec![],
            return_type: None,
        });

        let mut heap = heap::Heap::new();

        let main_closure_id = heap.alloc(heap::Obj::Closure(main_func.clone(), vec![]));

        let initial_frame = CallFrame {
            closure_id: main_closure_id,
            function: main_func,
            ip: 0,
            stack_offset: 0,
            base_offset: 0,
        };

        let mut tasks = VecDeque::new();

        let main_task = Task {
            stack: Vec::new(),
            frames: vec![initial_frame],
            state: TaskState::Runnable,
        };
        tasks.push_back(main_task);

        let mut vm = VM {
            tasks,
            globals: HashMap::new(),
            modules: HashMap::new(),
            current_line: 1,
            heap,
            gc_threshold: 1024,
            poll: Poll::new().unwrap(),
            listeners: HashMap::new(),
            streams: HashMap::new(),
            next_token: 1,
        };

        let (mut globals, modules) = crate::stdlib::register_natives(&mut vm);

        let builtins = ["int", "float", "str", "bool", "list", "map", "func", "chan"];

        for builtin in builtins {
            globals.insert(
                builtin.to_string(),
                Value::Str(Rc::new(builtin.to_string())),
            );
        }

        vm.globals = globals;
        vm.modules = modules;

        return vm;
    }

    pub fn collect_garbage(&mut self, current_task: &Task) {
        for val in &current_task.stack {
            if let Value::ObjRef(id) = val {
                self.heap.mark(*id);
            }
        }

        for task in &self.tasks {
            for val in &task.stack {
                if let Value::ObjRef(id) = val {
                    self.heap.mark(*id);
                }
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

        // self.heap.sweep();
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

        let mut events = Events::with_capacity(128);

        loop {
            if self.tasks.is_empty() {
                break;
            }

            let mut has_runnable = false;
            let mut next_wake_time: Option<SystemTime> = None;
            let active_tasks_count = self.tasks.len();

            for _ in 0..active_tasks_count {
                let mut current_task = self.tasks.pop_front().unwrap();
                let now = SystemTime::now();

                match current_task.state {
                    TaskState::Runnable => {}
                    TaskState::Sleeping(wake_time) => {
                        if now >= wake_time {
                            current_task.state = TaskState::Runnable;
                        } else {
                            if next_wake_time.map_or(true, |t| wake_time < t) {
                                next_wake_time = Some(wake_time);
                            }
                            self.tasks.push_back(current_task);
                            continue;
                        }
                    }
                    TaskState::Waiting => {
                        self.tasks.push_back(current_task);
                        continue;
                    }
                    TaskState::WaitingIO(_) => {
                        self.tasks.push_back(current_task);
                        continue;
                    }
                }

                has_runnable = true;

                if self.heap.live_count() >= self.gc_threshold {
                    self.collect_garbage(&current_task);
                    self.gc_threshold = std::cmp::max(self.heap.live_count() * 2, 1024);
                }

                macro_rules! pop {
                    () => {
                        current_task.stack.pop().ok_or_else(|| RuntimeError {
                            message: "Stack underflow".to_string(),
                            line: self.current_line,
                        })?
                    };
                }

                let mut fuel = 2000;

                while fuel > 0 && !current_task.frames.is_empty() {
                    let frame_idx = current_task.frames.len() - 1;

                    if current_task.frames[frame_idx].ip
                        >= current_task.frames[frame_idx].function.chunk.len()
                    {
                        current_task.frames.pop();
                        continue;
                    }

                    let instruction = current_task.frames[frame_idx].function.chunk
                        [current_task.frames[frame_idx].ip]
                        .clone();
                    current_task.frames[frame_idx].ip += 1;

                    match instruction {
                        OpCode::Push(val) => {
                            current_task.stack.push(val);
                        }
                        OpCode::Add => {
                            let b = pop!();
                            let a = pop!();
                            match (a, b) {
                                (Value::Int(x), Value::Int(y)) => {
                                    current_task.stack.push(Value::Int(x + y))
                                }
                                (Value::Float(x), Value::Float(y)) => {
                                    current_task.stack.push(Value::Float(x + y))
                                }
                                (Value::Int(x), Value::Float(y)) => {
                                    current_task.stack.push(Value::Float((x as f64) + y))
                                }
                                (Value::Float(x), Value::Int(y)) => {
                                    current_task.stack.push(Value::Float(x + (y as f64)))
                                }
                                (Value::Str(x), Value::Str(y)) => current_task
                                    .stack
                                    .push(Value::Str(Rc::new(format!("{}{}", x, y)))),
                                (Value::Str(x), y) => {
                                    let y_str = y.stringify(&self.heap);
                                    current_task
                                        .stack
                                        .push(Value::Str(Rc::new(format!("{}{}", x, y_str))))
                                }
                                (x, Value::Str(y)) => {
                                    let x_str = x.stringify(&self.heap);
                                    current_task
                                        .stack
                                        .push(Value::Str(Rc::new(format!("{}{}", x_str, y))))
                                }
                                _ => runtime_error!("Invalid types for '+' operation"),
                            }
                        }
                        OpCode::Sub => {
                            let b = pop!();
                            let a = pop!();
                            match (a, b) {
                                (Value::Int(x), Value::Int(y)) => {
                                    current_task.stack.push(Value::Int(x - y))
                                }
                                (Value::Float(x), Value::Float(y)) => {
                                    current_task.stack.push(Value::Float(x - y))
                                }
                                (Value::Int(x), Value::Float(y)) => {
                                    current_task.stack.push(Value::Float((x as f64) - y))
                                }
                                (Value::Float(x), Value::Int(y)) => {
                                    current_task.stack.push(Value::Float(x - (y as f64)))
                                }
                                _ => runtime_error!("Invalid types for '-' operation"),
                            }
                        }
                        OpCode::Mul => {
                            let b = pop!();
                            let a = pop!();
                            match (a, b) {
                                (Value::Int(x), Value::Int(y)) => {
                                    current_task.stack.push(Value::Int(x * y))
                                }
                                (Value::Float(x), Value::Float(y)) => {
                                    current_task.stack.push(Value::Float(x * y))
                                }
                                (Value::Int(x), Value::Float(y)) => {
                                    current_task.stack.push(Value::Float((x as f64) * y))
                                }
                                (Value::Float(x), Value::Int(y)) => {
                                    current_task.stack.push(Value::Float(x * (y as f64)))
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
                                    current_task.stack.push(Value::Int(x / y));
                                }
                                (Value::Float(x), Value::Float(y)) => {
                                    current_task.stack.push(Value::Float(x / y))
                                }
                                (Value::Int(x), Value::Float(y)) => {
                                    current_task.stack.push(Value::Float((x as f64) / y))
                                }
                                (Value::Float(x), Value::Int(y)) => {
                                    if y == 0 {
                                        runtime_error!("Division by zero");
                                    }
                                    current_task.stack.push(Value::Float(x / (y as f64)));
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
                                    current_task.stack.push(Value::Int(x % y));
                                }
                                (Value::Float(x), Value::Float(y)) => {
                                    current_task.stack.push(Value::Float(x % y))
                                }
                                (Value::Int(x), Value::Float(y)) => {
                                    current_task.stack.push(Value::Float((x as f64) % y))
                                }
                                (Value::Float(x), Value::Int(y)) => {
                                    if y == 0 {
                                        runtime_error!("Modulo by zero");
                                    }
                                    current_task.stack.push(Value::Float(x % (y as f64)));
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
                                current_task.stack.push(val.clone());
                            } else {
                                runtime_error!("Undefined variable '{}'", name);
                            }
                        }
                        OpCode::LoadLocal(idx) => {
                            let offset = current_task.frames[frame_idx].stack_offset;
                            let val = current_task.stack[offset + idx].clone();
                            current_task.stack.push(val);
                        }
                        OpCode::SetLocal(idx) => {
                            let val = pop!();
                            let offset = current_task.frames[frame_idx].stack_offset;
                            current_task.stack[offset + idx] = val;
                        }

                        OpCode::Call(arg_count) => {
                            let callee_index = current_task.stack.len() - arg_count - 1;
                            let callee = current_task.stack[callee_index].clone();

                            match callee {
                                Value::ObjRef(id) => {
                                    let obj = self.heap.get(id).map_err(|e| RuntimeError {
                                        message: e,
                                        line: self.current_line,
                                    })?;
                                    match obj {
                                        heap::Obj::Closure(func, _) => {
                                            if func.arity != arg_count {
                                                runtime_error!(
                                                    "Function '{}' expects {} arguments, got {}",
                                                    func.name,
                                                    func.arity,
                                                    arg_count
                                                );
                                            }
                                            let new_frame = CallFrame {
                                                closure_id: id,
                                                function: func.clone(),
                                                ip: 0,
                                                stack_offset: callee_index + 1,
                                                base_offset: callee_index,
                                            };
                                            current_task.frames.push(new_frame);
                                        }
                                        heap::Obj::StructDef { name, fields, .. } => {
                                            if fields.len() != arg_count {
                                                runtime_error!(
                                                    "Struct '{}' expects {} arguments, got {}",
                                                    name,
                                                    fields.len(),
                                                    arg_count
                                                );
                                            }
                                            let mut instance_fields = HashMap::new();
                                            let mut args = Vec::with_capacity(arg_count);
                                            for _ in 0..arg_count {
                                                args.push(pop!());
                                            }
                                            args.reverse();
                                            pop!();

                                            for (i, field_name) in fields.iter().enumerate() {
                                                instance_fields
                                                    .insert(field_name.clone(), args[i].clone());
                                            }

                                            let instance_id =
                                                self.heap.alloc(heap::Obj::Instance {
                                                    struct_id: id,
                                                    fields: instance_fields,
                                                });
                                            current_task.stack.push(Value::ObjRef(instance_id));
                                        }
                                        _ => {
                                            runtime_error!("Attempt to call a non-callable object")
                                        }
                                    }
                                }
                                Value::Native(native_fn) => {
                                    let mut args = Vec::with_capacity(arg_count);
                                    for _ in 0..arg_count {
                                        args.push(pop!());
                                    }
                                    args.reverse();
                                    pop!();

                                    match native_fn(args.clone(), self) {
                                        crate::value::NativeResult::Return(val) => {
                                            current_task.stack.push(val);
                                        }
                                        crate::value::NativeResult::SuspendSleep(secs) => {
                                            let wake_time =
                                                SystemTime::now() + Duration::from_secs_f64(secs);
                                            current_task.state = TaskState::Sleeping(wake_time);
                                            current_task.stack.push(Value::Nil);
                                            break;
                                        }
                                        crate::value::NativeResult::SuspendIO(token) => {
                                            current_task.frames[frame_idx].ip -= 1;
                                            current_task.stack.push(callee);
                                            for arg in args {
                                                current_task.stack.push(arg);
                                            }
                                            current_task.state = TaskState::WaitingIO(token);
                                            break;
                                        }
                                    }
                                }
                                _ => runtime_error!("Attempt to call a non-function value"),
                            }
                        }

                        OpCode::MethodCall(method_name, arg_count) => {
                            let mut args = Vec::with_capacity(arg_count);
                            for _ in 0..arg_count {
                                args.push(pop!());
                            }
                            args.reverse();
                            let obj = pop!();

                            let mut handled = false;

                            if let Value::ObjRef(id) = &obj {
                                if let Ok(heap::Obj::Instance { struct_id, .. }) =
                                    self.heap.get(*id)
                                {
                                    if let Ok(heap::Obj::StructDef { methods, .. }) =
                                        self.heap.get(*struct_id)
                                    {
                                        if let Some(method_val) = methods.get(&method_name) {
                                            handled = true;
                                            current_task.stack.push(obj.clone());
                                            for arg in &args {
                                                current_task.stack.push(arg.clone());
                                            }

                                            let callee_index =
                                                current_task.stack.len() - arg_count - 1;

                                            if let Value::ObjRef(closure_id) = method_val {
                                                if let Ok(heap::Obj::Closure(func, _)) =
                                                    self.heap.get(*closure_id)
                                                {
                                                    if func.arity != arg_count + 1 {
                                                        runtime_error!(
                                                            "Method '{}' expects {} arguments (including self), got {}",
                                                            method_name,
                                                            func.arity,
                                                            arg_count + 1
                                                        );
                                                    }
                                                    let new_frame = CallFrame {
                                                        closure_id: *closure_id,
                                                        function: func.clone(),
                                                        ip: 0,
                                                        stack_offset: callee_index,
                                                        base_offset: callee_index,
                                                    };
                                                    current_task.frames.push(new_frame);
                                                }
                                            }
                                        }
                                    }
                                }

                                if !handled {
                                    if let Ok(heap::Obj::Map(map)) = self.heap.get(*id) {
                                        if let Some(val) = map.get(&method_name) {
                                            handled = true;
                                            current_task.stack.push(val.clone());
                                            for arg in &args {
                                                current_task.stack.push(arg.clone());
                                            }

                                            let callee_index =
                                                current_task.stack.len() - arg_count - 1;
                                            match current_task.stack[callee_index].clone() {
                                                Value::Native(native_fn) => {
                                                    let mut n_args = Vec::new();
                                                    for _ in 0..arg_count {
                                                        n_args.push(pop!());
                                                    }
                                                    n_args.reverse();

                                                    let _method_callee = pop!();

                                                    match native_fn(n_args.clone(), self) {
                                                        crate::value::NativeResult::Return(val) => { current_task.stack.push(val); }
                                                        crate::value::NativeResult::SuspendSleep(secs) => {
                                                            let wake_time = SystemTime::now() + Duration::from_secs_f64(secs);
                                                            current_task.state = TaskState::Sleeping(wake_time);
                                                            current_task.stack.push(Value::Nil);
                                                            break;
                                                        }
                                                        crate::value::NativeResult::SuspendIO(token) => {
                                                            current_task.frames[frame_idx].ip -= 1; 
                                                            
                                                            current_task.stack.push(obj.clone());
                                                            
                                                            for arg in n_args { 
                                                                current_task.stack.push(arg);
                                                            }
                                                            current_task.state = TaskState::WaitingIO(token);
                                                            break;
                                                        }
                                                    }
                                                }
                                                Value::ObjRef(cid) => {
                                                    if let Ok(heap::Obj::Closure(func, _)) =
                                                        self.heap.get(cid)
                                                    {
                                                        if func.arity != arg_count {
                                                            runtime_error!("Wrong arity");
                                                        }
                                                        let new_frame = CallFrame {
                                                            closure_id: cid,
                                                            function: func.clone(),
                                                            ip: 0,
                                                            stack_offset: callee_index + 1,
                                                            base_offset: callee_index,
                                                        };
                                                        current_task.frames.push(new_frame);
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
                                    Ok(result) => current_task.stack.push(result),
                                    Err(err_msg) => runtime_error!("{}", err_msg),
                                }
                            }
                        }

                        OpCode::BuildList(size) => {
                            let start = current_task.stack.len() - size;
                            let elements: Vec<Value> = current_task.stack.drain(start..).collect();
                            let id = self.heap.alloc(heap::Obj::List(elements));
                            current_task.stack.push(Value::ObjRef(id));
                        }

                        OpCode::ListLen => {
                            let val = pop!();
                            if let Value::ObjRef(id) = val {
                                if let Ok(heap::Obj::List(list)) = self.heap.get(id) {
                                    current_task.stack.push(Value::Int(list.len() as i64));
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
                            current_task.stack.push(Value::ObjRef(id));
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
                                            current_task.stack.push(list[idx as usize].clone());
                                        }
                                        (crate::heap::Obj::Map(map), Value::Str(key)) => {
                                            let val = map.get(&*key).unwrap_or(&Value::Nil);
                                            current_task.stack.push(val.clone());
                                        }
                                        (
                                            crate::heap::Obj::Instance { fields, .. },
                                            Value::Str(key),
                                        ) => {
                                            let val = fields.get(&*key).unwrap_or(&Value::Nil);
                                            current_task.stack.push(val.clone());
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
                                        current_task.stack.push(Value::Str(std::rc::Rc::new(ch)));
                                    } else {
                                        runtime_error!("String index must be integer");
                                    }
                                }
                                Value::Tuple(elements) => {
                                    if let Value::Int(idx) = index {
                                        if idx < 0 || (idx as usize) >= elements.len() {
                                            runtime_error!("Tuple index out of bounds");
                                        }
                                        current_task.stack.push(elements[idx as usize].clone());
                                    } else {
                                        runtime_error!("Tuple index must be an integer");
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
                                        (
                                            crate::heap::Obj::Instance { fields, .. },
                                            Value::Str(key),
                                        ) => {
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
                            let result = current_task.stack.pop().unwrap_or(Value::Nil);
                            let frame = current_task.frames.pop().unwrap();

                            if let Some(expected_type) = &frame.function.return_type {
                                let type_val =
                                    self.globals.get(expected_type).cloned().unwrap_or_else(|| {
                                        Value::Str(Rc::new(expected_type.clone()))
                                    });
                                if !self.is_type_match(&result, &type_val) {
                                    runtime_error!(
                                        "TypeError: function {} expected to return {}, but got {}",
                                        frame.function.name,
                                        expected_type,
                                        type_val
                                    );
                                }
                            }
                            current_task.stack.truncate(frame.base_offset);
                            current_task.stack.push(result);
                        }

                        OpCode::JumpIfFalse(target_ip) => {
                            let condition = pop!();
                            if let Value::Bool(false) = condition {
                                current_task.frames[frame_idx].ip = target_ip;
                            }
                        }
                        OpCode::Jump(target_ip) => {
                            current_task.frames[frame_idx].ip = target_ip;
                        }
                        OpCode::Equal => {
                            let b = pop!();
                            let a = pop!();
                            current_task.stack.push(Value::Bool(a == b));
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
                            current_task.stack.push(Value::Bool(res));
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
                            current_task.stack.push(Value::Bool(res));
                        }
                        OpCode::LessEqual => {
                            let b = pop!();
                            let a = pop!();
                            let res = match (a, b) {
                                (Value::Int(x), Value::Int(y)) => x <= y,
                                (Value::Float(x), Value::Float(y)) => x <= y,
                                (Value::Int(x), Value::Float(y)) => (x as f64) <= y,
                                (Value::Float(x), Value::Int(y)) => x <= (y as f64),
                                _ => runtime_error!("Invalid types for '<=' operation"),
                            };
                            current_task.stack.push(Value::Bool(res));
                        }
                        OpCode::GreaterEqual => {
                            let b = pop!();
                            let a = pop!();
                            let res = match (a, b) {
                                (Value::Int(x), Value::Int(y)) => x >= y,
                                (Value::Float(x), Value::Float(y)) => x >= y,
                                (Value::Int(x), Value::Float(y)) => (x as f64) >= y,
                                (Value::Float(x), Value::Int(y)) => x >= (y as f64),
                                _ => runtime_error!("Invalid types for '>=' operation"),
                            };
                            current_task.stack.push(Value::Bool(res));
                        }
                        OpCode::And => {
                            let b = pop!();
                            let a = pop!();
                            if let (Value::Bool(x), Value::Bool(y)) = (a, b) {
                                current_task.stack.push(Value::Bool(x && y));
                            } else {
                                runtime_error!("'and' expects booleans");
                            }
                        }
                        OpCode::Or => {
                            let b = pop!();
                            let a = pop!();
                            if let (Value::Bool(x), Value::Bool(y)) = (a, b) {
                                current_task.stack.push(Value::Bool(x || y));
                            } else {
                                runtime_error!("'or' expects booleans");
                            }
                        }
                        OpCode::Not => {
                            let a = pop!();
                            if let Value::Bool(x) = a {
                                current_task.stack.push(Value::Bool(!x));
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
                                        let offset = current_task.frames[frame_idx].stack_offset;
                                        let val = current_task.stack[offset + idx].clone();
                                        let upvalue_id = self.heap.alloc(heap::Obj::Upvalue(val));
                                        captured.push(upvalue_id);
                                    }
                                    UpvalueLoc::Upvalue(idx) => {
                                        let current_closure_id =
                                            current_task.frames[frame_idx].closure_id;
                                        if let Ok(heap::Obj::Closure(_, upvs)) =
                                            self.heap.get(current_closure_id)
                                        {
                                            captured.push(upvs[idx]);
                                        }
                                    }
                                }
                            }
                            let closure_id = self.heap.alloc(heap::Obj::Closure(func, captured));
                            current_task.stack.push(Value::ObjRef(closure_id));
                        }
                        OpCode::GetUpvalue(idx) => {
                            let closure_id = current_task.frames[frame_idx].closure_id;
                            if let Ok(heap::Obj::Closure(_, upvs)) = self.heap.get(closure_id) {
                                let upvalue_id = upvs[idx];
                                if let Ok(heap::Obj::Upvalue(val)) = self.heap.get(upvalue_id) {
                                    current_task.stack.push(val.clone());
                                }
                            }
                        }
                        OpCode::SetUpvalue(idx) => {
                            let val = pop!();
                            let closure_id = current_task.frames[frame_idx].closure_id;
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
                            current_task.stack.push(Value::ObjRef(id));
                        }
                        OpCode::AddMethod(name) => {
                            let method = pop!();
                            let struct_val = current_task.stack.last().unwrap().clone();
                            if let Value::ObjRef(id) = struct_val {
                                if let Ok(heap::Obj::StructDef { methods, .. }) =
                                    self.heap.get_mut(id)
                                {
                                    methods.insert(name, method);
                                }
                            }
                        }
                        OpCode::BuildInterface(methods) => {
                            let id = self.heap.alloc(heap::Obj::Interface(methods));
                            current_task.stack.push(Value::ObjRef(id));
                        }
                        OpCode::CheckIs => {
                            let right = pop!();
                            let left = pop!();
                            let is_match = self.is_type_match(&left, &right);
                            current_task.stack.push(Value::Bool(is_match));
                        }
                        OpCode::Assert(msg) => {
                            let cond = pop!();
                            if let Value::Bool(false) = cond {
                                runtime_error!("{}", msg);
                            }
                        }
                        OpCode::BuildTuple(len) => {
                            let mut elements = Vec::with_capacity(len);
                            for _ in 0..len {
                                elements.push(pop!());
                            }
                            elements.reverse();
                            current_task.stack.push(Value::Tuple(Rc::new(elements)));
                        }
                        OpCode::UnpackTuple(expected_len) => {
                            let obj = pop!();
                            if let Value::Tuple(elements) = obj {
                                if elements.len() != expected_len {
                                    runtime_error!(
                                        "Cannot unpack tuple of length {} into {} variables",
                                        elements.len(),
                                        expected_len
                                    );
                                }
                                for el in elements.iter() {
                                    current_task.stack.push(el.clone());
                                }
                            } else {
                                runtime_error!("Cannot unpack non-tuple value");
                            }
                        }
                        OpCode::Spawn(arg_count) => {
                            let mut args = Vec::with_capacity(arg_count);
                            for _ in 0..arg_count {
                                args.push(pop!());
                            }
                            args.reverse();
                            let callee = pop!();

                            match callee {
                                Value::ObjRef(id) => {
                                    if let Ok(heap::Obj::Closure(func, _)) = self.heap.get(id) {
                                        if func.arity != arg_count {
                                            runtime_error!(
                                                "Spawned function expects {} args",
                                                func.arity
                                            );
                                        }
                                        let mut new_task = Task {
                                            stack: Vec::new(),
                                            frames: Vec::new(),
                                            state: TaskState::Runnable,
                                        };
                                        new_task.stack.push(Value::ObjRef(id));
                                        for arg in &args {
                                            new_task.stack.push(arg.clone());
                                        }
                                        let new_frame = CallFrame {
                                            closure_id: id,
                                            function: func.clone(),
                                            ip: 0,
                                            stack_offset: 1,
                                            base_offset: 0,
                                        };
                                        new_task.frames.push(new_frame);
                                        self.tasks.push_back(new_task);
                                    } else {
                                        runtime_error!("Can only spawn functions");
                                    }
                                }
                                Value::Native(native_fn) => match native_fn(args, self) {
                                    crate::value::NativeResult::Return(_) => {}
                                    crate::value::NativeResult::SuspendSleep(secs) => {
                                        let wake_time =
                                            SystemTime::now() + Duration::from_secs_f64(secs);
                                        let new_task = Task {
                                            stack: Vec::new(),
                                            frames: Vec::new(),
                                            state: TaskState::Sleeping(wake_time),
                                        };
                                        self.tasks.push_back(new_task);
                                    }
                                    crate::value::NativeResult::SuspendIO(token) => {
                                        let new_task = Task {
                                            stack: Vec::new(),
                                            frames: Vec::new(),
                                            state: TaskState::WaitingIO(token),
                                        };
                                        self.tasks.push_back(new_task);
                                    }
                                },
                                _ => {
                                    runtime_error!("Can only spawn functions");
                                }
                            }
                        }
                        OpCode::ChanRecv => {
                            let chan_val = pop!();
                            if let Value::ObjRef(id) = chan_val {
                                let mut has_msg = false;
                                let mut msg = Value::Nil;

                                if let Ok(crate::heap::Obj::Channel(queue)) = self.heap.get_mut(id)
                                {
                                    if let Some(m) = queue.pop_front() {
                                        has_msg = true;
                                        msg = m;
                                    }
                                } else {
                                    runtime_error!("Attempt to read from a non-channel");
                                }

                                if has_msg {
                                    current_task.stack.push(msg);
                                    for task in self.tasks.iter_mut() {
                                        if matches!(task.state, TaskState::Waiting) {
                                            task.state = TaskState::Runnable;
                                        }
                                    }
                                } else {
                                    current_task.stack.push(chan_val);
                                    current_task.frames[frame_idx].ip -= 1;
                                    current_task.state = TaskState::Waiting;
                                    break;
                                }
                            } else {
                                runtime_error!("Attempt to read from a non-channel");
                            }
                        }
                        OpCode::ChanSend => {
                            let val = pop!();
                            let chan_val = pop!();

                            if let Value::ObjRef(id) = chan_val {
                                let is_full = if let Ok(crate::heap::Obj::Channel(queue)) =
                                    self.heap.get(id)
                                {
                                    queue.len() >= queue.capacity()
                                } else {
                                    runtime_error!("Cannot send to a non-channel");
                                };

                                if is_full {
                                    current_task.stack.push(chan_val);
                                    current_task.stack.push(val);
                                    current_task.frames[frame_idx].ip -= 1;
                                    current_task.state = TaskState::Waiting;
                                    break;
                                } else {
                                    if let Ok(crate::heap::Obj::Channel(queue)) =
                                        self.heap.get_mut(id)
                                    {
                                        queue.push_back(val);
                                    }

                                    current_task.stack.push(Value::Bool(true));

                                    for task in self.tasks.iter_mut() {
                                        if matches!(task.state, TaskState::Waiting) {
                                            task.state = TaskState::Runnable;
                                        }
                                    }
                                }
                            } else {
                                runtime_error!("Cannot send to a non-channel");
                            }
                        }
                        OpCode::IterNext(state_idx) => {
                            let offset = current_task.frames[frame_idx].stack_offset;
                            let collection_idx = offset + state_idx - 1;
                            let counter_idx = offset + state_idx;
                            
                            let collection = current_task.stack[collection_idx].clone();
                            let state_val = current_task.stack[counter_idx].clone();

                            if let Value::Int(current_idx) = state_val {
                                match collection {
                                    Value::ObjRef(id) if matches!(self.heap.get(id), Ok(crate::heap::Obj::List(_))) => {
                                        if let Ok(crate::heap::Obj::List(list)) = self.heap.get(id) {
                                            if current_idx < list.len() as i64 {
                                                current_task.stack[counter_idx] = Value::Int(current_idx + 1);
                                                current_task.stack.push(list[current_idx as usize].clone());
                                                current_task.stack.push(Value::Bool(true));
                                            } else {
                                                current_task.stack.push(Value::Nil); // Фейковый элемент
                                                current_task.stack.push(Value::Bool(false)); // Флаг выхода
                                            }
                                        }
                                    }
                                    
                                    Value::ObjRef(id) if matches!(self.heap.get(id), Ok(crate::heap::Obj::Map(_))) => {
                                        if let Ok(crate::heap::Obj::Map(map)) = self.heap.get(id) {
                                            let keys: Vec<&String> = map.keys().collect();
                                            if current_idx < keys.len() as i64 {
                                                current_task.stack[counter_idx] = Value::Int(current_idx + 1);
                                                current_task.stack.push(Value::Str(std::rc::Rc::new(keys[current_idx as usize].clone())));
                                                current_task.stack.push(Value::Bool(true));
                                            } else {
                                                current_task.stack.push(Value::Nil);
                                                current_task.stack.push(Value::Bool(false));
                                            }
                                        }
                                    }

                                    Value::Range(start, end, step) => {
                                        let current_val = start + (current_idx * step);
                                        
                                        let has_next = if step > 0 { current_val < end } else { current_val > end };

                                        if has_next {
                                            current_task.stack[counter_idx] = Value::Int(current_idx + 1);
                                            current_task.stack.push(Value::Int(current_val));
                                            current_task.stack.push(Value::Bool(true));
                                        } else {
                                            current_task.stack.push(Value::Nil);
                                            current_task.stack.push(Value::Bool(false));
                                        }
                                    }
                                    
                                    Value::Str(s) => {
                                        if current_idx < s.len() as i64 {
                                            current_task.stack[counter_idx] = Value::Int(current_idx + 1);
                                            let ch = s.chars().nth(current_idx as usize).unwrap().to_string();
                                            current_task.stack.push(Value::Str(std::rc::Rc::new(ch)));
                                            current_task.stack.push(Value::Bool(true));
                                        } else {
                                            current_task.stack.push(Value::Nil);
                                            current_task.stack.push(Value::Bool(false));
                                        }
                                    }

                                    _ => runtime_error!("TypeError: object is not iterable"),
                                }
                            } else {
                                unreachable!("Iterator state is corrupted");
                            }
                        }
                    }

                    fuel -= 1;
                }

                if !current_task.frames.is_empty() {
                    self.tasks.push_back(current_task);
                }
            }

            if !has_runnable {
                if let Some(wake_time) = next_wake_time {
                    let timeout = wake_time
                        .duration_since(SystemTime::now())
                        .unwrap_or(Duration::ZERO);
                    self.poll.poll(&mut events, Some(timeout)).unwrap();
                } else {
                    self.poll.poll(&mut events, None).unwrap();
                }

                for event in events.iter() {
                    let token = event.token();
                    for task in self.tasks.iter_mut() {
                        if let TaskState::WaitingIO(wait_token) = task.state {
                            if wait_token == token {
                                task.state = TaskState::Runnable;
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }
    fn is_type_match(&self, obj: &Value, type_val: &Value) -> bool {
        if let Value::Str(type_name) = type_val {
            match (obj, type_name.as_str()) {
                (Value::Int(_), "int") => true,
                (Value::Float(_), "float") => true,
                (Value::Str(_), "str") => true,
                (Value::Bool(_), "bool") => true,
                (Value::ObjRef(id), "list") => {
                    matches!(self.heap.get(*id), Ok(crate::heap::Obj::List(_)))
                }
                (Value::ObjRef(id), "map") => {
                    matches!(self.heap.get(*id), Ok(crate::heap::Obj::Map(_)))
                }
                (Value::ObjRef(id), "func") => {
                    matches!(self.heap.get(*id), Ok(crate::heap::Obj::Closure(_, _)))
                }
                (Value::Tuple(_), "tuple") => true,

                (Value::Tuple(elements), t_str)
                    if t_str.starts_with('(') && t_str.ends_with(')') =>
                {
                    let inner = &t_str[1..t_str.len() - 1];
                    if inner.trim().is_empty() {
                        return elements.is_empty();
                    }

                    let type_parts: Vec<&str> = inner.split(',').map(|s| s.trim()).collect();
                    if elements.len() != type_parts.len() {
                        return false;
                    }

                    for (el, t_part) in elements.iter().zip(type_parts.iter()) {
                        let t_val = self
                            .globals
                            .get(*t_part)
                            .cloned()
                            .unwrap_or_else(|| Value::Str(Rc::new(t_part.to_string())));
                        if !self.is_type_match(el, &t_val) {
                            return false;
                        }
                    }
                    true
                }
                _ => false,
            }
        } else if let Value::ObjRef(right_id) = type_val {
            if let Value::ObjRef(left_id) = obj {
                if let Ok(crate::heap::Obj::Instance { struct_id, .. }) = self.heap.get(*left_id) {
                    if right_id == struct_id {
                        return true;
                    } else if let Ok(crate::heap::Obj::Interface(required_methods)) =
                        self.heap.get(*right_id)
                    {
                        if let Ok(crate::heap::Obj::StructDef { methods, .. }) =
                            self.heap.get(*struct_id)
                        {
                            return required_methods.iter().all(|m| methods.contains_key(m));
                        }
                    }
                }
            }
            false
        } else {
            false
        }
    }
}
