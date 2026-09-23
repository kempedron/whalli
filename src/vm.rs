use crate::{
    heap,
    opcode::{OpCode, UpvalueLoc},
    value::{FunctionObj, Value},
};
use crossbeam_deque::{Injector, Steal, Stealer, Worker};
use parking_lot::{Condvar, Mutex, RwLock};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant, SystemTime},
};

use mio::net::{TcpListener, TcpStream};
use mio::{Events, Poll, Token};

#[derive(Clone)]
pub enum DeferredCall {
    Call(Value, Vec<Value>),
    MethodCall(Value, String, Vec<Value>),
}

#[derive(Clone)]
pub struct CallFrame {
    pub closure_id: usize,
    pub function: Arc<FunctionObj>,
    pub ip: usize,
    pub stack_offset: usize,
    pub base_offset: usize,
    pub defers: Vec<DeferredCall>,
}

#[derive(Debug, Clone)]
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
    pub handle_id: Option<usize>,
    pub is_main: bool,
}

struct SleepingTask {
    wake_time: Instant,
    task: Task,
}

pub struct ChannelTimer {
    pub fire_time: Instant,
    pub chan_id: usize,
}

pub struct NetworkState {
    pub poll: Mutex<Poll>,
    pub listeners: RwLock<HashMap<usize, TcpListener>>,
    pub streams: RwLock<HashMap<usize, TcpStream>>,
    pub next_token: AtomicUsize,
}

struct SharedRuntime {
    heap: heap::Heap,
    globals: RwLock<HashMap<String, Value>>,
    modules: RwLock<HashMap<String, Value>>,
    imported_files: Mutex<HashSet<PathBuf>>,

    injector: Injector<Task>,
    condvar: Condvar,
    condvar_mutex: Mutex<()>,

    sleeping_tasks: Mutex<Vec<SleepingTask>>,
    channel_timers: Arc<Mutex<Vec<ChannelTimer>>>,
    waiting_io_tasks: Mutex<Vec<Task>>,

    net: Arc<NetworkState>,

    active_tasks: AtomicUsize,
    fatal_error: Mutex<Option<RuntimeError>>,
    shutdown: AtomicBool,

    // GC coordination
    gc_threshold: AtomicUsize,
    gc_start_mutex: Mutex<()>,
}

pub struct VM {
    main_task: Option<Task>,
    pub globals: HashMap<String, Value>,
    pub modules: HashMap<String, Value>,
    pub heap: heap::Heap,
    pub current_line: usize,
    pub gc_threshold: usize,
    pub imported_files: HashSet<PathBuf>,

    pub net: Arc<NetworkState>,
    pub channel_timers: Arc<Mutex<Vec<ChannelTimer>>>,
}

fn mark_value(heap: &heap::Heap, val: &Value) {
    match val {
        Value::ObjRef(id) => heap.mark(*id),
        Value::Tuple(elements) => {
            for elem in elements.iter() {
                mark_value(heap, elem);
            }
        }
        _ => {}
    }
}

fn trigger_gc(current_task: &Task, shared: &SharedRuntime) {
    let _guard = shared.gc_start_mutex.lock();
    if shared.heap.live_count() < shared.gc_threshold.load(Ordering::Relaxed) {
        return;
    }

    // Mark from current_task
    for val in &current_task.stack {
        mark_value(&shared.heap, val);
    }
    for frame in &current_task.frames {
        shared.heap.mark(frame.closure_id);
    }

    // Mark from run_queue / injector
    // (Note: Heap mark traces objects reachable from memory, and unreferenced items are cleaned up)

    // Mark from sleeping and io tasks
    {
        let sleepers = shared.sleeping_tasks.lock();
        for s in sleepers.iter() {
            for val in &s.task.stack {
                mark_value(&shared.heap, val);
            }
            for frame in &s.task.frames {
                shared.heap.mark(frame.closure_id);
            }
        }
    }
    {
        let io_tasks = shared.waiting_io_tasks.lock();
        for task in io_tasks.iter() {
            for val in &task.stack {
                mark_value(&shared.heap, val);
            }
            for frame in &task.frames {
                shared.heap.mark(frame.closure_id);
            }
        }
    }

    // Mark from globals and modules
    {
        let globals = shared.globals.read();
        for val in globals.values() {
            mark_value(&shared.heap, val);
        }
    }
    {
        let modules = shared.modules.read();
        for val in modules.values() {
            mark_value(&shared.heap, val);
        }
    }

    // Sweep
    shared.heap.sweep();
    let live = shared.heap.live_count();
    shared.gc_threshold.store(std::cmp::max(live * 2, 256), Ordering::Relaxed);
}

fn is_type_match(heap: &heap::Heap, globals: &HashMap<String, Value>, obj: &Value, type_val: &Value) -> bool {
    let type_name_opt = match type_val {
        Value::Type(t) => Some(t.as_str()),
        Value::Str(s) => Some(s.as_str()),
        _ => None,
    };

    if let Some(type_name) = type_name_opt {
        match (obj, type_name) {
            (Value::Int(_), "int") => true,
            (Value::Float(_), "float") => true,
            (Value::Str(_), "str") => true,
            (Value::Bool(_), "bool") => true,
            (Value::ObjRef(id), "list") => {
                heap.with_read(*id, |o| matches!(o, crate::heap::Obj::List(_))).unwrap_or(false)
            }
            (Value::ObjRef(id), "map") => {
                heap.with_read(*id, |o| matches!(o, crate::heap::Obj::Map(_))).unwrap_or(false)
            }
            (Value::ObjRef(id), "func") => {
                heap.with_read(*id, |o| matches!(o, crate::heap::Obj::Closure(_, _))).unwrap_or(false)
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
                    let t_val = globals
                        .get(*t_part)
                        .cloned()
                        .unwrap_or_else(|| Value::Str(Arc::new(t_part.to_string())));
                    if !is_type_match(heap, globals, el, &t_val) {
                        return false;
                    }
                }
                true
            }
            _ => false,
        }
    } else if let Value::ObjRef(right_id) = type_val {
        if let Value::ObjRef(left_id) = obj {
            let left_struct_id = heap.with_read(*left_id, |o| {
                if let crate::heap::Obj::Instance { struct_id, .. } = o {
                    Some(*struct_id)
                } else {
                    None
                }
            }).ok().flatten();

            if let Some(struct_id) = left_struct_id {
                if *right_id == struct_id {
                    return true;
                } else {
                    let req_methods = heap.with_read(*right_id, |o| {
                        if let crate::heap::Obj::Interface(req) = o {
                            Some(req.clone())
                        } else {
                            None
                        }
                    }).ok().flatten();

                    if let Some(required) = req_methods {
                        let has_all = heap.with_read(struct_id, |o| {
                            if let crate::heap::Obj::StructDef { methods, .. } = o {
                                required.iter().all(|m| methods.contains_key(m))
                            } else {
                                false
                            }
                        }).unwrap_or(false);
                        return has_all;
                    }
                }
            }
        }
        false
    } else {
        false
    }
}

impl VM {
    pub fn new(instructions: Vec<OpCode>) -> Self {
        let main_func = Arc::new(FunctionObj {
            name: "main".to_string(),
            arity: 0,
            chunk: instructions,
            param_types: vec![],
            return_type: None,
        });

        let heap = heap::Heap::new();
        let main_closure_id = heap.alloc(heap::Obj::Closure(main_func.clone(), vec![]));

        let initial_frame = CallFrame {
            closure_id: main_closure_id,
            function: main_func,
            ip: 0,
            stack_offset: 0,
            base_offset: 0,
            defers: Vec::new(),
        };

        let main_task = Task {
            stack: Vec::new(),
            frames: vec![initial_frame],
            state: TaskState::Runnable,
            handle_id: None,
            is_main: true,
        };

        let net_state = Arc::new(NetworkState {
            poll: Mutex::new(Poll::new().unwrap()),
            listeners: RwLock::new(HashMap::new()),
            streams: RwLock::new(HashMap::new()),
            next_token: AtomicUsize::new(1),
        });

        let mut vm = VM {
            main_task: Some(main_task),
            globals: HashMap::new(),
            modules: HashMap::new(),
            current_line: 1,
            heap,
            gc_threshold: 256,
            imported_files: HashSet::new(),
            net: net_state,
            channel_timers: Arc::new(Mutex::new(Vec::new())),
        };

        let (mut globals, modules) = crate::stdlib::register_natives(&mut vm);
        let builtins = ["int", "float", "str", "bool", "list", "map", "func", "chan"];
        for builtin in builtins {
            globals.insert(
                builtin.to_string(),
                Value::Type(builtin.to_string()),
            );
        }

        vm.globals = globals;
        vm.modules = modules;
        vm
    }

    pub fn run(&mut self) -> Result<(), RuntimeError> {
        let num_threads = std::env::var("WHALLI_NUM_THREADS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(|| {
                std::thread::available_parallelism()
                    .map(|n| n.get())
                    .unwrap_or(4)
            })
            .max(1);

