// ── nekodroid: ARMv7 CPU Emulator Core ─────────────────────────────────
//
// RegisterFile: 16 general-purpose registers + CPSR
// Cpu: owns RegisterFile + Mmu, orchestrates execution

use crate::memory::Mmu;

// ── CPSR bit positions ────────────────────────────────────────────────
// Bits 31-28: Condition flags
const CPSR_N: u32 = 1 << 31; // Negative
const CPSR_Z: u32 = 1 << 30; // Zero
const CPSR_C: u32 = 1 << 29; // Carry
const CPSR_V: u32 = 1 << 28; // Overflow
// Bit 5: Thumb state
const CPSR_T: u32 = 1 << 5;  // Thumb mode (0 = ARM, 1 = Thumb)

// ── Register aliases ──────────────────────────────────────────────────
pub const REG_SP: usize = 13; // Stack Pointer
pub const REG_LR: usize = 14; // Link Register
pub const REG_PC: usize = 15; // Program Counter

/// The ARMv7 register file: 16 general-purpose 32-bit registers + CPSR.
///
/// Register mapping:
///   R0–R12  : General purpose
///   R13 (SP): Stack Pointer
///   R14 (LR): Link Register (return address)
///   R15 (PC): Program Counter
///   CPSR    : Current Program Status Register (flags + state)
pub struct RegisterFile {
    /// General-purpose registers R0–R15
    regs: [u32; 16],
    /// Current Program Status Register
    cpsr: u32,
}

impl RegisterFile {
    /// Creates a new register file with all registers zeroed.
    pub fn new() -> Self {
        RegisterFile {
            regs: [0u32; 16],
            cpsr: 0,
        }
    }

    // ── Register access ───────────────────────────────────────────────

    /// Reads a general-purpose register (0–15).
    pub fn read(&self, reg: usize) -> u32 {
        self.regs[reg & 0xF]
    }

    /// Writes to a general-purpose register (0–15).
    pub fn write(&mut self, reg: usize, val: u32) {
        self.regs[reg & 0xF] = val;
    }

    // ── Convenience accessors ─────────────────────────────────────────

    /// Returns the current Program Counter.
    pub fn pc(&self) -> u32 {
        self.regs[REG_PC]
    }

    /// Sets the Program Counter.
    pub fn set_pc(&mut self, addr: u32) {
        self.regs[REG_PC] = addr;
    }

    /// Returns the current Stack Pointer.
    pub fn sp(&self) -> u32 {
        self.regs[REG_SP]
    }

    /// Sets the Stack Pointer.
    pub fn set_sp(&mut self, addr: u32) {
        self.regs[REG_SP] = addr;
    }

    /// Returns the Link Register.
    pub fn lr(&self) -> u32 {
        self.regs[REG_LR]
    }

    /// Sets the Link Register.
    pub fn set_lr(&mut self, addr: u32) {
        self.regs[REG_LR] = addr;
    }

    // ── CPSR access ───────────────────────────────────────────────────

    /// Returns the raw CPSR value.
    pub fn cpsr(&self) -> u32 {
        self.cpsr
    }

    /// Sets the raw CPSR value.
    pub fn set_cpsr(&mut self, val: u32) {
        self.cpsr = val;
    }

    // ── CPSR flag readers ─────────────────────────────────────────────

    /// Negative flag (bit 31): result was negative.
    pub fn flag_n(&self) -> bool {
        (self.cpsr & CPSR_N) != 0
    }

    /// Zero flag (bit 30): result was zero.
    pub fn flag_z(&self) -> bool {
        (self.cpsr & CPSR_Z) != 0
    }

    /// Carry flag (bit 29): unsigned overflow/borrow.
    pub fn flag_c(&self) -> bool {
        (self.cpsr & CPSR_C) != 0
    }

    /// Overflow flag (bit 28): signed overflow.
    pub fn flag_v(&self) -> bool {
        (self.cpsr & CPSR_V) != 0
    }

