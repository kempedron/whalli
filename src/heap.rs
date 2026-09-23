use crate::value::{FunctionObj, Value};
use parking_lot::{Mutex, RwLock};
use std::{
    collections::{HashMap, VecDeque},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

pub const CHUNK_SIZE: usize = 1024;

#[derive(Debug, Clone)]
pub enum TaskStatus {
    Running,
    Completed(Value),
    Failed(String),
}

#[derive(Debug, Clone)]
pub enum Obj {
    List(Vec<Value>),
    Map(HashMap<String, Value>),
    Upvalue(Value),
    Closure(Arc<FunctionObj>, Vec<usize>),

    StructDef {
        name: String,
        fields: Vec<String>,
        methods: HashMap<String, Value>,
    },
    Instance {
        struct_id: usize,
        fields: HashMap<String, Value>,
    },
    Interface(Vec<String>),
    Channel {
        queue: VecDeque<Value>,
        capacity: usize,
        closed: bool,
    },
    WaitGroup {
        count: usize,
        waiting_tasks: Vec<usize>,
    },
    Mutex {
        locked: bool,
        waiting_tasks: Vec<usize>,
    },
    TaskHandle {
        status: TaskStatus,
    },
}

pub struct HeapSlot {
    pub marked: AtomicBool,
    pub occupied: AtomicBool,
    pub data: RwLock<Obj>,
}

impl HeapSlot {
    pub fn new() -> Self {
        HeapSlot {
            marked: AtomicBool::new(false),
            occupied: AtomicBool::new(false),
            data: RwLock::new(Obj::Interface(Vec::new())),
        }
    }
}

#[derive(Clone)]
pub struct Heap {
    chunks: Arc<RwLock<Vec<Box<[HeapSlot; CHUNK_SIZE]>>>>,
    free_list: Arc<Mutex<Vec<usize>>>,
    allocated_count: Arc<Mutex<usize>>,
}

fn push_value_refs(val: &Value, worklist: &mut Vec<usize>) {
    match val {
        Value::ObjRef(id) => worklist.push(*id),
        Value::Tuple(elements) => {
            for elem in elements.iter() {
                push_value_refs(elem, worklist);
            }
        }
        _ => {}
    }
}

impl Heap {
    pub fn new() -> Self {
        Heap {
            chunks: Arc::new(RwLock::new(Vec::new())),
            free_list: Arc::new(Mutex::new(Vec::new())),
            allocated_count: Arc::new(Mutex::new(0)),
        }
    }

    pub fn live_count(&self) -> usize {
        let total = *self.allocated_count.lock();
        let free = self.free_list.lock().len();
        total.saturating_sub(free)
    }

    pub fn alloc(&self, data: Obj) -> usize {
        if let Some(idx) = self.free_list.lock().pop() {
            let chunk_idx = idx / CHUNK_SIZE;
            let slot_idx = idx % CHUNK_SIZE;
            let chunks = self.chunks.read();
            let slot = &chunks[chunk_idx][slot_idx];
            slot.marked.store(false, Ordering::Relaxed);
            *slot.data.write() = data;
            slot.occupied.store(true, Ordering::Release);
            return idx;
        }

        let mut chunks = self.chunks.write();
        let mut total = self.allocated_count.lock();
        let idx = *total;
        let chunk_idx = idx / CHUNK_SIZE;
        let slot_idx = idx % CHUNK_SIZE;

        if chunk_idx >= chunks.len() {
            let boxed: Box<[HeapSlot; CHUNK_SIZE]> = Box::new(std::array::from_fn(|_| HeapSlot::new()));
            chunks.push(boxed);
        }

        let slot = &chunks[chunk_idx][slot_idx];
        slot.marked.store(false, Ordering::Relaxed);
        *slot.data.write() = data;
        slot.occupied.store(true, Ordering::Release);
        *total += 1;
        idx
    }

    pub fn with_read<R>(&self, id: usize, f: impl FnOnce(&Obj) -> R) -> Result<R, String> {
        let chunk_idx = id / CHUNK_SIZE;
        let slot_idx = id % CHUNK_SIZE;
        let chunks = self.chunks.read();
        if chunk_idx >= chunks.len() {
            return Err(format!("Invalid memory address: {}", id));
        }
        let slot = &chunks[chunk_idx][slot_idx];
        if !slot.occupied.load(Ordering::Acquire) {
            return Err(format!("Invalid memory address: {}", id));
        }
        let guard = slot.data.read();
        Ok(f(&*guard))
    }

    pub fn with_write<R>(&self, id: usize, f: impl FnOnce(&mut Obj) -> R) -> Result<R, String> {
        let chunk_idx = id / CHUNK_SIZE;
        let slot_idx = id % CHUNK_SIZE;
        let chunks = self.chunks.read();
        if chunk_idx >= chunks.len() {
            return Err(format!("Invalid memory address: {}", id));
        }
        let slot = &chunks[chunk_idx][slot_idx];
        if !slot.occupied.load(Ordering::Acquire) {
            return Err(format!("Invalid memory address: {}", id));
        }
        let mut guard = slot.data.write();
        Ok(f(&mut *guard))
    }

    pub fn get(&self, id: usize) -> Result<Obj, String> {
        self.with_read(id, |obj| obj.clone())
    }

    pub fn mark(&self, root_id: usize) {
        let mut worklist = vec![root_id];

        let chunks = self.chunks.read();
        while let Some(id) = worklist.pop() {
            let chunk_idx = id / CHUNK_SIZE;
            let slot_idx = id % CHUNK_SIZE;
            if chunk_idx >= chunks.len() {
                continue;
            }
            let slot = &chunks[chunk_idx][slot_idx];
            if !slot.occupied.load(Ordering::Acquire) {
                continue;
            }

            if slot.marked.swap(true, Ordering::AcqRel) {
                continue;
            }

            let obj_guard = slot.data.read();
            match &*obj_guard {
                Obj::List(list) => {
                    for val in list {
                        push_value_refs(val, &mut worklist);
                    }
                }
                Obj::Map(map) => {
                    for val in map.values() {
                        push_value_refs(val, &mut worklist);
                    }
                }
                Obj::Upvalue(val) => {
                    push_value_refs(val, &mut worklist);
                }
                Obj::Closure(_, upvalues) => {
                    for upvalue_id in upvalues {
                        worklist.push(*upvalue_id);
                    }
                }
                Obj::StructDef { methods, .. } => {
                    for val in methods.values() {
                        push_value_refs(val, &mut worklist);
                    }
                }
                Obj::Instance { struct_id, fields } => {
                    worklist.push(*struct_id);
                    for val in fields.values() {
                        push_value_refs(val, &mut worklist);
                    }
                }
                Obj::Channel { queue, .. } => {
                    for val in queue {
                        push_value_refs(val, &mut worklist);
                    }
                }
                Obj::TaskHandle { status } => {
                    if let TaskStatus::Completed(val) = status {
                        push_value_refs(val, &mut worklist);
                    }
                }
                Obj::WaitGroup { .. } => {}
                Obj::Mutex { .. } => {}
                Obj::Interface(_) => {}
            }
        }
    }

    pub fn sweep(&self) -> usize {
        let mut freed_count = 0;
        let chunks = self.chunks.read();
        let total = *self.allocated_count.lock();
        let mut free_list = self.free_list.lock();

        for i in 0..total {
            let chunk_idx = i / CHUNK_SIZE;
            let slot_idx = i % CHUNK_SIZE;
            let slot = &chunks[chunk_idx][slot_idx];

            if slot.occupied.load(Ordering::Acquire) {
                if slot.marked.load(Ordering::Acquire) {
                    slot.marked.store(false, Ordering::Release);
                } else {
                    slot.occupied.store(false, Ordering::Release);
                    free_list.push(i);
                    freed_count += 1;
                }
            }
        }
        freed_count
    }
}