        let mut run_q = VecDeque::new();
        if let Some(t) = self.main_task.take() {
            run_q.push_back(t);
        }

        let shared = Arc::new(SharedRuntime {
            heap: self.heap.clone(),
            globals: RwLock::new(std::mem::take(&mut self.globals)),
            modules: RwLock::new(std::mem::take(&mut self.modules)),
            imported_files: Mutex::new(std::mem::take(&mut self.imported_files)),
            injector: Injector::new(),
            condvar: Condvar::new(),
            condvar_mutex: Mutex::new(()),
            sleeping_tasks: Mutex::new(Vec::new()),
            channel_timers: Arc::clone(&self.channel_timers),
            waiting_io_tasks: Mutex::new(Vec::new()),
            net: Arc::clone(&self.net),
            active_tasks: AtomicUsize::new(1),
            fatal_error: Mutex::new(None),
            shutdown: AtomicBool::new(false),
            gc_threshold: AtomicUsize::new(self.gc_threshold),
            gc_start_mutex: Mutex::new(()),
        });

        for t in run_q {
            shared.injector.push(t);
        }

        // Spawn timer/IO thread
        let io_shared = Arc::clone(&shared);
        let io_handle = thread::spawn(move || {
            let mut events = Events::with_capacity(128);
            while !io_shared.shutdown.load(Ordering::Relaxed) {
                if io_shared.active_tasks.load(Ordering::Relaxed) == 0 {
                    break;
                }

                let now = Instant::now();
                let mut ready_tasks = Vec::new();
                let mut next_timeout = Duration::from_millis(50);

                {
                    let mut sleepers = io_shared.sleeping_tasks.lock();
                    let mut i = 0;
                    while i < sleepers.len() {
                        if sleepers[i].wake_time <= now {
                            let item = sleepers.remove(i);
                            ready_tasks.push(item.task);
                        } else {
                            let diff = sleepers[i].wake_time - now;
                            if diff < next_timeout {
                                next_timeout = diff;
                            }
                            i += 1;
                        }
                    }
                }

                // Check channel timers (time.after)
                {
                    let mut timers = io_shared.channel_timers.lock();
                    let mut i = 0;
                    let now_secs = SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs_f64())
                        .unwrap_or(0.0);

                    while i < timers.len() {
                        if timers[i].fire_time <= now {
                            let item = timers.remove(i);
                            let _ = io_shared.heap.with_write(item.chan_id, |o| {
                                if let heap::Obj::Channel { queue, closed, .. } = o {
                                    if !*closed {
                                        queue.push_back(Value::Float(now_secs));
                                    }
                                }
                            });
                            io_shared.condvar.notify_all();
                        } else {
                            let diff = timers[i].fire_time - now;
                            if diff < next_timeout {
                                next_timeout = diff;
                            }
                            i += 1;
                        }
                    }
                }

                // If timers or sleepers are pending, keep next_timeout bounded
                let has_pending = !io_shared.sleeping_tasks.lock().is_empty() || !io_shared.channel_timers.lock().is_empty();
                if has_pending {
                    next_timeout = next_timeout.min(Duration::from_millis(5));
                }

                if !ready_tasks.is_empty() {
                    for t in ready_tasks {
                        io_shared.injector.push(t);
                    }
                    io_shared.condvar.notify_all();
                }

                {
                    let mut poller = io_shared.net.poll.lock();
                    let _ = poller.poll(&mut events, Some(next_timeout));
                }

                if !events.is_empty() {
                    let mut io_tasks = io_shared.waiting_io_tasks.lock();
                    let mut woken = Vec::new();
                    for event in events.iter() {
                        let tok = event.token();
                        let mut i = 0;
                        while i < io_tasks.len() {
                            if let TaskState::WaitingIO(wait_tok) = io_tasks[i].state {
                                if wait_tok == tok {
                                    let mut t = io_tasks.remove(i);
                                    t.state = TaskState::Runnable;
                                    woken.push(t);
                                    continue;
                                }
                            }
                            i += 1;
                        }
                    }
                    if !woken.is_empty() {
                        for t in woken {
                            io_shared.injector.push(t);
                        }
                        io_shared.condvar.notify_all();
                    }
                }
            }
        });

        // Initialize Work-Stealing workers & stealers
        let mut local_workers = Vec::with_capacity(num_threads);
        let mut stealers = Vec::with_capacity(num_threads);
        for _ in 0..num_threads {
            let w = Worker::new_fifo();
            stealers.push(w.stealer());
            local_workers.push(w);
        }
        let stealers = Arc::new(stealers);

        // Spawn worker threads
        let mut workers = Vec::with_capacity(num_threads);
        for worker_idx in 0..num_threads {
            let shared_worker = Arc::clone(&shared);
            let local_w = local_workers.pop().unwrap();
            let stealers_clone = Arc::clone(&stealers);
            let handle = thread::spawn(move || {
                run_worker_loop(shared_worker, local_w, worker_idx, stealers_clone);
            });
            workers.push(handle);
        }

        for w in workers {
            let _ = w.join();
        }

        {
            let globals = shared.globals.read();
            for val in globals.values() {
                mark_value(&shared.heap, val);
            }
            let modules = shared.modules.read();
            for val in modules.values() {
                mark_value(&shared.heap, val);
            }
            shared.heap.sweep();
        }

        shared.shutdown.store(true, Ordering::Relaxed);
        let _ = io_handle.join();

        let fatal = shared.fatal_error.lock().clone();
        let globals = shared.globals.read().clone();
        let modules = shared.modules.read().clone();
        let imported = shared.imported_files.lock().clone();

        self.globals = globals;
        self.modules = modules;
        self.imported_files = imported;
        self.gc_threshold = shared.gc_threshold.load(Ordering::Relaxed);

        if let Ok(arc_heap) = Arc::try_unwrap(shared) {
            self.heap = arc_heap.heap;
        } else {
            self.heap = self.heap.clone();
        }

        if let Some(err) = fatal {
            return Err(err);
        }

        Ok(())
    }
}

enum SliceResult {
    Completed,
    Exhausted(Task),
    Parked(Task),
    Sleep(Task, Instant),
    WaitIO(Task),
    Error,
}

fn execute_sync_deferred(deferred: &DeferredCall, shared: &SharedRuntime, current_line: usize) {
    match deferred {
        DeferredCall::Call(callee, args) => {
            if let Value::Native(native_fn) = callee {
                let mut temp_vm = VM {
                    main_task: None,
                    globals: shared.globals.read().clone(),
                    modules: shared.modules.read().clone(),
                    heap: shared.heap.clone(),
                    current_line,
                    gc_threshold: shared.gc_threshold.load(Ordering::Relaxed),
                    imported_files: HashSet::new(),
                    net: Arc::clone(&shared.net),
                    channel_timers: Arc::new(Mutex::new(Vec::new())),
                };
                let _ = native_fn(args.clone(), &mut temp_vm);
            }
        }
        DeferredCall::MethodCall(obj, method_name, args) => {
            if let Value::ObjRef(id) = obj {
                let _ = shared.heap.with_write(*id, |obj_ref| {
                    match obj_ref {
                        crate::heap::Obj::Mutex { locked, .. } => {
                            if method_name == "unlock" {
                                *locked = false;
                                shared.condvar.notify_all();
                            }
                        }
                        crate::heap::Obj::WaitGroup { count, .. } => {
                            if method_name == "done" {
                                *count = count.saturating_sub(1);
                                if *count == 0 {
                                    shared.condvar.notify_all();
                                }
                            }
                        }
                        crate::heap::Obj::Channel { closed, .. } => {
                            if method_name == "close" {
                                *closed = true;
                                shared.condvar.notify_all();
                            }
                        }
                        _ => {}
                    }
                });
            }
            let _ = obj.call_method(method_name, args.clone(), &shared.heap);
        }
    }
}

fn find_task(
    local: &Worker<Task>,
    injector: &Injector<Task>,
    stealers: &[Stealer<Task>],
    worker_idx: usize,
) -> Option<Task> {
    if let Some(t) = local.pop() {
        return Some(t);
    }

    // Steal a batch from global injector into local queue
    loop {
        match injector.steal_batch_and_pop(local) {
            Steal::Success(t) => return Some(t),
            Steal::Empty => break,
            Steal::Retry => {}
        }
    }

    // 3. Work-Stealing: steal from peers
    let num_stealers = stealers.len();
    if num_stealers > 1 {
        for i in 1..num_stealers {
            let victim = (worker_idx + i) % num_stealers;
            loop {
                match stealers[victim].steal_batch_and_pop(local) {
                    Steal::Success(t) => return Some(t),
                    Steal::Empty => break,
                    Steal::Retry => {}
                }
            }
        }
    }

    None
}

