//! Register-based Dalvik interpreter (MVP opcodes).

use crate::android::decode::{self, Insn};
use crate::android::dex::{CodeItem, DexFile, EncodedMethod};
use crate::android::error::{AndroidError, Result};
use crate::android::hle::{HleHost, Value};
use crate::android::opcodes;

#[derive(Debug)]
struct Frame {
    regs: Vec<Value>,
    code: CodeItem,
    pc: usize,
    #[allow(dead_code)]
    method_idx: u32,
}

#[derive(Debug)]
pub struct Vm {
    pub dex: DexFile,
    pub host: HleHost,
    stack: Vec<Frame>,
    result: Value,
    pub halted: bool,
    pub halt_reason: Option<String>,
    pub steps: u64,
}

impl Vm {
    pub fn new(dex: DexFile) -> Self {
        Self {
            dex,
            host: HleHost::default(),
            stack: Vec::new(),
            result: Value::Int(0),
            halted: false,
            halt_reason: None,
            steps: 0,
        }
    }

    pub fn call_method(&mut self, class: &str, name: &str, args: &[Value]) -> Result<()> {
        let method = self
            .dex
            .find_method(class, name)
            .ok_or_else(|| AndroidError::Vm(format!("method not found: {class}->{name}")))?;
        let method_idx = method.method_idx;
        let code = method
            .code
            .clone()
            .ok_or_else(|| AndroidError::Vm(format!("no code for {class}->{name}")))?;
        self.push_frame(method_idx, code, args)?;
        self.halted = false;
        self.halt_reason = None;
        Ok(())
    }

    fn push_frame(&mut self, method_idx: u32, code: CodeItem, args: &[Value]) -> Result<()> {
        let mut regs = vec![Value::Int(0); code.registers_size as usize];
        // Ins are placed at the end of the register file.
        let start = code
            .registers_size
            .saturating_sub(code.ins_size) as usize;
        for (i, a) in args.iter().enumerate() {
            if start + i < regs.len() {
                regs[start + i] = *a;
            }
        }
        self.stack.push(Frame {
            regs,
            code,
            pc: 0,
            method_idx,
        });
        Ok(())
    }

    pub fn step(&mut self) -> Result<bool> {
        if self.halted {
            return Ok(false);
        }
        let Some(frame_idx) = self.stack.len().checked_sub(1) else {
            self.halted = true;
            return Ok(false);
        };

        let pc = self.stack[frame_idx].pc;
        let insns = self.stack[frame_idx].code.insns.clone();
        let insn = decode::decode(&insns, pc).ok_or_else(|| {
            AndroidError::Vm(format!("decode failed at pc={pc}"))
        })?;

        self.steps += 1;
        self.execute(frame_idx, insn)?;
        Ok(!self.halted && !self.stack.is_empty())
    }

    pub fn run_batch(&mut self, max_steps: u32) -> Result<u32> {
        let mut n = 0u32;
        while n < max_steps {
            if !self.step()? {
                break;
            }
            n += 1;
        }
        Ok(n)
    }

