use std::collections::HashMap;
use crate::value::Value;

#[derive(Debug, Clone)]
pub enum Obj {
    List(Vec<Value>),
    Map(HashMap<String, Value>),
}

pub struct HeapObj {
    pub marked: bool,
    pub data: Obj,
}

pub struct Heap {
    objects: Vec<Option<HeapObj>>,
    free_list: Vec<usize>,
}

impl Heap {
    pub fn new() -> Self {
        Heap { objects: Vec::new(), free_list: Vec::new() }
    }

    pub fn live_count(&self) -> usize {
        self.objects.len() - self.free_list.len()
    }

    pub fn mark(&mut self, root_id: usize) {
        let mut worklist = vec![root_id];

        while let Some(id) = worklist.pop() {
            if let Some(Some(heap_obj)) = self.objects.get_mut(id) {
                if heap_obj.marked { continue; }
                
                heap_obj.marked = true;
                
                match &heap_obj.data {
                    Obj::List(list) => {
                        for val in list {
                            if let Value::ObjRef(child_id) = val {
                                worklist.push(*child_id);
                            }
                        }
                    }
                    Obj::Map(map) => {
                        for val in map.values() {
                            if let Value::ObjRef(child_id) = val {
                                worklist.push(*child_id);
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn sweep(&mut self) -> usize {
        let mut freed_count = 0;

        for i in 0..self.objects.len() {
            if let Some(obj) = &mut self.objects[i] {
                if obj.marked {
                    obj.marked = false;
                } else {
                    self.objects[i] = None;
                    self.free_list.push(i);
                    freed_count += 1;
                }
            }
        }
        freed_count
    }

    pub fn alloc(&mut self, data: Obj) -> usize {
        let heap_obj = HeapObj { marked: false, data };
        if let Some(idx) = self.free_list.pop() {
            self.objects[idx] = Some(heap_obj);
            idx
        } else {
            let idx = self.objects.len();
            self.objects.push(Some(heap_obj));
            idx
        }
    }

    pub fn get(&self, id: usize) -> Result<&Obj, String> {
        self.objects.get(id)
            .and_then(|opt| opt.as_ref())
            .map(|h| &h.data)
            .ok_or_else(|| format!("Invalid memory address: {}", id))
    }

    pub fn get_mut(&mut self, id: usize) -> Result<&mut Obj, String> {
        self.objects.get_mut(id)
            .and_then(|opt| opt.as_mut())
            .map(|h| &mut h.data)
            .ok_or_else(|| format!("Invalid memory address: {}", id))
    }
}