    /// Thumb state flag (bit 5): true if executing Thumb instructions.
    pub fn is_thumb(&self) -> bool {
        (self.cpsr & CPSR_T) != 0
    }

    // ── CPSR flag writers ─────────────────────────────────────────────

    /// Sets or clears the Negative flag.
    pub fn set_flag_n(&mut self, val: bool) {
        if val { self.cpsr |= CPSR_N; } else { self.cpsr &= !CPSR_N; }
    }

    /// Sets or clears the Zero flag.
    pub fn set_flag_z(&mut self, val: bool) {
        if val { self.cpsr |= CPSR_Z; } else { self.cpsr &= !CPSR_Z; }
    }

    /// Sets or clears the Carry flag.
    pub fn set_flag_c(&mut self, val: bool) {
        if val { self.cpsr |= CPSR_C; } else { self.cpsr &= !CPSR_C; }
    }

    /// Sets or clears the Overflow flag.
    pub fn set_flag_v(&mut self, val: bool) {
        if val { self.cpsr |= CPSR_V; } else { self.cpsr &= !CPSR_V; }
    }

    /// Sets or clears the Thumb state flag.
    pub fn set_thumb(&mut self, val: bool) {
        if val { self.cpsr |= CPSR_T; } else { self.cpsr &= !CPSR_T; }
    }

    /// Updates N and Z flags based on a 32-bit result.
    /// This is the common update performed after most ALU operations.
    pub fn update_nz(&mut self, result: u32) {
        self.set_flag_n(result & 0x8000_0000 != 0);
        self.set_flag_z(result == 0);
    }
}

// ── The CPU ───────────────────────────────────────────────────────────

/// The ARM CPU: owns the register file and memory bus.
///
/// This is the top-level struct that will eventually execute instructions,
/// handle interrupts, and manage the full CPU pipeline.
pub struct Cpu {
    /// The register file (R0–R15 + CPSR)
    pub regs: RegisterFile,
    /// The memory bus (RAM)
    pub mmu: Mmu,
    /// Whether the CPU is halted
    pub halted: bool,
}

impl Cpu {
    /// Creates a new CPU with the given RAM size.
    pub fn new(ram_size: usize) -> Self {
        Cpu {
            regs: RegisterFile::new(),
            mmu: Mmu::new(ram_size),
            halted: false,
        }
    }

    /// Creates a new CPU with the default 16 MB RAM.
    pub fn default() -> Self {
        Cpu {
            regs: RegisterFile::new(),
            mmu: Mmu::default(),
            halted: false,
        }
    }

    /// Resets the CPU to initial state: all registers zeroed, RAM cleared.
    pub fn reset(&mut self) {
        self.regs = RegisterFile::new();
        self.halted = false;
    }

    /// Fetches the next instruction word from memory at the current PC.
    pub fn fetch(&self) -> u32 {
        let pc = self.regs.pc();
        if self.regs.is_thumb() {
            self.mmu.read_u16(pc) as u32
        } else {
            self.mmu.read_u32(pc)
        }
    }

    /// Advances the PC by one instruction width.
    pub fn advance_pc(&mut self) {
        let step = if self.regs.is_thumb() { 2 } else { 4 };
        let new_pc = self.regs.pc().wrapping_add(step);
        self.regs.set_pc(new_pc);
    }

    /// Loads a program binary into memory at the given base address
    /// and sets the PC to that address.
    pub fn load_program(&mut self, base_addr: u32, program: &[u8]) {
        self.mmu.load_bytes(base_addr, program);
        self.regs.set_pc(base_addr);
    }

    // ── Fetch-Decode-Execute ──────────────────────────────────────────