fn run_worker_loop(
    shared: Arc<SharedRuntime>,
    local_worker: Worker<Task>,
    worker_idx: usize,
    stealers: Arc<Vec<Stealer<Task>>>,
) {
    let mut idle_spins = 0;

    loop {
        if shared.shutdown.load(Ordering::Relaxed) {
            break;
        }

        // Deadlock / completion check
        if shared.active_tasks.load(Ordering::Relaxed) == 0 {
            shared.condvar.notify_all();
            break;
        }

        let task_opt = find_task(&local_worker, &shared.injector, &stealers, worker_idx);

        let current_task = match task_opt {
            Some(t) => {
                idle_spins = 0;
                t
            }
            None => {
                let has_sleepers = !shared.sleeping_tasks.lock().is_empty();
                let has_timers = !shared.channel_timers.lock().is_empty();
                let has_io = !shared.waiting_io_tasks.lock().is_empty();

                if shared.active_tasks.load(Ordering::Relaxed) == 0 {
                    shared.condvar.notify_all();
                    break;
                }

                if !has_sleepers && !has_timers && !has_io {
                    idle_spins += 1;
                    if idle_spins > 50 {
                        let mut err_guard = shared.fatal_error.lock();
                        if err_guard.is_none() {
                            *err_guard = Some(RuntimeError {
                                message: "fatal error: all woroutines are asleep - deadlock!".to_string(),
                                line: 1,
                            });
                        }
                        shared.shutdown.store(true, Ordering::Relaxed);
                        shared.condvar.notify_all();
                        break;
                    }
                } else {
                    idle_spins = 0;
                }

                let mut guard = shared.condvar_mutex.lock();
                shared.condvar.wait_for(&mut guard, Duration::from_millis(5));
                continue;
            }
        };

        let res = execute_task_slice(current_task, &shared);
        match res {
            SliceResult::Completed => {
                shared.active_tasks.fetch_sub(1, Ordering::Relaxed);
                shared.condvar.notify_all();
            }
            SliceResult::Exhausted(t) => {
                local_worker.push(t);
                shared.condvar.notify_one();
            }
            SliceResult::Parked(t) => {
                shared.injector.push(t);
                shared.condvar.notify_all();
            }
            SliceResult::Sleep(t, wake_time) => {
                shared.sleeping_tasks.lock().push(SleepingTask {
                    wake_time,
                    task: t,
                });
            }
            SliceResult::WaitIO(t) => {
                shared.waiting_io_tasks.lock().push(t);
            }
            SliceResult::Error => {
                break;
            }
        }
    }
}

