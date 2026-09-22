//! Decode one Dalvik instruction into a structured form.

use crate::android::opcodes;

#[derive(Debug, Clone)]
pub struct Insn {
    pub op: u8,
    pub size: usize, // in code units (u16)
    pub a: u32,
    pub b: u32,
    pub c: u32,
    pub d: u32,
    pub e: u32,
    pub f: u32,
    pub g: u32,
    pub raw0: u16,
}

pub fn decode(insns: &[u16], pc: usize) -> Option<Insn> {
    let raw0 = *insns.get(pc)?;
    let op = (raw0 & 0xff) as u8;
    let mut insn = Insn {
        op,
        size: 1,
        a: 0,
        b: 0,
        c: 0,
        d: 0,
        e: 0,
        f: 0,
        g: 0,
        raw0,
    };

    match op {
        opcodes::NOP | opcodes::RETURN_VOID => {}
        opcodes::MOVE | opcodes::MOVE_OBJECT => {
            insn.a = ((raw0 >> 8) & 0xf) as u32;
            insn.b = ((raw0 >> 12) & 0xf) as u32;
        }
        opcodes::MOVE_FROM16 | opcodes::MOVE_OBJECT_FROM16 => {
            insn.a = ((raw0 >> 8) & 0xff) as u32;
            insn.b = *insns.get(pc + 1)? as u32;
            insn.size = 2;
        }
        opcodes::MOVE_16 => {
            insn.a = *insns.get(pc + 1)? as u32;
            insn.b = *insns.get(pc + 2)? as u32;
            insn.size = 3;
        }
        opcodes::MOVE_RESULT | opcodes::MOVE_RESULT_OBJECT | opcodes::RETURN | opcodes::RETURN_OBJECT => {
            insn.a = ((raw0 >> 8) & 0xff) as u32;
        }
        opcodes::CONST_4 => {
            insn.a = ((raw0 >> 8) & 0xf) as u32;
            let n = ((raw0 >> 12) & 0xf) as i32;
            insn.b = ((n << 28) >> 28) as u32; // sign extend 4-bit
        }
        opcodes::CONST_16 | opcodes::CONST_HIGH16 => {
            insn.a = ((raw0 >> 8) & 0xff) as u32;
            insn.b = *insns.get(pc + 1)? as u32;
            insn.size = 2;
        }
        opcodes::CONST => {
            insn.a = ((raw0 >> 8) & 0xff) as u32;
            let lo = *insns.get(pc + 1)? as u32;
            let hi = *insns.get(pc + 2)? as u32;
            insn.b = lo | (hi << 16);
            insn.size = 3;
        }
        opcodes::CONST_STRING => {
            insn.a = ((raw0 >> 8) & 0xff) as u32;
            insn.b = *insns.get(pc + 1)? as u32;
            insn.size = 2;
        }
        opcodes::CONST_STRING_JUMBO => {
            insn.a = ((raw0 >> 8) & 0xff) as u32;
            let lo = *insns.get(pc + 1)? as u32;
            let hi = *insns.get(pc + 2)? as u32;
            insn.b = lo | (hi << 16);
            insn.size = 3;
        }
        opcodes::NEW_INSTANCE | opcodes::SGET_OBJECT | opcodes::SPUT_OBJECT => {
            insn.a = ((raw0 >> 8) & 0xff) as u32;
            insn.b = *insns.get(pc + 1)? as u32;
            insn.size = 2;
        }
        opcodes::GOTO => {
            let off = ((raw0 >> 8) as i8) as i32;
            insn.b = off as u32;
        }
        opcodes::GOTO_16 => {
            insn.b = *insns.get(pc + 1)? as u32;
            insn.size = 2;
        }
        opcodes::IF_EQZ | opcodes::IF_NEZ => {
            insn.a = ((raw0 >> 8) & 0xff) as u32;
            insn.b = *insns.get(pc + 1)? as u16 as i16 as i32 as u32;
            insn.size = 2;
        }
        opcodes::IF_EQ | opcodes::IF_NE => {
            insn.a = ((raw0 >> 8) & 0xf) as u32;
            insn.b = ((raw0 >> 12) & 0xf) as u32;
            insn.c = *insns.get(pc + 1)? as u16 as i16 as i32 as u32;
            insn.size = 2;
        }
        opcodes::IGET_OBJECT | opcodes::IPUT_OBJECT => {
            insn.a = ((raw0 >> 8) & 0xf) as u32;
            insn.b = ((raw0 >> 12) & 0xf) as u32;
            insn.c = *insns.get(pc + 1)? as u32;
            insn.size = 2;
        }
        opcodes::INVOKE_VIRTUAL
        | opcodes::INVOKE_SUPER
        | opcodes::INVOKE_DIRECT
        | opcodes::INVOKE_STATIC
        | opcodes::INVOKE_INTERFACE => {
            // 35c
            let a = ((raw0 >> 12) & 0xf) as u32; // arg count
            let g = ((raw0 >> 8) & 0xf) as u32;
            let method_idx = *insns.get(pc + 1)? as u32;
            let rest = *insns.get(pc + 2)?;
            insn.a = a;
            insn.b = method_idx;
            insn.c = (rest & 0xf) as u32;
            insn.d = ((rest >> 4) & 0xf) as u32;
            insn.e = ((rest >> 8) & 0xf) as u32;
            insn.f = ((rest >> 12) & 0xf) as u32;
            insn.g = g;
            insn.size = 3;
        }
        _ => {
            // Unknown: consume 1 unit; interp will halt.
        }
    }
    Some(insn)
}