    /// Executes one instruction cycle: fetch → decode → execute.
    /// Returns true if the CPU executed an instruction, false if halted.
    pub fn step(&mut self) -> bool {
        if self.halted {
            return false;
        }

        // ── FETCH ─────────────────────────────────────────────────────
        let instr = self.fetch();
        let pc_at_fetch = self.regs.pc();
        self.advance_pc();

        // ── CONDITION CHECK ───────────────────────────────────────────
        // ARM instructions bits [31:28] are the condition code.
        // If the condition is not met, the instruction is a NOP.
        if !self.check_condition(instr) {
            return true; // Instruction skipped, but CPU is not halted
        }

        // ── DECODE & EXECUTE ──────────────────────────────────────────
        // Top-level decode using bits [27:25]
        let bits_27_25 = (instr >> 25) & 0b111;

        match bits_27_25 {
            // 000 = Data Processing (register) / Multiply / Misc
            0b000 => self.execute_data_processing(instr),
            // 001 = Data Processing (immediate)
            0b001 => self.execute_data_processing(instr),
            // 010 = Load/Store (immediate offset)
            0b010 => self.execute_load_store_stub(instr, pc_at_fetch),
            // 011 = Load/Store (register offset)
            0b011 => self.execute_load_store_stub(instr, pc_at_fetch),
            // 100 = Load/Store Multiple
            0b100 => self.log_unimplemented("Load/Store Multiple", instr, pc_at_fetch),
            // 101 = Branch (B / BL)
            0b101 => self.execute_branch(instr, pc_at_fetch),
            // 110 = Coprocessor
            0b110 => self.log_unimplemented("Coprocessor", instr, pc_at_fetch),
            // 111 = Software interrupt / Coprocessor
            0b111 => self.log_unimplemented("SWI/Coprocessor", instr, pc_at_fetch),
            _ => unreachable!(),
        }

        true
    }

    // ── Condition code evaluation ─────────────────────────────────────

    /// Checks the ARM condition code (bits [31:28]) against CPSR flags.
    /// Returns true if the instruction should execute.
    fn check_condition(&self, instr: u32) -> bool {
        let cond = (instr >> 28) & 0xF;
        let n = self.regs.flag_n();
        let z = self.regs.flag_z();
        let c = self.regs.flag_c();
        let v = self.regs.flag_v();

        match cond {
            0x0 => z,                          // EQ — Equal (Z set)
            0x1 => !z,                         // NE — Not equal (Z clear)
            0x2 => c,                          // CS/HS — Carry set
            0x3 => !c,                         // CC/LO — Carry clear
            0x4 => n,                          // MI — Negative
            0x5 => !n,                         // PL — Positive or zero
            0x6 => v,                          // VS — Overflow
            0x7 => !v,                         // VC — No overflow
            0x8 => c && !z,                    // HI — Unsigned higher
            0x9 => !c || z,                    // LS — Unsigned lower or same
            0xA => n == v,                     // GE — Signed >=
            0xB => n != v,                     // LT — Signed <
            0xC => !z && (n == v),             // GT — Signed >
            0xD => z || (n != v),              // LE — Signed <=
            0xE => true,                       // AL — Always
            0xF => true,                       // Unconditional (ARMv5+)
            _ => unreachable!(),
        }
    }

    // ── Data Processing ───────────────────────────────────────────────