fn execute_task_slice(mut current_task: Task, shared: &Arc<SharedRuntime>) -> SliceResult {
    let mut fuel = 2000;
    let mut current_line = 1;

    macro_rules! pop {
        () => {
            current_task.stack.pop().unwrap_or(Value::Nil)
        };
    }

    macro_rules! fail {
        ($msg:expr) => {{
            let err_msg = $msg.to_string();
            // If background task, isolate error and run defers
            if !current_task.is_main {
                while let Some(mut frame) = current_task.frames.pop() {
                    while let Some(deferred) = frame.defers.pop() {
                        execute_sync_deferred(&deferred, shared, current_line);
                    }
                }
                if let Some(hid) = current_task.handle_id {
                    let _ = shared.heap.with_write(hid, |o| {
                        if let heap::Obj::TaskHandle { status, .. } = o {
                            *status = heap::TaskStatus::Failed(err_msg.clone());
                        }
                    });
                    shared.condvar.notify_all();
                }
                return SliceResult::Completed;
            }

            let mut err = shared.fatal_error.lock();
            if err.is_none() {
                *err = Some(RuntimeError {
                    message: err_msg,
                    line: current_line,
                });
            }
            shared.shutdown.store(true, Ordering::Relaxed);
            shared.condvar.notify_all();
            return SliceResult::Error;
        }};
        ($fmt:expr, $($arg:expr),*) => {{
            fail!(format!($fmt, $($arg),*))
        }};
    }

    while fuel > 0 && !current_task.frames.is_empty() {
        if shared.heap.live_count() >= shared.gc_threshold.load(Ordering::Relaxed) {
            trigger_gc(&current_task, shared);
        }

        let frame_idx = current_task.frames.len() - 1;
        if current_task.frames[frame_idx].ip >= current_task.frames[frame_idx].function.chunk.len() {
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
                    (Value::Int(x), Value::Int(y)) => current_task.stack.push(Value::Int(x + y)),
                    (Value::Float(x), Value::Float(y)) => current_task.stack.push(Value::Float(x + y)),
                    (Value::Int(x), Value::Float(y)) => current_task.stack.push(Value::Float((x as f64) + y)),
                    (Value::Float(x), Value::Int(y)) => current_task.stack.push(Value::Float(x + (y as f64))),
                    (Value::Str(x), Value::Str(y)) => current_task.stack.push(Value::Str(Arc::new(format!("{}{}", x, y)))),
                    (Value::Str(x), y) => {
                        let y_str = y.stringify(&shared.heap);
                        current_task.stack.push(Value::Str(Arc::new(format!("{}{}", x, y_str))));
                    }
                    (x, Value::Str(y)) => {
                        let x_str = x.stringify(&shared.heap);
                        current_task.stack.push(Value::Str(Arc::new(format!("{}{}", x_str, y))));
                    }
                    _ => fail!("Invalid types for '+' operation"),
                }
            }
            OpCode::Sub => {
                let b = pop!();
                let a = pop!();
                match (a, b) {
                    (Value::Int(x), Value::Int(y)) => current_task.stack.push(Value::Int(x - y)),
                    (Value::Float(x), Value::Float(y)) => current_task.stack.push(Value::Float(x - y)),
                    (Value::Int(x), Value::Float(y)) => current_task.stack.push(Value::Float((x as f64) - y)),
                    (Value::Float(x), Value::Int(y)) => current_task.stack.push(Value::Float(x - (y as f64))),
                    _ => fail!("Invalid types for '-' operation"),
                }
            }
            OpCode::Mul => {
                let b = pop!();
                let a = pop!();
                match (a, b) {
                    (Value::Int(x), Value::Int(y)) => current_task.stack.push(Value::Int(x * y)),
                    (Value::Float(x), Value::Float(y)) => current_task.stack.push(Value::Float(x * y)),
                    (Value::Int(x), Value::Float(y)) => current_task.stack.push(Value::Float((x as f64) * y)),
                    (Value::Float(x), Value::Int(y)) => current_task.stack.push(Value::Float(x * (y as f64))),
                    _ => fail!("Invalid types for '*' operation"),
                }
            }
            OpCode::Div => {
                let b = pop!();
                let a = pop!();
                match (a, b) {
                    (Value::Int(x), Value::Int(y)) => {
                        if y == 0 { fail!("Division by zero"); }
                        current_task.stack.push(Value::Int(x / y));
                    }
                    (Value::Float(x), Value::Float(y)) => current_task.stack.push(Value::Float(x / y)),
                    (Value::Int(x), Value::Float(y)) => current_task.stack.push(Value::Float((x as f64) / y)),
                    (Value::Float(x), Value::Int(y)) => {
                        if y == 0 { fail!("Division by zero"); }
                        current_task.stack.push(Value::Float(x / (y as f64)));
                    }
                    _ => fail!("Invalid types for '/' operation"),
                }
            }
            OpCode::Mod => {
                let b = pop!();
                let a = pop!();
                match (a, b) {
                    (Value::Int(x), Value::Int(y)) => {
                        if y == 0 { fail!("Modulo by zero"); }
                        current_task.stack.push(Value::Int(x % y));
                    }
                    (Value::Float(x), Value::Float(y)) => current_task.stack.push(Value::Float(x % y)),
                    (Value::Int(x), Value::Float(y)) => current_task.stack.push(Value::Float((x as f64) % y)),
                    (Value::Float(x), Value::Int(y)) => {
                        if y == 0 { fail!("Modulo by zero"); }
                        current_task.stack.push(Value::Float(x % (y as f64)));
                    }
                    _ => fail!("Invalid types for '%' operation"),
                }
            }
            OpCode::StoreGlobal(name) => {
                let val = pop!();
                shared.globals.write().insert(name, val);
            }
            OpCode::LoadGlobal(name) => {
                if let Some(val) = shared.globals.read().get(&name).cloned() {
                    current_task.stack.push(val);
                } else {
                    fail!("Undefined variable '{}'", name);
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
                        let obj = match shared.heap.get(id) {
                            Ok(o) => o,
                            Err(e) => fail!("{}", e),
                        };
                        match obj {
                            heap::Obj::Closure(func, _) => {
                                if func.arity != arg_count {
                                    fail!("Function '{}' expects {} arguments, got {}", func.name, func.arity, arg_count);
                                }
                                    let new_frame = CallFrame {
                                        closure_id: id,
                                        function: func.clone(),
                                        ip: 0,
                                        stack_offset: callee_index + 1,
                                        base_offset: callee_index,
                                        defers: Vec::new(),
                                    };
                                current_task.frames.push(new_frame);
                            }
                            heap::Obj::StructDef { name, fields, .. } => {
                                if fields.len() != arg_count {
                                    fail!("Struct '{}' expects {} arguments, got {}", name, fields.len(), arg_count);
                                }
                                let mut instance_fields = HashMap::new();
                                let mut args = Vec::with_capacity(arg_count);
                                for _ in 0..arg_count {
                                    args.push(pop!());
                                }
                                args.reverse();
                                pop!();

                                for (i, field_name) in fields.iter().enumerate() {
                                    instance_fields.insert(field_name.clone(), args[i].clone());
                                }

                                let instance_id = shared.heap.alloc(heap::Obj::Instance {
                                    struct_id: id,
                                    fields: instance_fields,
                                });
                                current_task.stack.push(Value::ObjRef(instance_id));
                            }
                            _ => fail!("Attempt to call a non-callable object"),
                        }
                    }
                    Value::Native(native_fn) => {
                        let mut args = Vec::with_capacity(arg_count);
                        for _ in 0..arg_count {
                            args.push(pop!());
                        }
                        args.reverse();
                        pop!(); // callee

                        let mut temp_vm = VM {
                            main_task: None,
                            globals: shared.globals.read().clone(),
                            modules: shared.modules.read().clone(),
                            heap: shared.heap.clone(),
                            current_line,
                            gc_threshold: shared.gc_threshold.load(Ordering::Relaxed),
                            imported_files: HashSet::new(),
                            net: Arc::clone(&shared.net),
                            channel_timers: Arc::clone(&shared.channel_timers),
                        };

                        match native_fn(args.clone(), &mut temp_vm) {
                            crate::value::NativeResult::Return(val) => {
                                current_task.stack.push(val);
                            }
                            crate::value::NativeResult::SuspendSleep(secs) => {
                                let wake = Instant::now() + Duration::from_secs_f64(secs);
                                current_task.stack.push(Value::Nil);
                                return SliceResult::Sleep(current_task, wake);
                            }
                            crate::value::NativeResult::SuspendIO(token) => {
                                current_task.frames[frame_idx].ip -= 1;
                                current_task.stack.push(callee);
                                for arg in args {
                                    current_task.stack.push(arg);
                                }
                                current_task.state = TaskState::WaitingIO(token);
                                return SliceResult::WaitIO(current_task);
                            }
                        }
                    }
                    _ => fail!("Attempt to call a non-function value"),
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
                    let is_instance_or_map = shared.heap.with_read(*id, |o| {
                        match o {
                            heap::Obj::Instance { struct_id, .. } => Some((true, *struct_id)),
                            heap::Obj::Map(_) => Some((false, 0)),
                            _ => None,
                        }
                    }).ok().flatten();

                    if let Some((is_inst, struct_id)) = is_instance_or_map {
                        if is_inst {
                            let method_val = shared.heap.with_read(struct_id, |s_obj| {
                                if let heap::Obj::StructDef { methods, .. } = s_obj {
                                    methods.get(&method_name).cloned()
                                } else {
                                    None
                                }
                            }).ok().flatten();

                            if let Some(m_val) = method_val {
                                handled = true;
                                current_task.stack.push(obj.clone());
                                for arg in &args {
                                    current_task.stack.push(arg.clone());
                                }
                                let callee_index = current_task.stack.len() - arg_count - 1;
                                if let Value::ObjRef(closure_id) = m_val {
                                    if let Ok(heap::Obj::Closure(func, _)) = shared.heap.get(closure_id) {
                                        if func.arity != arg_count + 1 {
                                            fail!("Method '{}' expects {} arguments, got {}", method_name, func.arity, arg_count + 1);
                                        }
                                        let new_frame = CallFrame {
                                            closure_id,
                                            function: func.clone(),
                                            ip: 0,
                                            stack_offset: callee_index,
                                            base_offset: callee_index,
                                            defers: Vec::new(),
                                        };
                                        current_task.frames.push(new_frame);
                                    }
                                }
                            }
                        } else {
                            // Map property call
                            let map_val = shared.heap.with_read(*id, |m_obj| {
                                if let heap::Obj::Map(map) = m_obj {
                                    map.get(&method_name).cloned()
                                } else {
                                    None
                                }
                            }).ok().flatten();

                            if let Some(val) = map_val {
                                handled = true;
                                current_task.stack.push(val.clone());
                                for arg in &args {
                                    current_task.stack.push(arg.clone());
                                }
                                let callee_index = current_task.stack.len() - arg_count - 1;
                                match current_task.stack[callee_index].clone() {
                                    Value::Native(native_fn) => {
                                        let mut n_args = Vec::new();
                                        for _ in 0..arg_count {
                                            n_args.push(pop!());
                                        }
                                        n_args.reverse();
                                        let _callee = pop!();

                                            let mut temp_vm = VM {
                                                main_task: None,
                                                globals: shared.globals.read().clone(),
                                                modules: shared.modules.read().clone(),
                                                heap: shared.heap.clone(),
                                                current_line,
                                                gc_threshold: shared.gc_threshold.load(Ordering::Relaxed),
                                                imported_files: HashSet::new(),
                                                net: Arc::clone(&shared.net),
                                                channel_timers: Arc::clone(&shared.channel_timers),
                                            };

                                        match native_fn(n_args.clone(), &mut temp_vm) {
                                            crate::value::NativeResult::Return(v) => current_task.stack.push(v),
                                            crate::value::NativeResult::SuspendSleep(secs) => {
                                                let wake = Instant::now() + Duration::from_secs_f64(secs);
                                                current_task.stack.push(Value::Nil);
                                                return SliceResult::Sleep(current_task, wake);
                                            }
                                            crate::value::NativeResult::SuspendIO(token) => {
                                                current_task.frames[frame_idx].ip -= 1;
                                                current_task.stack.push(obj.clone());
                                                for arg in n_args {
                                                    current_task.stack.push(arg);
                                                }
                                                current_task.state = TaskState::WaitingIO(token);
                                                return SliceResult::WaitIO(current_task);
                                            }
                                        }
                                    }
                                    Value::ObjRef(cid) => {
                                        if let Ok(heap::Obj::Closure(func, _)) = shared.heap.get(cid) {
                                            if func.arity != arg_count {
                                                fail!("Wrong arity");
                                            }
                                                let new_frame = CallFrame {
                                                    closure_id: cid,
                                                    function: func.clone(),
                                                    ip: 0,
                                                    stack_offset: callee_index + 1,
                                                    base_offset: callee_index,
                                                    defers: Vec::new(),
                                                };
                                            current_task.frames.push(new_frame);
                                        }
                                    }
                                    _ => fail!("Property is not callable"),
                                }
                            }
                        }
                    }

                    // Concurrency primitives methods
                    if !handled {
                        let sync_result = shared.heap.with_write(*id, |obj_ref| {
                            match obj_ref {
                                crate::heap::Obj::Channel { closed, .. } => {
                                    if method_name == "close" {
                                        *closed = true;
                                        return Some(1); // Channel closed
                                    }
                                }
                                crate::heap::Obj::WaitGroup { count, .. } => {
                                    match method_name.as_str() {
                                        "add" => {
                                            let delta = match args.first() {
                                                Some(Value::Int(n)) => *n as usize,
                                                _ => 1,
                                            };
                                            *count += delta;
                                            return Some(2); // WaitGroup add
                                        }
                                        "done" => {
                                            *count = count.saturating_sub(1);
                                            if *count == 0 {
                                                return Some(3); // WaitGroup done zero
                                            }
                                            return Some(4); // WaitGroup done >0
                                        }
                                        "wait" => {
                                            if *count > 0 {
                                                return Some(5); // WaitGroup wait blocked
                                            } else {
                                                return Some(6); // WaitGroup wait immediate
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                                crate::heap::Obj::Mutex { locked, .. } => {
                                    match method_name.as_str() {
                                        "lock" => {
                                            if *locked {
                                                return Some(7); // Mutex lock blocked
                                            } else {
                                                *locked = true;
                                                return Some(8); // Mutex lock acquired
                                            }
                                        }
                                        "unlock" => {
                                            *locked = false;
                                            return Some(9); // Mutex unlocked
                                        }
                                        "try_lock" => {
                                            if *locked {
                                                return Some(10); // try_lock failed
                                            } else {
                                                *locked = true;
                                                return Some(11); // try_lock success
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                                crate::heap::Obj::TaskHandle { status, .. } => {
                                    match method_name.as_str() {
                                        "result" | "wait" => {
                                            match status {
                                                heap::TaskStatus::Running => return Some(12), // TaskHandle blocked
                                                heap::TaskStatus::Completed(_) => return Some(13), // TaskHandle completed
                                                heap::TaskStatus::Failed(_) => return Some(14), // TaskHandle failed
                                            }
                                        }
                                        "is_done" => {
                                            let done = !matches!(status, heap::TaskStatus::Running);
                                            return Some(if done { 15 } else { 16 });
                                        }
                                        "status" => {
                                            match status {
                                                heap::TaskStatus::Running => return Some(17),
                                                heap::TaskStatus::Completed(_) => return Some(18),
                                                heap::TaskStatus::Failed(_) => return Some(19),
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                                _ => {}
                            }
                            None
                        }).ok().flatten();

                        if let Some(code) = sync_result {
                            handled = true;
                            match code {
                                1 => {
                                    shared.condvar.notify_all();
                                    current_task.stack.push(Value::Nil);
                                }
                                2 | 4 => {
                                    current_task.stack.push(Value::Nil);
                                }
                                3 => {
                                    shared.condvar.notify_all();
                                    current_task.stack.push(Value::Nil);
                                }
                                5 => {
                                    current_task.stack.push(obj.clone());
                                    for arg in &args {
                                        current_task.stack.push(arg.clone());
                                    }
                                    current_task.frames[frame_idx].ip -= 1;
                                    current_task.state = TaskState::Waiting;
                                    return SliceResult::Parked(current_task);
                                }
                                6 => {
                                    current_task.stack.push(Value::Nil);
                                }
                                7 => {
                                    current_task.stack.push(obj.clone());
                                    for arg in &args {
                                        current_task.stack.push(arg.clone());
                                    }
                                    current_task.frames[frame_idx].ip -= 1;
                                    current_task.state = TaskState::Waiting;
                                    return SliceResult::Parked(current_task);
                                }
                                8 => {
                                    current_task.stack.push(Value::Nil);
                                }
                                9 => {
                                    shared.condvar.notify_all();
                                    current_task.stack.push(Value::Nil);
                                }
                                10 => {
                                    current_task.stack.push(Value::Bool(false));
                                }
                                11 => {
                                    current_task.stack.push(Value::Bool(true));
                                }
                                12 => {
                                    // TaskHandle still running: park current task to wait
                                    current_task.stack.push(obj.clone());
                                    for arg in &args {
                                        current_task.stack.push(arg.clone());
                                    }
                                    current_task.frames[frame_idx].ip -= 1;
                                    current_task.state = TaskState::Waiting;
                                    return SliceResult::Parked(current_task);
                                }
                                13 => {
                                    // TaskHandle completed: return (result, nil)
                                    let res_val = shared.heap.with_read(*id, |o| {
                                        if let heap::Obj::TaskHandle { status: heap::TaskStatus::Completed(val), .. } = o {
                                            val.clone()
                                        } else {
                                            Value::Nil
                                        }
                                    }).unwrap_or(Value::Nil);
                                    let tuple = Value::Tuple(Arc::new(vec![res_val, Value::Nil]));
                                    current_task.stack.push(tuple);
                                }
                                14 => {
                                    // TaskHandle failed: return (nil, error_str)
                                    let err_val = shared.heap.with_read(*id, |o| {
                                        if let heap::Obj::TaskHandle { status: heap::TaskStatus::Failed(msg), .. } = o {
                                            Value::Str(Arc::new(msg.clone()))
                                        } else {
                                            Value::Str(Arc::new("Unknown task error".to_string()))
                                        }
                                    }).unwrap_or_else(|_| Value::Str(Arc::new("Unknown task error".to_string())));
                                    let tuple = Value::Tuple(Arc::new(vec![Value::Nil, err_val]));
                                    current_task.stack.push(tuple);
                                }
                                15 => {
                                    current_task.stack.push(Value::Bool(true));
                                }
                                16 => {
                                    current_task.stack.push(Value::Bool(false));
                                }
                                17 => {
                                    current_task.stack.push(Value::Str(Arc::new("running".to_string())));
                                }
                                18 => {
                                    current_task.stack.push(Value::Str(Arc::new("completed".to_string())));
                                }
                                19 => {
                                    current_task.stack.push(Value::Str(Arc::new("failed".to_string())));
                                }
                                _ => {}
                            }
                        }
                    }
                }

                if !handled {
                    match obj.call_method(&method_name, args, &shared.heap) {
                        Ok(result) => current_task.stack.push(result),
                        Err(err_msg) => fail!("{}", err_msg),
                    }
                }
            }
                OpCode::BuildList(size) => {
                    let start = current_task.stack.len() - size;
                    let elements: Vec<Value> = current_task.stack.drain(start..).collect();
                    let id = shared.heap.alloc(heap::Obj::List(elements));
                    current_task.stack.push(Value::ObjRef(id));
                }
                OpCode::ListLen => {
                    let val = pop!();
                    if let Value::ObjRef(id) = val {
                        if let Ok(len) = shared.heap.with_read(id, |o| {
                            if let heap::Obj::List(l) = o {
                                Ok(l.len())
                            } else {
                                Err(())
                            }
                        }) {
                            if let Ok(l) = len {
                                current_task.stack.push(Value::Int(l as i64));
                            } else {
                                fail!("Attempt to get length of a non-list");
                            }
                        }
                    } else {
                        fail!("Attempt to get length of a non-list");
                    }
                }
                OpCode::BuildMap(size) => {
                    let mut map = HashMap::new();
                    for _ in 0..size {
                        let val = pop!();
                        let key = pop!();
                        let key_str = match key {
                            Value::Str(s) => (*s).clone(),
                            _ => fail!("Map keys must be strings"),
                        };
                        map.insert(key_str, val);
                    }
                    let id = shared.heap.alloc(heap::Obj::Map(map));
                    current_task.stack.push(Value::ObjRef(id));
                }
                OpCode::IndexGet => {
                    let index = pop!();
                    let collection = pop!();

                    match collection {
                        Value::ObjRef(id) => {
                            let res = shared.heap.with_read(id, |obj| {
                                match (obj, &index) {
                                    (crate::heap::Obj::List(list), Value::Int(idx)) => {
                                        if *idx < 0 || *idx >= list.len() as i64 {
                                            Err(format!("Index {} out of bounds", idx))
                                        } else {
                                            Ok(list[*idx as usize].clone())
                                        }
                                    }
                                    (crate::heap::Obj::Map(map), Value::Str(key)) => {
                                        Ok(map.get(&**key).unwrap_or(&Value::Nil).clone())
                                    }
                                    (crate::heap::Obj::Instance { fields, .. }, Value::Str(key)) => {
                                        Ok(fields.get(&**key).unwrap_or(&Value::Nil).clone())
                                    }
                                    _ => Err("Invalid index type for collection".to_string()),
                                }
                            });
                            match res {
                                Ok(Ok(v)) => current_task.stack.push(v),
                                Ok(Err(e)) => fail!("{}", e),
                                Err(e) => fail!("{}", e),
                            }
                        }
                        Value::Str(s) => {
                            if let Value::Int(idx) = index {
                                if idx < 0 || idx >= s.len() as i64 {
                                    fail!("String index out of bounds");
                                }
                                let ch = s.chars().nth(idx as usize).unwrap().to_string();
                                current_task.stack.push(Value::Str(Arc::new(ch)));
                            } else {
                                fail!("String index must be integer");
                            }
                        }
                        Value::Tuple(elements) => {
                            if let Value::Int(idx) = index {
                                if idx < 0 || (idx as usize) >= elements.len() {
                                    fail!("Tuple index out of bounds");
                                }
                                current_task.stack.push(elements[idx as usize].clone());
                            } else {
                                fail!("Tuple index must be an integer");
                            }
                        }
                        _ => fail!("Invalid target for reading index"),
                    }
                }
                OpCode::IndexSet => {
                    let value = pop!();
                    let index = pop!();
                    let collection = pop!();

                    match collection {
                        Value::ObjRef(id) => {
                            let res = shared.heap.with_write(id, |obj| {
                                match (obj, index) {
                                    (crate::heap::Obj::List(list), Value::Int(idx)) => {
                                        if idx < 0 || idx >= list.len() as i64 {
                                            Err(format!("Index {} out of bounds", idx))
                                        } else {
                                            list[idx as usize] = value;
                                            Ok(())
                                        }
                                    }
                                    (crate::heap::Obj::Map(map), Value::Str(key)) => {
                                        map.insert((*key).clone(), value);
                                        Ok(())
                                    }
                                    (crate::heap::Obj::Instance { fields, .. }, Value::Str(key)) => {
                                        fields.insert((*key).clone(), value);
                                        Ok(())
                                    }
                                    _ => Err("Invalid index type for collection".to_string()),
                                }
                            });
                            match res {
                                Ok(Ok(())) => {}
                                Ok(Err(e)) => fail!("{}", e),
                                Err(e) => fail!("{}", e),
                            }
                        }
                        _ => fail!("Invalid target for assignment (only lists and maps are mutable)"),
                    }
                }
                OpCode::Import(module_name) => {
                    if let Some(module_val) = shared.modules.read().get(&module_name).cloned() {
                        shared.globals.write().insert(module_name.clone(), module_val);
                    } else {
                        fail!("Module '{}' not found", module_name);
                    }
                }
                OpCode::ImportFile(path_str) => {
                    let path = Path::new(&path_str);
                    let resolved = if path.is_relative() {
                        std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")).join(path)
                    } else {
                        path.to_path_buf()
                    };

                    let canonical = match resolved.canonicalize() {
                        Ok(c) => c,
                        Err(e) => fail!("Cannot import file '{}': {}", path_str, e),
                    };

                    let mut files = shared.imported_files.lock();
                    if !files.contains(&canonical) {
                        files.insert(canonical.clone());

                        let source = match std::fs::read_to_string(&canonical) {
                            Ok(s) => s,
                            Err(e) => fail!("Cannot read import file '{}': {}", path_str, e),
                        };

                        let mut lexer = crate::lexer::Lexer::new(&source);
                        let tokens = match lexer.tokenize() {
                            Ok(t) => t,
                            Err(e) => fail!("Lexer error in import: {}", e.message),
                        };

                        let mut parser = crate::parser::Parser::new(tokens);
                        let ast = match parser.parse() {
                            Ok(a) => a,
                            Err(e) => fail!("Syntax error in import: {}", e.message),
                        };

                        let compiler = crate::compiler::Compiler::new();
                        let bytecode = compiler.compile(&ast);

                        let imported_func = Arc::new(FunctionObj {
                            name: path_str.clone(),
                            arity: 0,
                            chunk: bytecode,
                            param_types: vec![],
                            return_type: None,
                        });

                        let closure_id = shared.heap.alloc(heap::Obj::Closure(imported_func.clone(), vec![]));
                        let base_offset = current_task.stack.len();
                        current_task.frames.push(CallFrame {
                            closure_id,
                            function: imported_func,
                            ip: 0,
                            stack_offset: base_offset,
                            base_offset,
                            defers: Vec::new(),
                        });
                    }
                }
                OpCode::DeferCall(arg_count) => {
                    let mut args = Vec::with_capacity(arg_count);
                    for _ in 0..arg_count {
                        args.push(pop!());
                    }
                    args.reverse();
                    let callee = pop!();
                    current_task.frames[frame_idx].defers.push(DeferredCall::Call(callee, args));
                }
                OpCode::DeferMethodCall(method_name, arg_count) => {
                    let mut args = Vec::with_capacity(arg_count);
                    for _ in 0..arg_count {
                        args.push(pop!());
                    }
                    args.reverse();
                    let obj = pop!();
                    current_task.frames[frame_idx].defers.push(DeferredCall::MethodCall(obj, method_name, args));
                }
                OpCode::Return => {
                    if let Some(deferred) = current_task.frames[frame_idx].defers.pop() {
                        current_task.frames[frame_idx].ip -= 1;
                        match deferred {
                            DeferredCall::Call(callee, args) => match callee {
                                Value::ObjRef(id) => {
                                    if let Ok(heap::Obj::Closure(func, _)) = shared.heap.get(id) {
                                        let new_base = current_task.stack.len();
                                        current_task.stack.push(Value::ObjRef(id));
                                        for arg in args {
                                            current_task.stack.push(arg);
                                        }
                                        let new_frame = CallFrame {
                                            closure_id: id,
                                            function: func,
                                            ip: 0,
                                            stack_offset: new_base + 1,
                                            base_offset: new_base,
                                            defers: Vec::new(),
                                        };
                                        current_task.frames.push(new_frame);
                                    }
                                }
                                Value::Native(native_fn) => {
                                    let mut temp_vm = VM {
                                        main_task: None,
                                        globals: shared.globals.read().clone(),
                                        modules: shared.modules.read().clone(),
                                        heap: shared.heap.clone(),
                                        current_line,
                                        gc_threshold: shared.gc_threshold.load(Ordering::Relaxed),
                                        imported_files: HashSet::new(),
                                        net: Arc::clone(&shared.net),
                                        channel_timers: Arc::clone(&shared.channel_timers),
                                    };
                                    let _ = native_fn(args, &mut temp_vm);
                                }
                                _ => {}
                            },
                            DeferredCall::MethodCall(obj, method_name, args) => {
                                if let Value::ObjRef(id) = &obj {
                                    let _ = shared.heap.with_write(*id, |obj_ref| {
                                        match obj_ref {
                                            crate::heap::Obj::Mutex { locked, .. } => {
                                                if method_name == "unlock" {
                                                    *locked = false;
                                                    shared.condvar.notify_all();
                                                }
                                            }
                                            crate::heap::Obj::WaitGroup { count, .. } => {
                                                if method_name == "done" {
                                                    *count = count.saturating_sub(1);
                                                    if *count == 0 {
                                                        shared.condvar.notify_all();
                                                    }
                                                }
                                            }
                                            crate::heap::Obj::Channel { closed, .. } => {
                                                if method_name == "close" {
                                                    *closed = true;
                                                    shared.condvar.notify_all();
                                                }
                                            }
                                            _ => {}
                                        }
                                    });
                                }
                                let _ = obj.call_method(&method_name, args, &shared.heap);
                            }
                        }
                        continue;
                    }

                    let result = current_task.stack.pop().unwrap_or(Value::Nil);
                    let frame = current_task.frames.pop().unwrap();

                    if let Some(expected_type) = &frame.function.return_type {
                        let type_val = shared.globals.read().get(expected_type).cloned().unwrap_or_else(|| {
                            Value::Str(Arc::new(expected_type.clone()))
                        });
                        let globals = shared.globals.read();
                        if !is_type_match(&shared.heap, &globals, &result, &type_val) {
                            fail!("TypeError: function {} expected to return {}, but got {}", frame.function.name, expected_type, type_val);
                        }
                    }
                    current_task.stack.truncate(frame.base_offset);
                    current_task.stack.push(result.clone());

                    // If this was the last frame of a background task, record result in TaskHandle
                    if current_task.frames.is_empty() {
                        if let Some(hid) = current_task.handle_id {
                            let _ = shared.heap.with_write(hid, |o| {
                                if let heap::Obj::TaskHandle { status, .. } = o {
                                    *status = heap::TaskStatus::Completed(result);
                                }
                            });
                            shared.condvar.notify_all();
                        }
                    }
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
                        _ => fail!("Invalid types for '<' operation"),
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
                        _ => fail!("Invalid types for '>' operation"),
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
                        _ => fail!("Invalid types for '<=' operation"),
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
                        _ => fail!("Invalid types for '>=' operation"),
                    };
                    current_task.stack.push(Value::Bool(res));
                }
                OpCode::And => {
                    let b = pop!();
                    let a = pop!();
                    if let (Value::Bool(x), Value::Bool(y)) = (a, b) {
                        current_task.stack.push(Value::Bool(x && y));
                    } else {
                        fail!("'and' expects booleans");
                    }
                }
                OpCode::Or => {
                    let b = pop!();
                    let a = pop!();
                    if let (Value::Bool(x), Value::Bool(y)) = (a, b) {
                        current_task.stack.push(Value::Bool(x || y));
                    } else {
                        fail!("'or' expects booleans");
                    }
                }
                OpCode::Not => {
                    let a = pop!();
                    if let Value::Bool(x) = a {
                        current_task.stack.push(Value::Bool(!x));
                    } else {
                        fail!("'not' expects a boolean");
                    }
                }
                OpCode::Pop => {
                    pop!();
                }
                OpCode::SetLine(line) => {
                    current_line = line;
                }
                OpCode::Closure(func, upvalues) => {
                    let mut captured = Vec::new();
                    for loc in upvalues {
                        match loc {
                            UpvalueLoc::Local(idx) => {
                                let offset = current_task.frames[frame_idx].stack_offset;
                                let val = current_task.stack[offset + idx].clone();
                                let upvalue_id = shared.heap.alloc(heap::Obj::Upvalue(val));
                                captured.push(upvalue_id);
                            }
                            UpvalueLoc::Upvalue(idx) => {
                                let current_closure_id = current_task.frames[frame_idx].closure_id;
                                if let Ok(upvs) = shared.heap.with_read(current_closure_id, |o| {
                                    if let heap::Obj::Closure(_, u) = o {
                                        Some(u[idx])
                                    } else {
                                        None
                                    }
                                }) {
                                    if let Some(uid) = upvs {
                                        captured.push(uid);
                                    }
                                }
                            }
                        }
                    }
                    let closure_id = shared.heap.alloc(heap::Obj::Closure(func, captured));
                    current_task.stack.push(Value::ObjRef(closure_id));
                }
                OpCode::GetUpvalue(idx) => {
                    let closure_id = current_task.frames[frame_idx].closure_id;
                    let uid = shared.heap.with_read(closure_id, |o| {
                        if let heap::Obj::Closure(_, u) = o {
                            Some(u[idx])
                        } else {
                            None
                        }
                    }).ok().flatten();

                    if let Some(upvalue_id) = uid {
                        if let Ok(val) = shared.heap.with_read(upvalue_id, |o| {
                            if let heap::Obj::Upvalue(v) = o {
                                Some(v.clone())
                            } else {
                                None
                            }
                        }) {
                            if let Some(v) = val {
                                current_task.stack.push(v);
                            }
                        }
                    }
                }
                OpCode::SetUpvalue(idx) => {
                    let val = pop!();
                    let closure_id = current_task.frames[frame_idx].closure_id;
                    let uid = shared.heap.with_read(closure_id, |o| {
                        if let heap::Obj::Closure(_, u) = o {
                            Some(u[idx])
                        } else {
                            None
                        }
                    }).ok().flatten();

                    if let Some(upvalue_id) = uid {
                        let _ = shared.heap.with_write(upvalue_id, |o| {
                            if let heap::Obj::Upvalue(inner) = o {
                                *inner = val;
                            }
                        });
                    }
                }
                OpCode::BuildStruct(name, fields) => {
                    let id = shared.heap.alloc(heap::Obj::StructDef {
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
                        let _ = shared.heap.with_write(id, |o| {
                            if let heap::Obj::StructDef { methods, .. } = o {
                                methods.insert(name, method);
                            }
                        });
                    }
                }
                OpCode::BuildInterface(methods) => {
                    let id = shared.heap.alloc(heap::Obj::Interface(methods));
                    current_task.stack.push(Value::ObjRef(id));
                }
                OpCode::CheckIs => {
                    let right = pop!();
                    let left = pop!();
                    let globals = shared.globals.read();
                    let is_match = is_type_match(&shared.heap, &globals, &left, &right);
                    current_task.stack.push(Value::Bool(is_match));
                }
                OpCode::Assert(msg) => {
                    let cond = pop!();
                    if let Value::Bool(false) = cond {
                        fail!("{}", msg);
                    }
                }
                OpCode::BuildTuple(len) => {
                    let mut elements = Vec::with_capacity(len);
                    for _ in 0..len {
                        elements.push(pop!());
                    }
                    elements.reverse();
                    current_task.stack.push(Value::Tuple(Arc::new(elements)));
                }
                OpCode::UnpackTuple(expected_len) => {
                    let obj = pop!();
                    if let Value::Tuple(elements) = obj {
                        if elements.len() != expected_len {
                            fail!("Cannot unpack tuple of length {} into {} variables", elements.len(), expected_len);
                        }
                        for el in elements.iter() {
                            current_task.stack.push(el.clone());
                        }
                    } else {
                        fail!("Cannot unpack non-tuple value");
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
                            let closure_opt = shared.heap.with_read(id, |o| {
                                if let heap::Obj::Closure(func, _) = o {
                                    Some(func.clone())
                                } else {
                                    None
                                }
                            }).ok().flatten();

                            if let Some(func) = closure_opt {
                                if func.arity != arg_count {
                                    fail!("Spawned function expects {} args", func.arity);
                                }
                                let handle_id = shared.heap.alloc(heap::Obj::TaskHandle {
                                    status: heap::TaskStatus::Running,
                                });

                                let mut new_task = Task {
                                    stack: Vec::new(),
                                    frames: Vec::new(),
                                    state: TaskState::Runnable,
                                    handle_id: Some(handle_id),
                                    is_main: false,
                                };
                                new_task.stack.push(Value::ObjRef(id));
                                for arg in &args {
                                    new_task.stack.push(arg.clone());
                                }
                                let new_frame = CallFrame {
                                    closure_id: id,
                                    function: func,
                                    ip: 0,
                                    stack_offset: 1,
                                    base_offset: 0,
                                    defers: Vec::new(),
                                };
                                new_task.frames.push(new_frame);
                                shared.active_tasks.fetch_add(1, Ordering::Relaxed);
                                shared.injector.push(new_task);
                                shared.condvar.notify_one();
                                current_task.stack.push(Value::ObjRef(handle_id));
                            } else {
                                fail!("Can only spawn functions");
                            }
                        }
                        Value::Native(native_fn) => {
                            let mut temp_vm = VM {
                                main_task: None,
                                globals: shared.globals.read().clone(),
                                modules: shared.modules.read().clone(),
                                heap: shared.heap.clone(),
                                current_line,
                                gc_threshold: shared.gc_threshold.load(Ordering::Relaxed),
                                imported_files: HashSet::new(),
                                net: Arc::clone(&shared.net),
                                channel_timers: Arc::clone(&shared.channel_timers),
                            };
                            match native_fn(args, &mut temp_vm) {
                                crate::value::NativeResult::Return(_) => {
                                    current_task.stack.push(Value::Nil);
                                }
                                crate::value::NativeResult::SuspendSleep(secs) => {
                                    let wake = Instant::now() + Duration::from_secs_f64(secs);
                                    let new_task = Task {
                                        stack: Vec::new(),
                                        frames: Vec::new(),
                                        state: TaskState::Runnable,
                                        handle_id: None,
                                        is_main: false,
                                    };
                                    shared.active_tasks.fetch_add(1, Ordering::Relaxed);
                                    shared.sleeping_tasks.lock().push(SleepingTask { wake_time: wake, task: new_task });
                                    current_task.stack.push(Value::Nil);
                                }
                                crate::value::NativeResult::SuspendIO(token) => {
                                    let new_task = Task {
                                        stack: Vec::new(),
                                        frames: Vec::new(),
                                        state: TaskState::WaitingIO(token),
                                        handle_id: None,
                                        is_main: false,
                                    };
                                    shared.active_tasks.fetch_add(1, Ordering::Relaxed);
                                    shared.waiting_io_tasks.lock().push(new_task);
                                    current_task.stack.push(Value::Nil);
                                }
                            }
                        }
                        _ => fail!("Can only spawn functions"),
                    }
                }
                OpCode::ChanRecv => {
                    let chan_val = pop!();
                    if let Value::ObjRef(id) = chan_val {
                        // Check if it's a Channel or TaskHandle
                        let is_handle = shared.heap.with_read(id, |o| matches!(o, heap::Obj::TaskHandle { .. })).unwrap_or(false);
                        if is_handle {
                            let status_res = shared.heap.with_read(id, |o| {
                                if let heap::Obj::TaskHandle { status, .. } = o {
                                    Some(status.clone())
                                } else {
                                    None
                                }
                            }).ok().flatten();

                            match status_res {
                                Some(heap::TaskStatus::Running) => {
                                    current_task.stack.push(chan_val);
                                    current_task.frames[frame_idx].ip -= 1;
                                    current_task.state = TaskState::Waiting;
                                    return SliceResult::Parked(current_task);
                                }
                                Some(heap::TaskStatus::Completed(val)) => {
                                    let tuple = Value::Tuple(Arc::new(vec![val, Value::Nil]));
                                    current_task.stack.push(tuple);
                                }
                                Some(heap::TaskStatus::Failed(err_str)) => {
                                    let tuple = Value::Tuple(Arc::new(vec![Value::Nil, Value::Str(Arc::new(err_str))]));
                                    current_task.stack.push(tuple);
                                }
                                None => fail!("Invalid task handle"),
                            }
                            fuel -= 1;
                            continue;
                        }

                        let res = shared.heap.with_write(id, |o| {
                            if let crate::heap::Obj::Channel { queue, closed, .. } = o {
                                if let Some(m) = queue.pop_front() {
                                    Some((true, m, *closed))
                                } else {
                                    Some((false, Value::Nil, *closed))
                                }
                            } else {
                                None
                            }
                        }).ok().flatten();

                        match res {
                            Some((true, msg, _)) => {
                                current_task.stack.push(msg);
                                shared.condvar.notify_all();
                            }
                            Some((false, _, true)) => {
                                current_task.stack.push(Value::Nil);
                            }
                            Some((false, _, false)) => {
                                current_task.stack.push(chan_val);
                                current_task.frames[frame_idx].ip -= 1;
                                current_task.state = TaskState::Waiting;
                                return SliceResult::Parked(current_task);
                            }
                            None => fail!("Attempt to read from a non-channel"),
                        }
                    } else {
                        fail!("Attempt to read from a non-channel");
                    }
                }
                OpCode::ChanSend => {
                    let val = pop!();
                    let chan_val = pop!();

                    if let Value::ObjRef(id) = chan_val {
                        let res = shared.heap.with_write(id, |o| {
                            if let crate::heap::Obj::Channel { queue, capacity, closed } = o {
                                if *closed {
                                    return Some(Err("Cannot send to a closed channel".to_string()));
                                }
                                if queue.len() >= *capacity {
                                    return Some(Ok(false)); // Full
                                }
                                queue.push_back(val.clone());
                                return Some(Ok(true)); // Enqueued
                            }
                            None
                        }).ok().flatten();

                        match res {
                            Some(Ok(true)) => {
                                current_task.stack.push(Value::Bool(true));
                                shared.condvar.notify_all();
                            }
                            Some(Ok(false)) => {
                                current_task.stack.push(chan_val);
                                current_task.stack.push(val);
                                current_task.frames[frame_idx].ip -= 1;
                                current_task.state = TaskState::Waiting;
                                return SliceResult::Parked(current_task);
                            }
                            Some(Err(err)) => fail!("{}", err),
                            None => fail!("Cannot send to a non-channel"),
                        }
                    } else {
                        fail!("Cannot send to a non-channel");
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
                            Value::ObjRef(id) => {
                                let (kind, list_item, map_key, chan_res) = shared.heap.with_write(id, |o| {
                                    match o {
                                        crate::heap::Obj::List(list) => {
                                            if current_idx < list.len() as i64 {
                                                (1, Some(list[current_idx as usize].clone()), None, None)
                                            } else {
                                                (1, None, None, None)
                                            }
                                        }
                                        crate::heap::Obj::Map(map) => {
                                            let keys: Vec<String> = map.keys().cloned().collect();
                                            if current_idx < keys.len() as i64 {
                                                (2, None, Some(keys[current_idx as usize].clone()), None)
                                            } else {
                                                (2, None, None, None)
                                            }
                                        }
                                        crate::heap::Obj::Channel { queue, closed, .. } => {
                                            if let Some(m) = queue.pop_front() {
                                                (3, None, None, Some((true, m, *closed)))
                                            } else {
                                                (3, None, None, Some((false, Value::Nil, *closed)))
                                            }
                                        }
                                        crate::heap::Obj::TaskHandle { status, .. } => {
                                            match status {
                                                heap::TaskStatus::Running => (4, None, None, Some((true, Value::Nil, false))),
                                                heap::TaskStatus::Completed(v) => (4, None, None, Some((false, Value::Tuple(Arc::new(vec![v.clone(), Value::Nil])), false))),
                                                heap::TaskStatus::Failed(e) => (4, None, None, Some((false, Value::Tuple(Arc::new(vec![Value::Nil, Value::Str(Arc::new(e.clone()))])), false))),
                                            }
                                        }
                                        _ => (0, None, None, None),
                                    }
                                }).unwrap_or((0, None, None, None));

                                match kind {
                                    1 => {
                                        if let Some(item) = list_item {
                                            current_task.stack[counter_idx] = Value::Int(current_idx + 1);
                                            current_task.stack.push(item);
                                            current_task.stack.push(Value::Bool(true));
                                        } else {
                                            current_task.stack.push(Value::Nil);
                                            current_task.stack.push(Value::Bool(false));
                                        }
                                    }
                                    2 => {
                                        if let Some(key) = map_key {
                                            current_task.stack[counter_idx] = Value::Int(current_idx + 1);
                                            current_task.stack.push(Value::Str(Arc::new(key)));
                                            current_task.stack.push(Value::Bool(true));
                                        } else {
                                            current_task.stack.push(Value::Nil);
                                            current_task.stack.push(Value::Bool(false));
                                        }
                                    }
                                    3 => {
                                        let (has_msg, msg, closed) = chan_res.unwrap();
                                        if has_msg {
                                            current_task.stack.push(msg);
                                            current_task.stack.push(Value::Bool(true));
                                            shared.condvar.notify_all();
                                        } else if closed {
                                            current_task.stack.push(Value::Nil);
                                            current_task.stack.push(Value::Bool(false));
                                        } else {
                                            current_task.frames[frame_idx].ip -= 1;
                                            current_task.state = TaskState::Waiting;
                                            return SliceResult::Parked(current_task);
                                        }
                                    }
                                    4 => {
                                        // TaskHandle iteration: wait for completion, yield single result
                                        let (is_running, val_opt, _) = chan_res.unwrap();
                                        if current_idx == 0 {
                                            if is_running {
                                                current_task.frames[frame_idx].ip -= 1;
                                                current_task.state = TaskState::Waiting;
                                                return SliceResult::Parked(current_task);
                                            } else {
                                                current_task.stack[counter_idx] = Value::Int(1);
                                                current_task.stack.push(val_opt);
                                                current_task.stack.push(Value::Bool(true));
                                            }
                                        } else {
                                            current_task.stack.push(Value::Nil);
                                            current_task.stack.push(Value::Bool(false));
                                        }
                                    }
                                    _ => fail!("TypeError: object is not iterable"),
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
                                    current_task.stack.push(Value::Str(Arc::new(ch)));
                                    current_task.stack.push(Value::Bool(true));
                                } else {
                                    current_task.stack.push(Value::Nil);
                                    current_task.stack.push(Value::Bool(false));
                                }
                            }
                            _ => fail!("TypeError: object is not iterable"),
                        }
                    } else {
                        fail!("Iterator state is corrupted");
                    }
                }
                OpCode::PropagateError => {
                    let obj = pop!();
                    if let Value::Tuple(ref elements) = obj {
                        if elements.len() == 2 {
                            let res = &elements[0];
                            let err = &elements[1];

                            if *err != Value::Nil {
                                if current_task.frames.len() > 1 {
                                    let mut frame = current_task.frames.pop().unwrap();
                                    while let Some(deferred) = frame.defers.pop() {
                                        execute_sync_deferred(&deferred, shared, current_line);
                                    }
                                    current_task.stack.truncate(frame.base_offset);
                                    current_task.stack.push(obj);
                                    continue;
                                } else {
                                    fail!("Unhandled error in '?': {}", err);
                                }
                            } else {
                                current_task.stack.push(res.clone());
                            }
                        } else {
                            current_task.stack.push(obj);
                        }
                    } else {
                        current_task.stack.push(obj);
                    }
                }
                OpCode::Select(cases) => {
                    use crate::opcode::SelectCaseOp;

                    let mut total_args = 0;
                    for c in &cases {
                        match c {
                            SelectCaseOp::Recv => total_args += 1,
                            SelectCaseOp::Send => total_args += 2,
                            SelectCaseOp::Default => {}
                        }
                    }

                    let stack_start = current_task.stack.len() - total_args;
                    let mut selected = None;
                    let mut default_idx = None;
                    let mut cursor = stack_start;

                    for (idx, c) in cases.iter().enumerate() {
                        match c {
                            SelectCaseOp::Default => {
                                default_idx = Some(idx);
                            }
                            SelectCaseOp::Recv => {
                                let chan_val = &current_task.stack[cursor];
                                cursor += 1;
                                if let Value::ObjRef(id) = chan_val {
                                    let ready = shared.heap.with_read(*id, |o| {
                                        match o {
                                            crate::heap::Obj::Channel { queue, closed, .. } => !queue.is_empty() || *closed,
                                            crate::heap::Obj::TaskHandle { status, .. } => !matches!(status, heap::TaskStatus::Running),
                                            _ => false,
                                        }
                                    }).unwrap_or(false);

                                    if ready {
                                        selected = Some((idx, true, *id, Value::Nil));
                                        break;
                                    }
                                }
                            }
                            SelectCaseOp::Send => {
                                let send_val = current_task.stack[cursor].clone();
                                let chan_val = &current_task.stack[cursor + 1];
                                cursor += 2;
                                if let Value::ObjRef(id) = chan_val {
                                    let ready = shared.heap.with_read(*id, |o| {
                                        if let crate::heap::Obj::Channel { queue, capacity, closed } = o {
                                            !*closed && queue.len() < *capacity
                                        } else {
                                            false
                                        }
                                    }).unwrap_or(false);

                                    if ready {
                                        selected = Some((idx, false, *id, send_val));
                                        break;
                                    }
                                }
                            }
                        }
                    }

                    if let Some((ready_idx, is_recv, chan_id, send_val)) = selected {
                        current_task.stack.truncate(stack_start);
                        let received_msg = if is_recv {
                            let is_task_handle = shared.heap.with_read(chan_id, |o| matches!(o, heap::Obj::TaskHandle { .. })).unwrap_or(false);
                            if is_task_handle {
                                shared.heap.with_read(chan_id, |o| {
                                    if let heap::Obj::TaskHandle { status, .. } = o {
                                        match status {
                                            heap::TaskStatus::Completed(val) => Value::Tuple(Arc::new(vec![val.clone(), Value::Nil])),
                                            heap::TaskStatus::Failed(err_str) => Value::Tuple(Arc::new(vec![Value::Nil, Value::Str(Arc::new(err_str.clone()))])),
                                            _ => Value::Nil,
                                        }
                                    } else {
                                        Value::Nil
                                    }
                                }).unwrap_or(Value::Nil)
                            } else {
                                shared.heap.with_write(chan_id, |o| {
                                    if let crate::heap::Obj::Channel { queue, .. } = o {
                                        queue.pop_front().unwrap_or(Value::Nil)
                                    } else {
                                        Value::Nil
                                    }
                                }).unwrap_or(Value::Nil)
                            }
                        } else {
                            let _ = shared.heap.with_write(chan_id, |o| {
                                if let crate::heap::Obj::Channel { queue, .. } = o {
                                    queue.push_back(send_val);
                                }
                            });
                            Value::Nil
                        };

                        current_task.stack.push(received_msg);
                        current_task.stack.push(Value::Int(ready_idx as i64));
                        shared.condvar.notify_all();
                    } else if let Some(def_idx) = default_idx {
                        current_task.stack.truncate(stack_start);
                        current_task.stack.push(Value::Nil);
                        current_task.stack.push(Value::Int(def_idx as i64));
                    } else {
                        current_task.frames[frame_idx].ip -= 1;
                        current_task.state = TaskState::Waiting;
                        return SliceResult::Parked(current_task);
                    }
                }
            }

            fuel -= 1;
        }

        if current_task.frames.is_empty() {
            SliceResult::Completed
        } else {
            SliceResult::Exhausted(current_task)
        }
    }