    fn execute(&mut self, frame_idx: usize, insn: Insn) -> Result<()> {
        let op = insn.op;
        match op {
            opcodes::NOP => {
                self.stack[frame_idx].pc += insn.size;
            }
            opcodes::MOVE
            | opcodes::MOVE_FROM16
            | opcodes::MOVE_16
            | opcodes::MOVE_OBJECT
            | opcodes::MOVE_OBJECT_FROM16 => {
                let src = self.reg(frame_idx, insn.b)?;
                self.set_reg(frame_idx, insn.a, src)?;
                self.stack[frame_idx].pc += insn.size;
            }
            opcodes::MOVE_RESULT | opcodes::MOVE_RESULT_OBJECT => {
                let v = self.result;
                self.set_reg(frame_idx, insn.a, v)?;
                self.stack[frame_idx].pc += insn.size;
            }
            opcodes::RETURN_VOID => {
                self.stack.pop();
                if self.stack.is_empty() {
                    self.halted = true;
                }
            }
            opcodes::RETURN | opcodes::RETURN_OBJECT => {
                self.result = self.reg(frame_idx, insn.a)?;
                self.stack.pop();
                if self.stack.is_empty() {
                    self.halted = true;
                }
            }
            opcodes::CONST_4 | opcodes::CONST_16 => {
                let v = insn.b as i16 as i32;
                self.set_reg(frame_idx, insn.a, Value::Int(v))?;
                self.stack[frame_idx].pc += insn.size;
            }
            opcodes::CONST | opcodes::CONST_HIGH16 => {
                let v = if op == opcodes::CONST_HIGH16 {
                    (insn.b as i32) << 16
                } else {
                    insn.b as i32
                };
                self.set_reg(frame_idx, insn.a, Value::Int(v))?;
                self.stack[frame_idx].pc += insn.size;
            }
            opcodes::CONST_STRING | opcodes::CONST_STRING_JUMBO => {
                let s = self
                    .dex
                    .string(insn.b)
                    .ok_or_else(|| AndroidError::Vm("bad string idx".into()))?
                    .to_string();
                let obj = self.host.alloc_string(s);
                self.set_reg(frame_idx, insn.a, obj)?;
                self.stack[frame_idx].pc += insn.size;
            }
            opcodes::NEW_INSTANCE => {
                let ty = self
                    .dex
                    .types
                    .get(insn.b as usize)
                    .cloned()
                    .unwrap_or_else(|| "Ljava/lang/Object;".into());
                let obj = self.host.alloc_instance(ty);
                self.set_reg(frame_idx, insn.a, obj)?;
                self.stack[frame_idx].pc += insn.size;
            }
            opcodes::GOTO | opcodes::GOTO_16 => {
                let off = insn.b as i16 as i32;
                let new_pc = (pc_i32(self.stack[frame_idx].pc) + off) as usize;
                self.stack[frame_idx].pc = new_pc;
            }
            opcodes::IF_EQZ => {
                let v = self.reg(frame_idx, insn.a)?.as_int();
                if v == 0 {
                    let off = insn.b as i16 as i32;
                    self.stack[frame_idx].pc =
                        (pc_i32(self.stack[frame_idx].pc) + off) as usize;
                } else {
                    self.stack[frame_idx].pc += insn.size;
                }
            }
            opcodes::IF_NEZ => {
                let v = self.reg(frame_idx, insn.a)?.as_int();
                if v != 0 {
                    let off = insn.b as i16 as i32;
                    self.stack[frame_idx].pc =
                        (pc_i32(self.stack[frame_idx].pc) + off) as usize;
                } else {
                    self.stack[frame_idx].pc += insn.size;
                }
            }
            opcodes::IF_EQ | opcodes::IF_NE | opcodes::IF_LT | opcodes::IF_GE | opcodes::IF_GT | opcodes::IF_LE => {
                let a = self.reg(frame_idx, insn.a)?.as_int();
                let b = self.reg(frame_idx, insn.b)?.as_int();
                let take = match op {
                    opcodes::IF_EQ => a == b,
                    opcodes::IF_NE => a != b,
                    opcodes::IF_LT => a < b,
                    opcodes::IF_GE => a >= b,
                    opcodes::IF_GT => a > b,
                    opcodes::IF_LE => a <= b,
                    _ => false,
                };
                if take {
                    let off = insn.c as i16 as i32;
                    self.stack[frame_idx].pc =
                        (pc_i32(self.stack[frame_idx].pc) + off) as usize;
                } else {
                    self.stack[frame_idx].pc += insn.size;
                }
            }
            opcodes::CHECK_CAST => {
                // Soft check: keep object as-is
                self.stack[frame_idx].pc += insn.size;
            }
            opcodes::NEW_ARRAY => {
                let len = self.reg(frame_idx, insn.b)?.as_int();
                let arr = self.host.alloc_array(len);
                self.set_reg(frame_idx, insn.a, arr)?;
                self.stack[frame_idx].pc += insn.size;
            }
            opcodes::ARRAY_LENGTH => {
                let arr = self.reg(frame_idx, insn.b)?;
                let len = self.host.array_len(arr).unwrap_or(0);
                self.set_reg(frame_idx, insn.a, Value::Int(len))?;
                self.stack[frame_idx].pc += insn.size;
            }
            opcodes::AGET => {
                let arr = self.reg(frame_idx, insn.b)?;
                let idx = self.reg(frame_idx, insn.c)?.as_int();
                let v = self.host.array_get(arr, idx).unwrap_or(Value::Int(0));
                self.set_reg(frame_idx, insn.a, v)?;
                self.stack[frame_idx].pc += insn.size;
            }
            opcodes::APUT => {
                let val = self.reg(frame_idx, insn.a)?;
                let arr = self.reg(frame_idx, insn.b)?;
                let idx = self.reg(frame_idx, insn.c)?.as_int();
                let _ = self.host.array_set(arr, idx, val);
                self.stack[frame_idx].pc += insn.size;
            }
            opcodes::ADD_INT | opcodes::SUB_INT | opcodes::MUL_INT => {
                let lhs = self.reg(frame_idx, insn.b)?.as_int();
                let rhs = self.reg(frame_idx, insn.c)?.as_int();
                let v = match op {
                    opcodes::ADD_INT => lhs.wrapping_add(rhs),
                    opcodes::SUB_INT => lhs.wrapping_sub(rhs),
                    opcodes::MUL_INT => lhs.wrapping_mul(rhs),
                    _ => 0,
                };
                self.set_reg(frame_idx, insn.a, Value::Int(v))?;
                self.stack[frame_idx].pc += insn.size;
            }
            opcodes::ADD_INT_2ADDR | opcodes::SUB_INT_2ADDR | opcodes::MUL_INT_2ADDR => {
                let lhs = self.reg(frame_idx, insn.a)?.as_int();
                let rhs = self.reg(frame_idx, insn.b)?.as_int();
                let v = match op {
                    opcodes::ADD_INT_2ADDR => lhs.wrapping_add(rhs),
                    opcodes::SUB_INT_2ADDR => lhs.wrapping_sub(rhs),
                    opcodes::MUL_INT_2ADDR => lhs.wrapping_mul(rhs),
                    _ => 0,
                };
                self.set_reg(frame_idx, insn.a, Value::Int(v))?;
                self.stack[frame_idx].pc += insn.size;
            }
            opcodes::IGET_OBJECT | opcodes::IPUT_OBJECT => {
                // Minimal: treat as no-op store/load Int(0) for fixture apps.
                if op == opcodes::IGET_OBJECT {
                    self.set_reg(frame_idx, insn.a, Value::Obj(0))?;
                }
                self.stack[frame_idx].pc += insn.size;
            }
            opcodes::SGET_OBJECT | opcodes::SPUT_OBJECT => {
                if op == opcodes::SGET_OBJECT {
                    // Provide a dummy PrintStream object for System.out
                    let obj = self.host.alloc_instance("Ljava/io/PrintStream;".into());
                    self.set_reg(frame_idx, insn.a, obj)?;
                }
                self.stack[frame_idx].pc += insn.size;
            }
            opcodes::INVOKE_VIRTUAL
            | opcodes::INVOKE_SUPER
            | opcodes::INVOKE_DIRECT
            | opcodes::INVOKE_STATIC
            | opcodes::INVOKE_INTERFACE => {
                self.do_invoke(frame_idx, &insn)?;
            }
            _ => {
                self.halted = true;
                self.halt_reason = Some(format!(
                    "unimplemented opcode 0x{op:02x} at pc={}",
                    self.stack[frame_idx].pc
                ));
                return Err(AndroidError::UnimplementedOpcode {
                    op,
                    pc: self.stack[frame_idx].pc as u32,
                });
            }
        }
        Ok(())
    }