    /// Decodes and executes a Data Processing instruction.
    ///
    /// ARM encoding:  cond | 00 | I | opcode | S | Rn | Rd | operand2
    ///   I (bit 25): 1 = immediate, 0 = register
    ///   opcode (bits [24:21]): ALU operation
    ///   S (bit 20): 1 = update CPSR flags
    ///   Rn (bits [19:16]): first operand register
    ///   Rd (bits [15:12]): destination register
    fn execute_data_processing(&mut self, instr: u32) {
        let is_imm = (instr >> 25) & 1 == 1;
        let opcode = (instr >> 21) & 0xF;
        let set_flags = (instr >> 20) & 1 == 1;
        let rn = ((instr >> 16) & 0xF) as usize;
        let rd = ((instr >> 12) & 0xF) as usize;

        // Compute operand2
        let op2 = if is_imm {
            // Immediate: 8-bit value rotated right by 2 * rotate
            let imm8 = instr & 0xFF;
            let rotate = ((instr >> 8) & 0xF) * 2;
            imm8.rotate_right(rotate)
        } else {
            // Register: Rm (bits [3:0]) — simplified, no shift for now
            let rm = (instr & 0xF) as usize;
            self.regs.read(rm)
        };

        let rn_val = self.regs.read(rn);

        // Execute based on opcode
        match opcode {
            // 0000 = AND
            0x0 => {
                let result = rn_val & op2;
                self.regs.write(rd, result);
                if set_flags { self.regs.update_nz(result); }
            }
            // 0001 = EOR (XOR)
            0x1 => {
                let result = rn_val ^ op2;
                self.regs.write(rd, result);
                if set_flags { self.regs.update_nz(result); }
            }
            // 0010 = SUB
            0x2 => {
                let result = rn_val.wrapping_sub(op2);
                self.regs.write(rd, result);
                if set_flags {
                    self.regs.update_nz(result);
                    self.regs.set_flag_c(rn_val >= op2); // borrow
                    let overflow = ((rn_val ^ op2) & (rn_val ^ result)) >> 31 != 0;
                    self.regs.set_flag_v(overflow);
                }
            }
            // 0100 = ADD
            0x4 => {
                let result = rn_val.wrapping_add(op2);
                self.regs.write(rd, result);
                if set_flags {
                    self.regs.update_nz(result);
                    self.regs.set_flag_c(result < rn_val || result < op2);
                    let overflow = (!((rn_val ^ op2)) & (rn_val ^ result)) >> 31 != 0;
                    self.regs.set_flag_v(overflow);
                }
            }
            // 1010 = CMP (compare — like SUB but result discarded)
            0xA => {
                let result = rn_val.wrapping_sub(op2);
                // CMP always updates flags, result not stored
                self.regs.update_nz(result);
                self.regs.set_flag_c(rn_val >= op2);
                let overflow = ((rn_val ^ op2) & (rn_val ^ result)) >> 31 != 0;
                self.regs.set_flag_v(overflow);
            }
            // 1100 = ORR
            0xC => {
                let result = rn_val | op2;
                self.regs.write(rd, result);
                if set_flags { self.regs.update_nz(result); }
            }
            // 1101 = MOV (Rd = op2, Rn ignored)
            0xD => {
                self.regs.write(rd, op2);
                if set_flags { self.regs.update_nz(op2); }
            }
            // 1110 = BIC (bit clear: Rd = Rn AND NOT op2)
            0xE => {
                let result = rn_val & !op2;
                self.regs.write(rd, result);
                if set_flags { self.regs.update_nz(result); }
            }
            // 1111 = MVN (move NOT: Rd = NOT op2)
            0xF => {
                let result = !op2;
                self.regs.write(rd, result);
                if set_flags { self.regs.update_nz(result); }
            }
            _ => {
                // Unimplemented data processing opcode
            }
        }
    }

    // ── Branch ────────────────────────────────────────────────────────

    /// Executes a Branch (B) or Branch with Link (BL) instruction.
    ///
    /// ARM encoding:  cond | 101 | L | offset24
    ///   L (bit 24): 1 = BL (saves return address in LR)
    ///   offset24: signed 24-bit offset, shifted left 2, added to PC+8
    fn execute_branch(&mut self, instr: u32, pc_at_fetch: u32) {
        let link = (instr >> 24) & 1 == 1;

        // Sign-extend the 24-bit offset to 32 bits
        let offset24 = instr & 0x00FF_FFFF;
        let offset = if offset24 & 0x0080_0000 != 0 {
            // Negative: sign-extend by filling upper 8 bits with 1
            (offset24 | 0xFF00_0000) << 2
        } else {
            offset24 << 2
        };

        // Branch target: PC at fetch + 8 (pipeline) + offset
        // In ARM, the PC reads as current instruction + 8 due to pipeline
        let target = pc_at_fetch.wrapping_add(8).wrapping_add(offset);

        if link {
            // BL: save return address (next instruction after this one)
            self.regs.set_lr(pc_at_fetch.wrapping_add(4));
        }

        self.regs.set_pc(target);
    }

