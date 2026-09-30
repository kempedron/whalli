use std::sync::Arc;

use crate::value::{FunctionObj, Value};

#[derive(Debug, Clone, PartialEq)]
pub enum UpvalueLoc {
    Local(usize),
    Upvalue(usize),
}

#[derive(Debug, Clone, PartialEq)]
pub enum SelectCaseOp {
    Recv,
    Send,
    Default,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Op {
    Push = 0,
    Constant,
    Pop,
    Add,
    Sub,
    Div,
    Mul,
    Mod,
    StoreGlobal,
    LoadGlobal,
    LoadLocal,
    SetLocal,
    Call,
    TailCall,
    MethodCall,
    Return,
    Equal,
    Less,
    Greater,
    LessEqual,
    GreaterEqual,
    JumpIfFalse,
    Jump,
    BuildList,
    BuildMap,
    IndexGet,
    ListLen,
    IndexSet,
    And,
    Or,
    Not,
    SetLine,
    Import,
    ImportFile,
    GetUpvalue,
    SetUpvalue,
    Closure,
    BuildStruct,
    AddMethod,
    BuildInterface,
    CheckIs,
    Assert,
    BuildTuple,
    UnpackTuple,
    UnpackList,
    UnpackObject,
    FromImport,
    Spawn,
    ChanSend,
    ChanRecv,
    IterNext,
    PropagateError,
    Select,
    DeferCall,
    DeferMethodCall,
    Export,
    BuildModule,
}

impl Op {
    #[inline(always)]
    pub fn from_u8(b: u8) -> Self {
        unsafe { std::mem::transmute(b) }
    }
}

pub fn serialize_chunk(chunk: &[OpCode], constants: &mut Vec<Value>) -> Vec<u8> {
    let mut bytecode = Vec::with_capacity(chunk.len() * 3);
    // Maps original instruction index in `chunk` to bytecode offset
    let mut ip_map = Vec::with_capacity(chunk.len() + 1);

    // Helper closure to add or deduplicate constant
    let mut add_const = |val: Value| -> u16 {
        for (i, c) in constants.iter().enumerate() {
            if c == &val {
                return i as u16;
            }
        }
        constants.push(val);
        (constants.len() - 1) as u16
    };

    // First pass: emit instructions with placeholder jump targets
    for op in chunk {
        ip_map.push(bytecode.len());
        match op {
            OpCode::Push(v) => {
                let idx = add_const(v.clone());
                bytecode.push(Op::Constant as u8);
                bytecode.extend_from_slice(&idx.to_le_bytes());
            }
            OpCode::Constant(idx) => {
                bytecode.push(Op::Constant as u8);
                bytecode.extend_from_slice(&(*idx as u16).to_le_bytes());
            }
            OpCode::Pop => bytecode.push(Op::Pop as u8),
            OpCode::Add => bytecode.push(Op::Add as u8),
            OpCode::Sub => bytecode.push(Op::Sub as u8),
            OpCode::Div => bytecode.push(Op::Div as u8),
            OpCode::Mul => bytecode.push(Op::Mul as u8),
            OpCode::Mod => bytecode.push(Op::Mod as u8),
            OpCode::StoreGlobal(name) => {
                let idx = add_const(Value::Str(name.clone()));
                bytecode.push(Op::StoreGlobal as u8);
                bytecode.extend_from_slice(&idx.to_le_bytes());
            }
            OpCode::LoadGlobal(name) => {
                let idx = add_const(Value::Str(name.clone()));
                bytecode.push(Op::LoadGlobal as u8);
                bytecode.extend_from_slice(&idx.to_le_bytes());
            }
            OpCode::LoadLocal(idx) => {
                bytecode.push(Op::LoadLocal as u8);
                bytecode.extend_from_slice(&(*idx as u16).to_le_bytes());
            }
            OpCode::SetLocal(idx) => {
                bytecode.push(Op::SetLocal as u8);
                bytecode.extend_from_slice(&(*idx as u16).to_le_bytes());
            }
            OpCode::Call(arity) => {
                bytecode.push(Op::Call as u8);
                bytecode.push(*arity as u8);
            }
            OpCode::TailCall(arity) => {
                bytecode.push(Op::TailCall as u8);
                bytecode.push(*arity as u8);
            }
            OpCode::MethodCall(name, arity) => {
                let idx = add_const(Value::Str(name.clone()));
                bytecode.push(Op::MethodCall as u8);
                bytecode.extend_from_slice(&idx.to_le_bytes());
                bytecode.push(*arity as u8);
            }
            OpCode::Return => bytecode.push(Op::Return as u8),
            OpCode::Equal => bytecode.push(Op::Equal as u8),
            OpCode::Less => bytecode.push(Op::Less as u8),
            OpCode::Greater => bytecode.push(Op::Greater as u8),
            OpCode::LessEqual => bytecode.push(Op::LessEqual as u8),
            OpCode::GreaterEqual => bytecode.push(Op::GreaterEqual as u8),
            OpCode::JumpIfFalse(target_ip) => {
                bytecode.push(Op::JumpIfFalse as u8);
                // Placeholder, will patch in second pass
                bytecode.extend_from_slice(&(*target_ip as u32).to_le_bytes());
            }
            OpCode::Jump(target_ip) => {
                bytecode.push(Op::Jump as u8);
                bytecode.extend_from_slice(&(*target_ip as u32).to_le_bytes());
            }
            OpCode::BuildList(len) => {
                bytecode.push(Op::BuildList as u8);
                bytecode.extend_from_slice(&(*len as u16).to_le_bytes());
            }
            OpCode::BuildMap(len) => {
                bytecode.push(Op::BuildMap as u8);
                bytecode.extend_from_slice(&(*len as u16).to_le_bytes());
            }
            OpCode::IndexGet => bytecode.push(Op::IndexGet as u8),
            OpCode::ListLen => bytecode.push(Op::ListLen as u8),
            OpCode::IndexSet => bytecode.push(Op::IndexSet as u8),
            OpCode::And => bytecode.push(Op::And as u8),
            OpCode::Or => bytecode.push(Op::Or as u8),
            OpCode::Not => bytecode.push(Op::Not as u8),
            OpCode::SetLine(line) => {
                bytecode.push(Op::SetLine as u8);
                bytecode.extend_from_slice(&(*line as u32).to_le_bytes());
            }
            OpCode::Import { name, alias } => {
                let name_idx = add_const(Value::Str(name.clone()));
                let alias_idx = add_const(Value::Str(alias.clone()));
                bytecode.push(Op::Import as u8);
                bytecode.extend_from_slice(&name_idx.to_le_bytes());
                bytecode.extend_from_slice(&alias_idx.to_le_bytes());
            }
            OpCode::ImportFile { path, alias } => {
                let path_idx = add_const(Value::Str(path.clone()));
                let alias_idx = add_const(Value::Str(alias.clone()));
                bytecode.push(Op::ImportFile as u8);
                bytecode.extend_from_slice(&path_idx.to_le_bytes());
                bytecode.extend_from_slice(&alias_idx.to_le_bytes());
            }
            OpCode::GetUpvalue(idx) => {
                bytecode.push(Op::GetUpvalue as u8);
                bytecode.extend_from_slice(&(*idx as u16).to_le_bytes());
            }
            OpCode::SetUpvalue(idx) => {
                bytecode.push(Op::SetUpvalue as u8);
                bytecode.extend_from_slice(&(*idx as u16).to_le_bytes());
            }
            OpCode::Closure(func, upvalues) => {
                // Ensure the closure's function also has its bytecode serialized!
                let mut inner_func = (**func).clone();
                if inner_func.bytecode.is_empty() {
                    inner_func.bytecode = serialize_chunk(&inner_func.chunk, &mut inner_func.constants);
                }
                let func_idx = add_const(Value::Function(Arc::new(inner_func)));
                bytecode.push(Op::Closure as u8);
                bytecode.extend_from_slice(&func_idx.to_le_bytes());
                bytecode.extend_from_slice(&(upvalues.len() as u16).to_le_bytes());
                for loc in upvalues.iter() {
                    match loc {
                        UpvalueLoc::Local(idx) => {
                            bytecode.push(1u8);
                            bytecode.extend_from_slice(&(*idx as u16).to_le_bytes());
                        }
                        UpvalueLoc::Upvalue(idx) => {
                            bytecode.push(0u8);
                            bytecode.extend_from_slice(&(*idx as u16).to_le_bytes());
                        }
                    }
                }
            }
            OpCode::BuildStruct(name, fields) => {
                let name_idx = add_const(Value::Str(name.clone()));
                bytecode.push(Op::BuildStruct as u8);
                bytecode.extend_from_slice(&name_idx.to_le_bytes());
                bytecode.extend_from_slice(&(fields.len() as u16).to_le_bytes());
                for f in fields.iter() {
                    let f_idx = add_const(Value::Str(Arc::new(f.clone())));
                    bytecode.extend_from_slice(&f_idx.to_le_bytes());
                }
            }
            OpCode::AddMethod(name) => {
                let name_idx = add_const(Value::Str(name.clone()));
                bytecode.push(Op::AddMethod as u8);
                bytecode.extend_from_slice(&name_idx.to_le_bytes());
            }
            OpCode::BuildInterface(methods) => {
                bytecode.push(Op::BuildInterface as u8);
                bytecode.extend_from_slice(&(methods.len() as u16).to_le_bytes());
                for m in methods.iter() {
                    let m_idx = add_const(Value::Str(Arc::new(m.clone())));
                    bytecode.extend_from_slice(&m_idx.to_le_bytes());
                }
            }
            OpCode::CheckIs => bytecode.push(Op::CheckIs as u8),
            OpCode::Assert(msg) => {
                let msg_idx = add_const(Value::Str(msg.clone()));
                bytecode.push(Op::Assert as u8);
                bytecode.extend_from_slice(&msg_idx.to_le_bytes());
            }
            OpCode::BuildTuple(len) => {
                bytecode.push(Op::BuildTuple as u8);
                bytecode.extend_from_slice(&(*len as u16).to_le_bytes());
            }
            OpCode::UnpackTuple(len) => {
                bytecode.push(Op::UnpackTuple as u8);
                bytecode.extend_from_slice(&(*len as u16).to_le_bytes());
            }
            OpCode::UnpackList(len) => {
                bytecode.push(Op::UnpackList as u8);
                bytecode.extend_from_slice(&(*len as u16).to_le_bytes());
            }
            OpCode::UnpackObject(props) => {
                bytecode.push(Op::UnpackObject as u8);
                bytecode.extend_from_slice(&(props.len() as u16).to_le_bytes());
                for p in props.iter() {
                    let p_idx = add_const(Value::Str(Arc::new(p.clone())));
                    bytecode.extend_from_slice(&p_idx.to_le_bytes());
                }
            }
            OpCode::FromImport { source, symbols } => {
                let src_idx = add_const(Value::Str(source.clone()));
                bytecode.push(Op::FromImport as u8);
                bytecode.extend_from_slice(&src_idx.to_le_bytes());
                bytecode.extend_from_slice(&(symbols.len() as u16).to_le_bytes());
                for (from_sym, to_sym) in symbols.iter() {
                    let from_idx = add_const(Value::Str(Arc::new(from_sym.clone())));
                    let to_idx = add_const(Value::Str(Arc::new(to_sym.clone())));
                    bytecode.extend_from_slice(&from_idx.to_le_bytes());
                    bytecode.extend_from_slice(&to_idx.to_le_bytes());
                }
            }
            OpCode::Spawn(arity) => {
                bytecode.push(Op::Spawn as u8);
                bytecode.push(*arity as u8);
            }
            OpCode::ChanSend => bytecode.push(Op::ChanSend as u8),
            OpCode::ChanRecv => bytecode.push(Op::ChanRecv as u8),
            OpCode::IterNext(iter_idx) => {
                bytecode.push(Op::IterNext as u8);
                bytecode.extend_from_slice(&(*iter_idx as u16).to_le_bytes());
            }
            OpCode::PropagateError => bytecode.push(Op::PropagateError as u8),
            OpCode::Select(cases) => {
                bytecode.push(Op::Select as u8);
                bytecode.extend_from_slice(&(cases.len() as u16).to_le_bytes());
                for case_op in cases.iter() {
                    let code = match case_op {
                        SelectCaseOp::Recv => 0u8,
                        SelectCaseOp::Send => 1u8,
                        SelectCaseOp::Default => 2u8,
                    };
                    bytecode.push(code);
                }
            }
            OpCode::DeferCall(arity) => {
                bytecode.push(Op::DeferCall as u8);
                bytecode.push(*arity as u8);
            }
            OpCode::DeferMethodCall(name, arity) => {
                let name_idx = add_const(Value::Str(name.clone()));
                bytecode.push(Op::DeferMethodCall as u8);
                bytecode.extend_from_slice(&name_idx.to_le_bytes());
                bytecode.push(*arity as u8);
            }
            OpCode::Export(name) => {
                let name_idx = add_const(Value::Str(name.clone()));
                bytecode.push(Op::Export as u8);
                bytecode.extend_from_slice(&name_idx.to_le_bytes());
            }
            OpCode::BuildModule(count) => {
                bytecode.push(Op::BuildModule as u8);
                bytecode.extend_from_slice(&(*count as u16).to_le_bytes());
            }
        }
    }
    ip_map.push(bytecode.len());

    // Second pass: patch Jump and JumpIfFalse target byte offsets
    let mut scan_ip = 0;
    while scan_ip < bytecode.len() {
        let op = Op::from_u8(bytecode[scan_ip]);
        scan_ip += 1;
        match op {
            Op::Jump | Op::JumpIfFalse => {
                let old_target_ip = u32::from_le_bytes([
                    bytecode[scan_ip],
                    bytecode[scan_ip + 1],
                    bytecode[scan_ip + 2],
                    bytecode[scan_ip + 3],
                ]) as usize;

                let target_byte_offset = if old_target_ip < ip_map.len() {
                    ip_map[old_target_ip]
                } else {
                    bytecode.len()
                };

                let target_bytes = (target_byte_offset as u32).to_le_bytes();
                bytecode[scan_ip..scan_ip + 4].copy_from_slice(&target_bytes);
                scan_ip += 4;
            }
            Op::Constant | Op::StoreGlobal | Op::LoadGlobal | Op::LoadLocal | Op::SetLocal | Op::BuildList
            | Op::BuildMap | Op::GetUpvalue | Op::SetUpvalue | Op::AddMethod | Op::BuildTuple | Op::UnpackTuple
            | Op::UnpackList | Op::IterNext | Op::Export | Op::BuildModule => {
                scan_ip += 2;
            }
            Op::Call | Op::TailCall | Op::Spawn | Op::DeferCall => {
                scan_ip += 1;
            }
            Op::MethodCall | Op::DeferMethodCall => {
                scan_ip += 3; // u16 + u8
            }
            Op::SetLine => {
                scan_ip += 4; // u32
            }
            Op::Import | Op::ImportFile => {
                scan_ip += 4; // u16 + u16
            }
            Op::Closure => {
                scan_ip += 2; // func idx (u16)
                let count = u16::from_le_bytes([bytecode[scan_ip], bytecode[scan_ip + 1]]) as usize;
                scan_ip += 2;
                scan_ip += count * 3; // 1 byte tag + 2 bytes idx
            }
            Op::BuildStruct => {
                scan_ip += 2; // name idx
                let count = u16::from_le_bytes([bytecode[scan_ip], bytecode[scan_ip + 1]]) as usize;
                scan_ip += 2;
                scan_ip += count * 2;
            }
            Op::BuildInterface | Op::UnpackObject => {
                let count = u16::from_le_bytes([bytecode[scan_ip], bytecode[scan_ip + 1]]) as usize;
                scan_ip += 2;
                scan_ip += count * 2;
            }
            Op::FromImport => {
                scan_ip += 2; // src idx
                let count = u16::from_le_bytes([bytecode[scan_ip], bytecode[scan_ip + 1]]) as usize;
                scan_ip += 2;
                scan_ip += count * 4;
            }
            Op::Select => {
                let count = u16::from_le_bytes([bytecode[scan_ip], bytecode[scan_ip + 1]]) as usize;
                scan_ip += 2;
                scan_ip += count;
            }
            Op::Assert => {
                scan_ip += 2;
            }
            _ => {}
        }
    }

    bytecode
}

#[derive(Debug, Clone, PartialEq)]
pub enum OpCode {
    Push(Value),
    Constant(usize),
    Pop,
    Add,
    Sub,
    Div,
    Mul,
    Mod,
    StoreGlobal(Arc<String>),
    LoadGlobal(Arc<String>),
    LoadLocal(usize),
    SetLocal(usize),
    Call(usize),
    TailCall(usize),
    MethodCall(Arc<String>, usize),
    Return,
    Equal,
    Less,
    Greater,
    LessEqual,
    GreaterEqual,
    JumpIfFalse(usize),
    Jump(usize),
    BuildList(usize),
    BuildMap(usize),
    IndexGet,
    ListLen,
    IndexSet,
    And,
    Or,
    Not,
    SetLine(usize),
    Import {
        name: Arc<String>,
        alias: Arc<String>,
    },
    ImportFile {
        path: Arc<String>,
        alias: Arc<String>,
    },
    GetUpvalue(usize),
    SetUpvalue(usize),
    Closure(Arc<FunctionObj>, Arc<Vec<UpvalueLoc>>),
    BuildStruct(Arc<String>, Arc<Vec<String>>),
    AddMethod(Arc<String>),
    BuildInterface(Arc<Vec<String>>),
    CheckIs,
    Assert(Arc<String>),
    BuildTuple(usize),
    UnpackTuple(usize),
    UnpackList(usize),
    UnpackObject(Arc<Vec<String>>),
    FromImport {
        source: Arc<String>,
        symbols: Arc<Vec<(String, String)>>, // (source_key, target_var)
    },
    Spawn(usize),
    ChanSend,
    ChanRecv,
    IterNext(usize),
    PropagateError,
    Select(Arc<Vec<SelectCaseOp>>),
    DeferCall(usize),
    DeferMethodCall(Arc<String>, usize),
    Export(Arc<String>),
    BuildModule(usize),
}