    fn do_invoke(&mut self, frame_idx: usize, insn: &Insn) -> Result<()> {
        let method = self
            .dex
            .method_id(insn.b)
            .ok_or_else(|| AndroidError::Vm("bad method idx".into()))?
            .clone();

        let arg_regs = collect_arg_regs(insn);
        let mut args = Vec::new();
        for r in arg_regs {
            args.push(self.reg(frame_idx, r)?);
        }

        if let Some(ret) = self.host.try_invoke(&method, &args) {
            self.result = ret;
            self.stack[frame_idx].pc += insn.size;
            return Ok(());
        }

        // Call into DEX method if present.
        if let Some(em) = find_encoded(&self.dex, insn.b) {
            if let Some(code) = em.code.clone() {
                self.stack[frame_idx].pc += insn.size;
                self.push_frame(insn.b, code, &args)?;
                return Ok(());
            }
        }

        // Unknown native/framework: stub success.
        self.result = Value::Int(0);
        self.stack[frame_idx].pc += insn.size;
        Ok(())
    }

    fn reg(&self, frame_idx: usize, idx: u32) -> Result<Value> {
        self.stack
            .get(frame_idx)
            .and_then(|f| f.regs.get(idx as usize).copied())
            .ok_or_else(|| AndroidError::Vm(format!("bad reg v{idx}")))
    }

    fn set_reg(&mut self, frame_idx: usize, idx: u32, v: Value) -> Result<()> {
        let frame = self
            .stack
            .get_mut(frame_idx)
            .ok_or_else(|| AndroidError::Vm("no frame".into()))?;
        let slot = frame
            .regs
            .get_mut(idx as usize)
            .ok_or_else(|| AndroidError::Vm(format!("bad reg v{idx}")))?;
        *slot = v;
        Ok(())
    }
}

fn pc_i32(pc: usize) -> i32 {
    pc as i32
}

fn collect_arg_regs(insn: &Insn) -> Vec<u32> {
    let n = insn.a as usize;
    let regs = [insn.c, insn.d, insn.e, insn.f, insn.g];
    regs.into_iter().take(n).collect()
}

fn find_encoded(dex: &DexFile, method_idx: u32) -> Option<&EncodedMethod> {
    for c in &dex.classes {
        for m in c.direct_methods.iter().chain(c.virtual_methods.iter()) {
            if m.method_idx == method_idx {
                return Some(m);
            }
        }
    }
    None
}