    // ── Load/Store stub ───────────────────────────────────────────────

    fn execute_load_store_stub(&mut self, instr: u32, pc: u32) {
        self.log_unimplemented("Load/Store", instr, pc);
    }

    fn log_unimplemented(&self, category: &str, instr: u32, pc: u32) {
        #[cfg(not(test))]
        {
            // Only log in non-test builds to avoid noise
            let _ = (category, instr, pc);
        }
        #[cfg(test)]
        {
            panic!(
                "Unimplemented {} instruction: {:#010X} at PC {:#010X}",
                category, instr, pc
            );
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: creates a CPU and loads a program at address 0.
    fn cpu_with_program(program: &[u8]) -> Cpu {
        let mut cpu = Cpu::new(4096);
        cpu.load_program(0, program);
        cpu
    }

    // ── RegisterFile tests ────────────────────────────────────────────

    #[test]
    fn test_register_read_write() {
        let mut rf = RegisterFile::new();
        rf.write(0, 0xDEADBEEF);
        assert_eq!(rf.read(0), 0xDEADBEEF);
        rf.write(REG_PC, 0x8000);
        assert_eq!(rf.pc(), 0x8000);
    }

    #[test]
    fn test_sp_lr_pc() {
        let mut rf = RegisterFile::new();
        rf.set_sp(0x7FFF_0000);
        rf.set_lr(0x0000_1234);
        rf.set_pc(0x0000_8000);
        assert_eq!(rf.sp(), 0x7FFF_0000);
        assert_eq!(rf.lr(), 0x0000_1234);
        assert_eq!(rf.pc(), 0x0000_8000);
    }

    #[test]
    fn test_cpsr_flags() {
        let mut rf = RegisterFile::new();
        assert!(!rf.flag_n());
        assert!(!rf.flag_z());

        rf.set_flag_n(true);
        rf.set_flag_z(true);
        rf.set_flag_c(true);
        rf.set_flag_v(true);
        assert!(rf.flag_n());
        assert!(rf.flag_z());
        assert!(rf.flag_c());
        assert!(rf.flag_v());

        rf.set_flag_n(false);
        assert!(!rf.flag_n());
        assert!(rf.flag_z());
    }

    #[test]
    fn test_thumb_mode() {
        let mut rf = RegisterFile::new();
        assert!(!rf.is_thumb());
        rf.set_thumb(true);
        assert!(rf.is_thumb());
        rf.set_thumb(false);
        assert!(!rf.is_thumb());
    }

    #[test]
    fn test_update_nz() {
        let mut rf = RegisterFile::new();
        rf.update_nz(0);
        assert!(rf.flag_z());
        assert!(!rf.flag_n());
        rf.update_nz(0x8000_0000);
        assert!(!rf.flag_z());
        assert!(rf.flag_n());
    }

    // ── CPU fetch/advance tests ───────────────────────────────────────

    #[test]
    fn test_cpu_fetch_arm() {
        let mut cpu = Cpu::new(1024);
        cpu.mmu.write_u32(0, 0xE3A01001); // MOV R1, #1
        cpu.regs.set_pc(0);
        assert_eq!(cpu.fetch(), 0xE3A01001);
    }

    #[test]
    fn test_cpu_fetch_thumb() {
        let mut cpu = Cpu::new(1024);
        cpu.mmu.write_u16(0, 0x2001);
        cpu.regs.set_pc(0);
        cpu.regs.set_thumb(true);
        assert_eq!(cpu.fetch(), 0x2001);
    }

    #[test]
    fn test_cpu_advance_pc() {
        let mut cpu = Cpu::new(1024);
        cpu.regs.set_pc(0x100);
        cpu.advance_pc();
        assert_eq!(cpu.regs.pc(), 0x104);
        cpu.regs.set_thumb(true);
        cpu.advance_pc();
        assert_eq!(cpu.regs.pc(), 0x106);
    }

    #[test]
    fn test_cpu_load_program() {
        let mut cpu = Cpu::new(1024);
        let prog = [0x01, 0x10, 0xA0, 0xE3]; // MOV R1, #1 (LE)
        cpu.load_program(0x200, &prog);
        assert_eq!(cpu.regs.pc(), 0x200);
        assert_eq!(cpu.mmu.read_u32(0x200), 0xE3A01001);
    }

    // ── ALU execution tests ───────────────────────────────────────────

    #[test]
    fn test_basic_alu() {
        // MOV R0, #5   →  E3A00005
        // ADD R1, R0, #10 → E280100A
        //
        // ARM encoding for MOV R0, #5:
        //   cond=1110(AL) 001 opcode=1101 S=0 Rn=0000 Rd=0000 rotate=0000 imm8=00000101
        //   = 0xE3A00005
        //
        // ARM encoding for ADD R1, R0, #10:
        //   cond=1110(AL) 001 opcode=0100 S=0 Rn=0000 Rd=0001 rotate=0000 imm8=00001010
        //   = 0xE280100A
        let program: Vec<u8> = [
            0xE3A00005u32.to_le_bytes(), // MOV R0, #5
            0xE280100Au32.to_le_bytes(), // ADD R1, R0, #10
        ].concat();

        let mut cpu = cpu_with_program(&program);
        cpu.step(); // MOV R0, #5
        assert_eq!(cpu.regs.read(0), 5, "R0 should be 5 after MOV R0, #5");

        cpu.step(); // ADD R1, R0, #10
        assert_eq!(cpu.regs.read(1), 15, "R1 should be 15 after ADD R1, R0, #10");
    }

    #[test]
    fn test_mov_register() {
        // MOV R0, #42  → E3A0002A
        // MOV R1, R0   → E1A01000 (register form: I=0, opcode=MOV, Rm=R0)
        let program: Vec<u8> = [
            0xE3A0002Au32.to_le_bytes(), // MOV R0, #42
            0xE1A01000u32.to_le_bytes(), // MOV R1, R0
        ].concat();

        let mut cpu = cpu_with_program(&program);
        cpu.step();
        cpu.step();
        assert_eq!(cpu.regs.read(1), 42);
    }

    #[test]
    fn test_sub_instruction() {
        // MOV R0, #20  → E3A00014
        // SUB R1, R0, #5 → E2401005
        //   cond=AL 001 opcode=0010(SUB) S=0 Rn=R0 Rd=R1 imm=5
        let program: Vec<u8> = [
            0xE3A00014u32.to_le_bytes(), // MOV R0, #20
            0xE2401005u32.to_le_bytes(), // SUB R1, R0, #5
        ].concat();

        let mut cpu = cpu_with_program(&program);
        cpu.step();
        cpu.step();
        assert_eq!(cpu.regs.read(1), 15);
    }

    #[test]
    fn test_cmp_sets_flags() {
        // MOV R0, #5   → E3A00005
        // CMP R0, #5   → E3500005
        //   cond=AL 001 opcode=1010(CMP) S=1 Rn=R0 Rd=0 imm=5
        let program: Vec<u8> = [
            0xE3A00005u32.to_le_bytes(), // MOV R0, #5
            0xE3500005u32.to_le_bytes(), // CMP R0, #5
        ].concat();

        let mut cpu = cpu_with_program(&program);
        cpu.step(); // MOV
        cpu.step(); // CMP
        assert!(cpu.regs.flag_z(), "Z flag should be set (5 - 5 = 0)");
        assert!(!cpu.regs.flag_n(), "N flag should be clear");
    }

    // ── Branch tests ──────────────────────────────────────────────────

    #[test]
    fn test_branch_forward() {
        // B +8   → EA000000
        //   cond=AL 101 L=0 offset=0x000000
        //   target = PC_fetch + 8 + (0 << 2) = 0 + 8 = 8
        //   (skip 1 instruction)
        let program: Vec<u8> = [
            0xEA000000u32.to_le_bytes(), // B +8 (branch to addr 8)
            0xE3A00001u32.to_le_bytes(), // MOV R0, #1 (should be skipped)
            0xE3A01002u32.to_le_bytes(), // MOV R1, #2 (branch target)
        ].concat();

        let mut cpu = cpu_with_program(&program);
        cpu.step(); // B +8
        assert_eq!(cpu.regs.pc(), 8, "PC should jump to 8");

        cpu.step(); // MOV R1, #2
        assert_eq!(cpu.regs.read(1), 2);
        assert_eq!(cpu.regs.read(0), 0, "R0 should still be 0 (MOV R0,#1 was skipped)");
    }

    #[test]
    fn test_branch_backward() {
        // Set up a tiny loop:
        // 0x00: MOV R0, #0       → E3A00000
        // 0x04: ADD R0, R0, #1   → E2800001
        // 0x08: B -8 (back to 0x04) → EAFFFFFD
        //   offset = -2 in instruction words → 0xFFFFFD sign-extended
        //   target = 0x08 + 8 + (0x3FFFFFD << 2)... let me compute:
        //   24-bit: 0xFFFFFD, sign-extend → 0xFFFFFFFD, <<2 → 0xFFFFFFF4
        //   target = 0x08 + 8 + 0xFFFFFFF4 = 0x04 ✓
        let program: Vec<u8> = [
            0xE3A00000u32.to_le_bytes(), // MOV R0, #0
            0xE2800001u32.to_le_bytes(), // ADD R0, R0, #1
            0xEAFFFFFDu32.to_le_bytes(), // B back to 0x04
        ].concat();

        let mut cpu = cpu_with_program(&program);
        cpu.step(); // MOV R0, #0
        assert_eq!(cpu.regs.read(0), 0);

        cpu.step(); // ADD R0, R0, #1 → R0 = 1
        assert_eq!(cpu.regs.read(0), 1);

        cpu.step(); // B back to 0x04
        assert_eq!(cpu.regs.pc(), 0x04, "PC should loop back to 0x04");

        cpu.step(); // ADD R0, R0, #1 → R0 = 2
        assert_eq!(cpu.regs.read(0), 2);
    }

    // ── Condition code tests ──────────────────────────────────────────

    #[test]
    fn test_conditional_execution() {
        // MOV R0, #5     → E3A00005  (AL)
        // CMP R0, #5     → E3500005  (AL, sets Z=1)
        // MOVEQ R1, #99  → 03A01063  (EQ — executes because Z=1)
        // MOVNE R2, #77  → 13A0204D  (NE — skipped because Z=1)
        let program: Vec<u8> = [
            0xE3A00005u32.to_le_bytes(), // MOV R0, #5
            0xE3500005u32.to_le_bytes(), // CMP R0, #5
            0x03A01063u32.to_le_bytes(), // MOVEQ R1, #99
            0x13A0204Du32.to_le_bytes(), // MOVNE R2, #77
        ].concat();

        let mut cpu = cpu_with_program(&program);
        cpu.step(); // MOV
        cpu.step(); // CMP → Z=1
        cpu.step(); // MOVEQ R1, #99 → executes (Z=1)
        cpu.step(); // MOVNE R2, #77 → skipped (Z=1, NE needs Z=0)

        assert_eq!(cpu.regs.read(1), 99, "R1 should be 99 (EQ condition met)");
        assert_eq!(cpu.regs.read(2), 0, "R2 should be 0 (NE condition NOT met)");
    }
}

