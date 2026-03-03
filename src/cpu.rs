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
// Bit 7: IRQ disable
const CPSR_I: u32 = 1 << 7;  // IRQ disabled
// Bit 5: Thumb state
const CPSR_T: u32 = 1 << 5;  // Thumb mode (0 = ARM, 1 = Thumb)
// Bits [4:0]: CPU mode
const CPSR_MODE_MASK: u32 = 0x1F;

// ARM CPU modes
const MODE_USER: u32 = 0x10;  // User mode
const MODE_SVC:  u32 = 0x13;  // Supervisor mode (SWI handler)

// Exception vector addresses
const SWI_VECTOR: u32 = 0x0000_0008;

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
    /// Saved Program Status Register (Supervisor mode)
    spsr_svc: u32,
}

impl RegisterFile {
    /// Creates a new register file with all registers zeroed.
    pub fn new() -> Self {
        RegisterFile {
            regs: [0u32; 16],
            cpsr: MODE_USER, // Start in User mode
            spsr_svc: 0,
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
    pub fn update_nz(&mut self, result: u32) {
        self.set_flag_n(result & 0x8000_0000 != 0);
        self.set_flag_z(result == 0);
    }

    // ── CPU mode ──────────────────────────────────────────────────────

    /// Returns the CPU mode (bits [4:0] of CPSR).
    pub fn cpu_mode(&self) -> u32 {
        self.cpsr & CPSR_MODE_MASK
    }

    /// Sets the CPU mode (bits [4:0] of CPSR).
    pub fn set_cpu_mode(&mut self, mode: u32) {
        self.cpsr = (self.cpsr & !CPSR_MODE_MASK) | (mode & CPSR_MODE_MASK);
    }

    /// Returns true if IRQ interrupts are disabled.
    pub fn irq_disabled(&self) -> bool {
        (self.cpsr & CPSR_I) != 0
    }

    /// Sets or clears the IRQ disable bit.
    pub fn set_irq_disabled(&mut self, val: bool) {
        if val { self.cpsr |= CPSR_I; } else { self.cpsr &= !CPSR_I; }
    }

    /// Returns the Supervisor mode SPSR.
    pub fn spsr_svc(&self) -> u32 {
        self.spsr_svc
    }

    /// Sets the Supervisor mode SPSR.
    pub fn set_spsr_svc(&mut self, val: u32) {
        self.spsr_svc = val;
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

    // ── Disassembler ──────────────────────────────────────────────────

    /// Register name lookup
    fn reg_name(r: u32) -> &'static str {
        match r {
            0 => "R0", 1 => "R1", 2 => "R2", 3 => "R3",
            4 => "R4", 5 => "R5", 6 => "R6", 7 => "R7",
            8 => "R8", 9 => "R9", 10 => "R10", 11 => "R11",
            12 => "R12", 13 => "SP", 14 => "LR", 15 => "PC",
            _ => "??",
        }
    }

    /// Condition suffix string
    fn cond_suffix(cond: u32) -> &'static str {
        match cond {
            0x0 => "EQ", 0x1 => "NE", 0x2 => "CS", 0x3 => "CC",
            0x4 => "MI", 0x5 => "PL", 0x6 => "VS", 0x7 => "VC",
            0x8 => "HI", 0x9 => "LS", 0xA => "GE", 0xB => "LT",
            0xC => "GT", 0xD => "LE", 0xE => "",   0xF => "",
            _ => "",
        }
    }

    /// Disassembles a single 32-bit ARM instruction into human-readable assembly.
    pub fn disassemble_instruction(instr: u32) -> String {
        if instr == 0 {
            return "NOP (0x00000000)".to_string();
        }

        let cond = (instr >> 28) & 0xF;
        let cs = Self::cond_suffix(cond);
        let bits_27_25 = (instr >> 25) & 0b111;

        match bits_27_25 {
            // Data Processing
            0b000 | 0b001 => {
                // Check for Multiply: bits [7:4] = 1001, bits [27:22] = 000000 or 000001
                if bits_27_25 == 0b000 && (instr & 0x0FC0_00F0) == 0x0000_0090 {
                    let a = (instr >> 21) & 1 == 1;
                    let rd = (instr >> 16) & 0xF;
                    let rn = (instr >> 12) & 0xF;
                    let rs = (instr >> 8) & 0xF;
                    let rm = instr & 0xF;
                    if a {
                        return format!("MLA{} {}, {}, {}, {}", cs,
                            Self::reg_name(rd), Self::reg_name(rm),
                            Self::reg_name(rs), Self::reg_name(rn));
                    } else {
                        return format!("MUL{} {}, {}, {}", cs,
                            Self::reg_name(rd), Self::reg_name(rm),
                            Self::reg_name(rs));
                    }
                }
                // Check for BLX (register)
                if bits_27_25 == 0b000 && (instr & 0x0FFF_FFF0) == 0x012F_FF30 {
                    let rm = instr & 0xF;
                    return format!("BLX{} {}", cs, Self::reg_name(rm));
                }
                // Check for BX
                if bits_27_25 == 0b000 && (instr & 0x0FFF_FFF0) == 0x012F_FF10 {
                    let rm = instr & 0xF;
                    return format!("BX{} {}", cs, Self::reg_name(rm));
                }
                // Check for halfword/signed transfers
                if bits_27_25 == 0b000 && (instr & 0x90) == 0x90 && (instr & 0x0E000000) == 0
                    && (instr & 0x0FC000F0) != 0x00000090 {
                    let pre   = (instr >> 24) & 1 == 1;
                    let up    = (instr >> 23) & 1 == 1;
                    let load  = (instr >> 20) & 1 == 1;
                    let s_bit = (instr >> 6) & 1 == 1;
                    let h_bit = (instr >> 5) & 1 == 1;
                    let rn = (instr >> 16) & 0xF;
                    let rd = (instr >> 12) & 0xF;
                    let mnemonic = if !load {
                        "STRH"
                    } else if s_bit && h_bit {
                        "LDRSH"
                    } else if s_bit {
                        "LDRSB"
                    } else {
                        "LDRH"
                    };
                    let sign = if up { "+" } else { "-" };
                    let imm = (instr >> 22) & 1 == 1;
                    let off_str = if imm {
                        let off = ((instr >> 8) & 0xF) << 4 | (instr & 0xF);
                        format!("#{}0x{:X}", sign, off)
                    } else {
                        let rm = instr & 0xF;
                        format!("{}{}", sign, Self::reg_name(rm))
                    };
                    let addr_str = if pre {
                        format!("[{}, {}]", Self::reg_name(rn), off_str)
                    } else {
                        format!("[{}], {}", Self::reg_name(rn), off_str)
                    };
                    return format!("{}{} {}, {}", mnemonic, cs, Self::reg_name(rd), addr_str);
                }
                let is_imm = (instr >> 25) & 1 == 1;
                let opcode = (instr >> 21) & 0xF;
                let s = if (instr >> 20) & 1 == 1 { "S" } else { "" };
                let rn = (instr >> 16) & 0xF;
                let rd = (instr >> 12) & 0xF;

                let op2_str = if is_imm {
                    let imm8 = instr & 0xFF;
                    let rotate = ((instr >> 8) & 0xF) * 2;
                    let val = imm8.rotate_right(rotate);
                    format!("#{}", val)
                } else {
                    let rm = instr & 0xF;
                    let shift_type = (instr >> 5) & 0x3;
                    let shift_amount = (instr >> 7) & 0x1F;
                    let shift_name = match shift_type {
                        0 => "LSL", 1 => "LSR", 2 => "ASR", 3 => "ROR", _ => "??",
                    };
                    if shift_amount == 0 {
                        Self::reg_name(rm).to_string()
                    } else {
                        format!("{}, {} #{}", Self::reg_name(rm), shift_name, shift_amount)
                    }
                };

                let mnemonic = match opcode {
                    0x0 => "AND", 0x1 => "EOR", 0x2 => "SUB", 0x3 => "RSB",
                    0x4 => "ADD", 0x5 => "ADC", 0x6 => "SBC", 0x7 => "RSC",
                    0x8 => "TST", 0x9 => "TEQ", 0xA => "CMP", 0xB => "CMN",
                    0xC => "ORR", 0xD => "MOV", 0xE => "BIC", 0xF => "MVN",
                    _ => "???",
                };

                match opcode {
                    // MOV, MVN: Rd, op2 (no Rn)
                    0xD | 0xF => format!("{}{}{} {}, {}", mnemonic, cs, s, Self::reg_name(rd), op2_str),
                    // CMP, CMN, TST, TEQ: Rn, op2 (no Rd, always sets flags)
                    0x8 | 0x9 | 0xA | 0xB => format!("{}{} {}, {}", mnemonic, cs, Self::reg_name(rn), op2_str),
                    // Normal 3-operand: Rd, Rn, op2
                    _ => format!("{}{}{} {}, {}, {}", mnemonic, cs, s, Self::reg_name(rd), Self::reg_name(rn), op2_str),
                }
            }
            // LDR / STR
            0b010 | 0b011 => {
                let p = (instr >> 24) & 1 == 1;
                let u = (instr >> 23) & 1 == 1;
                let b = (instr >> 22) & 1 == 1;
                let w = (instr >> 21) & 1 == 1;
                let l = (instr >> 20) & 1 == 1;
                let rn = (instr >> 16) & 0xF;
                let rd = (instr >> 12) & 0xF;

                let mnemonic = if l {
                    if b { "LDRB" } else { "LDR" }
                } else {
                    if b { "STRB" } else { "STR" }
                };

                let offset = instr & 0xFFF;
                let sign = if u { "" } else { "-" };
                let wb = if w && p { "!" } else { "" };

                if p {
                    if offset == 0 {
                        format!("{}{} {}, [{}]{}", mnemonic, cs, Self::reg_name(rd), Self::reg_name(rn), wb)
                    } else {
                        format!("{}{} {}, [{}, #{}{}]{}", mnemonic, cs, Self::reg_name(rd), Self::reg_name(rn), sign, offset, wb)
                    }
                } else {
                    format!("{}{} {}, [{}], #{}{}", mnemonic, cs, Self::reg_name(rd), Self::reg_name(rn), sign, offset)
                }
            }
            // LDM / STM
            0b100 => {
                let p = (instr >> 24) & 1 == 1;
                let u = (instr >> 23) & 1 == 1;
                let w = (instr >> 21) & 1 == 1;
                let l = (instr >> 20) & 1 == 1;
                let rn = (instr >> 16) & 0xF;
                let reg_list = instr & 0xFFFF;

                let mode = match (p, u) {
                    (false, true) => "IA",
                    (true, true)  => "IB",
                    (false, false) => "DA",
                    (true, false)  => "DB",
                };

                let mnemonic = if l { "LDM" } else { "STM" };
                let wb = if w { "!" } else { "" };

                let mut regs = Vec::new();
                for i in 0..16u32 {
                    if reg_list & (1 << i) != 0 {
                        regs.push(Self::reg_name(i).to_string());
                    }
                }

                format!("{}{}{} {}{}, {{{}}}", mnemonic, cs, mode, Self::reg_name(rn), wb, regs.join(", "))
            }
            // Branch
            0b101 => {
                let link = (instr >> 24) & 1 == 1;
                let mnemonic = if link { "BL" } else { "B" };
                let offset24 = instr & 0x00FF_FFFF;
                let offset = if offset24 & 0x0080_0000 != 0 {
                    ((offset24 | 0xFF00_0000) << 2) as i32
                } else {
                    (offset24 << 2) as i32
                };
                // Offset is relative to PC+8
                format!("{}{} #{:+}", mnemonic, cs, offset.wrapping_add(8))
            }
            // SWI
            0b111 => {
                if (instr >> 24) & 1 == 1 {
                    let swi_num = instr & 0x00FF_FFFF;
                    format!("SWI{} #0x{:06X}", cs, swi_num)
                } else {
                    format!("CDP{} (Coprocessor)", cs)
                }
            }
            _ => format!("??? ({:#010X})", instr),
        }
    }

    /// Disassembles the instruction at the given memory address.
    pub fn disassemble_at(&self, addr: u32) -> String {
        let instr = self.mmu.read_u32(addr);
        Self::disassemble_instruction(instr)
    }

    // ── Fetch-Decode-Execute ──────────────────────────────────────────

    /// Executes one instruction cycle: fetch → decode → execute.
    /// Returns true if the CPU executed an instruction, false if halted.
    pub fn step(&mut self) -> bool {
        if self.halted {
            return false;
        }

        // ── HLE BIOS Intercept ────────────────────────────────────────
        // If the PC has reached the SWI vector (0x08) AND we are in Supervisor mode,
        // intercept execution to handle the syscall in Rust instead of executing ARM code.
        if self.regs.pc() == SWI_VECTOR && self.regs.cpu_mode() == MODE_SVC {
            self.handle_bios_syscall();
            return true;
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
            0b000 => {
                // Check for Multiply: bits [7:4] = 1001 and bits [27:22] = 000000 or 000001
                if (instr & 0x0FC0_00F0) == 0x0000_0090 {
                    self.execute_multiply(instr);
                }
                // Check for BLX (register): bits [27:4] = 0x012FFF3
                else if (instr & 0x0FFF_FFF0) == 0x012F_FF30 {
                    self.execute_blx_register(instr, pc_at_fetch);
                }
                // Check for BX: bits [27:4] = 0x012FFF1
                else if (instr & 0x0FFF_FFF0) == 0x012F_FF10 {
                    self.execute_branch_exchange(instr);
                }
                // Check for Halfword/Signed transfers: bit[7]=1, bit[4]=1 (not multiply)
                else if (instr & 0x90) == 0x90 && (instr & 0x0E000000) == 0 {
                    // Extra load/stores: LDRH, STRH, LDRSB, LDRSH
                    self.execute_halfword_transfer(instr);
                }
                else {
                    self.execute_data_processing(instr);
                }
            }
            // 001 = Data Processing (immediate)
            0b001 => self.execute_data_processing(instr),
            // 010 = Load/Store (immediate offset)
            0b010 => self.execute_single_data_transfer(instr),
            // 011 = Load/Store (register offset)
            0b011 => self.execute_single_data_transfer(instr),
            // 100 = Load/Store Multiple (LDM / STM)
            0b100 => self.execute_block_data_transfer(instr),
            // 101 = Branch (B / BL)
            0b101 => self.execute_branch(instr, pc_at_fetch),
            // 110 = Coprocessor
            0b110 => self.log_unimplemented("Coprocessor", instr, pc_at_fetch),
            // 111 = Software Interrupt (SWI) / Coprocessor
            0b111 => {
                // SWI is identified by bit 24 = 1
                if (instr >> 24) & 1 == 1 {
                    self.execute_swi(instr, pc_at_fetch);
                } else {
                    self.log_unimplemented("Coprocessor", instr, pc_at_fetch);
                }
            }
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

    // ── Barrel Shifter ─────────────────────────────────────────────────

    /// Applies a barrel shift operation to a value.
    ///
    /// shift_type: 0=LSL, 1=LSR, 2=ASR, 3=ROR
    /// shift_amount: number of bits to shift (0–31)
    fn shift_operand(value: u32, shift_type: u8, shift_amount: u32) -> u32 {
        if shift_amount == 0 {
            return value; // No shift (LSL #0 = identity)
        }
        match shift_type {
            0 => { // LSL — Logical Shift Left
                if shift_amount >= 32 { 0 } else { value << shift_amount }
            }
            1 => { // LSR — Logical Shift Right
                if shift_amount >= 32 { 0 } else { value >> shift_amount }
            }
            2 => { // ASR — Arithmetic Shift Right (preserves sign)
                if shift_amount >= 32 {
                    if value & 0x8000_0000 != 0 { 0xFFFF_FFFF } else { 0 }
                } else {
                    ((value as i32) >> shift_amount) as u32
                }
            }
            3 => { // ROR — Rotate Right
                value.rotate_right(shift_amount)
            }
            _ => value,
        }
    }

    /// Decodes the register operand2 field, including barrel shift.
    ///
    /// Encoding: [11:8]=shift_amount [6:5]=shift_type [4]=0 [3:0]=Rm
    ///   (bit 4 = 0: shift by immediate, bit 4 = 1: shift by register — we handle immediate here)
    fn decode_register_operand(&self, instr: u32) -> u32 {
        let rm = (instr & 0xF) as usize;
        let rm_val = self.regs.read(rm);
        let shift_type = ((instr >> 5) & 0x3) as u8;
        let shift_amount = (instr >> 7) & 0x1F;
        Self::shift_operand(rm_val, shift_type, shift_amount)
    }

    // ── Data Processing ───────────────────────────────────────────────

    /// Decodes and executes a Data Processing instruction.
    ///
    /// ARM encoding:  cond | 00 | I | opcode | S | Rn | Rd | operand2
    ///   I (bit 25): 1 = immediate, 0 = register (with barrel shift)
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
            // Register with barrel shift
            self.decode_register_operand(instr)
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

    // ── Multiply (MUL / MLA) ─────────────────────────────────────────

    /// Decodes and executes a Multiply (MUL) or Multiply-Accumulate (MLA).
    ///
    /// ARM encoding:  cond | 000000 | A | S | Rd | Rn | Rs | 1001 | Rm
    ///   A (bit 21): 0 = MUL, 1 = MLA (add Rn)
    ///   S (bit 20): 1 = update CPSR flags
    ///   Rd [19:16]: destination register
    ///   Rn [15:12]: accumulate register (MLA only)
    ///   Rs [11:8]:  multiplier register
    ///   Rm [3:0]:   multiplicand register
    fn execute_multiply(&mut self, instr: u32) {
        let accumulate = (instr >> 21) & 1 == 1;
        let set_flags  = (instr >> 20) & 1 == 1;
        let rd = ((instr >> 16) & 0xF) as usize;
        let rn = ((instr >> 12) & 0xF) as usize;
        let rs = ((instr >> 8) & 0xF) as usize;
        let rm = (instr & 0xF) as usize;

        let rm_val = self.regs.read(rm);
        let rs_val = self.regs.read(rs);

        let result = if accumulate {
            let rn_val = self.regs.read(rn);
            rm_val.wrapping_mul(rs_val).wrapping_add(rn_val) // MLA: Rd = Rm * Rs + Rn
        } else {
            rm_val.wrapping_mul(rs_val) // MUL: Rd = Rm * Rs
        };

        self.regs.write(rd, result);
        if set_flags {
            self.regs.update_nz(result);
        }
    }

    // ── Branch Exchange (BX) ────────────────────────────────────────

    /// Executes a Branch Exchange (BX) instruction.
    ///
    /// ARM encoding:  cond | 0001 0010 1111 1111 1111 0001 | Rm
    ///   Rm [3:0]: register containing target address
    ///   If bit 0 of Rm is 1 → switch to Thumb mode, clear LSB
    ///   If bit 0 of Rm is 0 → stay in ARM mode
    fn execute_branch_exchange(&mut self, instr: u32) {
        let rm = (instr & 0xF) as usize;
        let target = self.regs.read(rm);

        if target & 1 != 0 {
            // Switch to Thumb mode: set T flag, clear LSB
            self.regs.set_thumb(true);
            self.regs.set_pc(target & !1);
        } else {
            // Stay in ARM mode: clear T flag
            self.regs.set_thumb(false);
            self.regs.set_pc(target);
        }
    }

    // ── Branch with Link and Exchange (BLX register) ─────────────────

    /// Executes BLX (register) — Branch with Link and Exchange.
    ///
    /// ARM encoding:  cond | 0001 0010 1111 1111 1111 0011 | Rm
    ///   Same as BX but saves return address in LR first.
    fn execute_blx_register(&mut self, instr: u32, pc_at_fetch: u32) {
        let rm = (instr & 0xF) as usize;
        let target = self.regs.read(rm);

        // Save return address in LR (next instruction after this one)
        self.regs.set_lr(pc_at_fetch.wrapping_add(4));

        // Branch with exchange (same as BX)
        if target & 1 != 0 {
            self.regs.set_thumb(true);
            self.regs.set_pc(target & !1);
        } else {
            self.regs.set_thumb(false);
            self.regs.set_pc(target);
        }
    }

    // ── Halfword / Signed Byte Transfers ──────────────────────────────

    /// Decodes and executes LDRH, STRH, LDRSB, LDRSH.
    ///
    /// ARM encoding:  cond | 000 | P | U | I | W | L | Rn | Rd | offset_hi | 1 | S | H | 1 | offset_lo/Rm
    ///   P (bit 24): pre/post indexing
    ///   U (bit 23): up/down (add/subtract offset)
    ///   I (bit 22): 1 = immediate offset (hi:lo), 0 = register offset (Rm)
    ///   W (bit 21): write-back
    ///   L (bit 20): load/store
    ///   S (bit 6): signed
    ///   H (bit 5): halfword (1) or byte (0, only when S=1)
    fn execute_halfword_transfer(&mut self, instr: u32) {
        let pre_index  = (instr >> 24) & 1 == 1;
        let up         = (instr >> 23) & 1 == 1;
        let imm_offset = (instr >> 22) & 1 == 1;
        let write_back = (instr >> 21) & 1 == 1;
        let load       = (instr >> 20) & 1 == 1;
        let is_signed  = (instr >> 6) & 1 == 1;
        let is_half    = (instr >> 5) & 1 == 1;
        let rn = ((instr >> 16) & 0xF) as usize;
        let rd = ((instr >> 12) & 0xF) as usize;

        let base = self.regs.read(rn);

        // Calculate offset
        let offset = if imm_offset {
            // Immediate: [11:8] | [3:0]
            let hi = (instr >> 8) & 0xF;
            let lo = instr & 0xF;
            (hi << 4) | lo
        } else {
            // Register: Rm in [3:0]
            let rm = (instr & 0xF) as usize;
            self.regs.read(rm)
        };

        let addr_offset = if up {
            base.wrapping_add(offset)
        } else {
            base.wrapping_sub(offset)
        };

        let addr = if pre_index { addr_offset } else { base };

        if load {
            let val = if is_signed && is_half {
                // LDRSH: load signed halfword, sign-extend to 32 bits
                let hw = self.mmu.read_u16(addr);
                hw as i16 as i32 as u32
            } else if is_signed && !is_half {
                // LDRSB: load signed byte, sign-extend to 32 bits
                let b = self.mmu.read_u8(addr);
                b as i8 as i32 as u32
            } else {
                // LDRH: load unsigned halfword, zero-extend
                self.mmu.read_u16(addr) as u32
            };
            self.regs.write(rd, val);
        } else {
            // STRH: store halfword
            let val = self.regs.read(rd);
            self.mmu.write_u16(addr, val as u16);
        }

        // Write-back or post-index
        if write_back || !pre_index {
            self.regs.write(rn, addr_offset);
        }
    }

    // ── Software Interrupt (SWI / SVC) ────────────────────────────────

    /// Executes a Software Interrupt (SWI / SVC) instruction.
    fn execute_swi(&mut self, instr: u32, pc_at_fetch: u32) {
        let syscall_num = instr & 0x00FF_FFFF;

        #[cfg(not(test))]
        {
            crate::log(&format!("\u{1F6A8} SWI executed: Syscall number 0x{:06X}", syscall_num));
        }
        #[cfg(test)]
        {
            let _ = syscall_num;
        }

        let saved_cpsr = self.regs.cpsr();
        self.regs.set_spsr_svc(saved_cpsr);
        self.regs.set_lr(pc_at_fetch.wrapping_add(4));
        self.regs.set_cpu_mode(MODE_SVC);
        self.regs.set_irq_disabled(true);
        self.regs.set_thumb(false);
        self.regs.set_pc(SWI_VECTOR);
    }

    // ── High-Level Emulation (HLE) BIOS ──────────────────────────────

    /// Intercepts execution at the SWI vector (0x08) to simulate an OS kernel.
    fn handle_bios_syscall(&mut self) {
        // The original SWI instruction is at LR - 4 (LR points to the instruction AFTER SWI)
        let swi_addr = self.regs.lr().wrapping_sub(4);
        let swi_instr = self.mmu.read_u32(swi_addr);
        let syscall_num = swi_instr & 0x00FF_FFFF;

        match syscall_num {
            // Linux sys_write (fd, buf, count)
            0x04 => {
                let _fd = self.regs.read(0);
                let ptr = self.regs.read(1);
                let len = self.regs.read(2);

                let mut string_buf = String::new();
                for i in 0..len {
                    let b = self.mmu.read_u8(ptr.wrapping_add(i));
                    string_buf.push(b as char);
                }

                #[cfg(not(test))]
                {
                    crate::log(&format!("⚙️ BIOS sys_write: {}", string_buf));
                }
                #[cfg(test)]
                {
                    let _ = string_buf; // avoid unused warning
                }

                // Return bytes written in R0 (simulate success)
                self.regs.write(0, len);
            }
            _ => {
                #[cfg(not(test))]
                {
                    crate::log(&format!("⚠️ Unimplemented BIOS syscall: 0x{:06X}", syscall_num));
                }
            }
        }

        // Exception return (simulate MOVS PC, LR)
        // 1. Restore CPSR from SPSR_svc
        let saved_cpsr = self.regs.spsr_svc();
        self.regs.set_cpsr(saved_cpsr);
        
        // 2. Set PC to return address (LR)
        let return_pc = self.regs.lr();
        self.regs.set_pc(return_pc);
    }

    // ── Single Data Transfer (LDR / STR) ───────────────────────────────

    /// Decodes and executes a Single Data Transfer (LDR / STR) instruction.
    ///
    /// ARM encoding:  cond | 01 | I | P | U | B | W | L | Rn | Rd | offset
    ///   I (bit 25): 0 = immediate offset, 1 = register offset (shifted)
    ///   P (bit 24): 1 = pre-indexed, 0 = post-indexed
    ///   U (bit 23): 1 = add offset, 0 = subtract offset
    ///   B (bit 22): 1 = byte transfer, 0 = word transfer
    ///   W (bit 21): 1 = write-back (update Rn), 0 = no write-back
    ///   L (bit 20): 1 = load (LDR), 0 = store (STR)
    ///   Rn (bits [19:16]): base register
    ///   Rd (bits [15:12]): destination (LDR) or source (STR) register
    ///   offset: 12-bit immediate (I=0) or register+shift (I=1)
    fn execute_single_data_transfer(&mut self, instr: u32) {
        let is_reg_offset = (instr >> 25) & 1 == 1;
        let pre_index     = (instr >> 24) & 1 == 1;
        let up            = (instr >> 23) & 1 == 1;
        let byte_transfer = (instr >> 22) & 1 == 1;
        let write_back    = (instr >> 21) & 1 == 1;
        let load          = (instr >> 20) & 1 == 1;
        let rn = ((instr >> 16) & 0xF) as usize;
        let rd = ((instr >> 12) & 0xF) as usize;

        // Compute the offset
        let offset = if is_reg_offset {
            // Register offset with barrel shift
            self.decode_register_operand(instr)
        } else {
            // 12-bit immediate offset
            instr & 0xFFF
        };

        let base = self.regs.read(rn);

        // Calculate the effective address
        let offset_addr = if up {
            base.wrapping_add(offset)
        } else {
            base.wrapping_sub(offset)
        };

        // Pre-indexed: use offset address, Post-indexed: use base
        let addr = if pre_index { offset_addr } else { base };

        // Execute the transfer
        if load {
            // LDR: read from memory → Rd
            let val = if byte_transfer {
                self.mmu.read_u8(addr) as u32
            } else {
                self.mmu.read_u32(addr)
            };
            self.regs.write(rd, val);
        } else {
            // STR: Rd → write to memory
            let val = self.regs.read(rd);
            if byte_transfer {
                self.mmu.write_u8(addr, (val & 0xFF) as u8);
            } else {
                self.mmu.write_u32(addr, val);
            }
        }

        // Write-back: update base register
        // Pre-indexed with W=1, or post-indexed (always writes back)
        if (pre_index && write_back) || !pre_index {
            self.regs.write(rn, offset_addr);
        }
    }
    // ── Block Data Transfer (LDM / STM) ────────────────────────────────

    /// Decodes and executes a Block Data Transfer (LDM / STM) instruction.
    ///
    /// ARM encoding:  cond | 100 | P | U | S | W | L | Rn | register_list
    ///   P (bit 24): 0 = post (after), 1 = pre (before)
    ///   U (bit 23): 0 = down (decrement), 1 = up (increment)
    ///   S (bit 22): PSR / force user mode (not implemented)
    ///   W (bit 21): 1 = write-back (update Rn)
    ///   L (bit 20): 0 = store (STM), 1 = load (LDM)
    ///   Rn [19:16]: base register
    ///   register_list [15:0]: bitmask of registers to transfer
    ///
    /// Addressing modes:
    ///   P=0 U=1 → IA (Increment After)   — LDMIA / STMIA
    ///   P=1 U=1 → IB (Increment Before)  — LDMIB / STMIB
    ///   P=0 U=0 → DA (Decrement After)   — LDMDA / STMDA
    ///   P=1 U=0 → DB (Decrement Before)  — LDMDB / STMDB / PUSH
    fn execute_block_data_transfer(&mut self, instr: u32) {
        let pre       = (instr >> 24) & 1 == 1;
        let up        = (instr >> 23) & 1 == 1;
        let write_back = (instr >> 21) & 1 == 1;
        let load      = (instr >> 20) & 1 == 1;
        let rn = ((instr >> 16) & 0xF) as usize;
        let reg_list = instr & 0xFFFF;

        // Count how many registers are in the list
        let reg_count = reg_list.count_ones();
        let block_size = reg_count * 4; // each register is 4 bytes

        let base = self.regs.read(rn);

        // Calculate the start address based on addressing mode
        let start_addr = match (pre, up) {
            (false, true) => base,                              // IA: start at base
            (true, true)  => base.wrapping_add(4),             // IB: start at base+4
            (false, false) => base.wrapping_sub(block_size).wrapping_add(4), // DA
            (true, false)  => base.wrapping_sub(block_size),   // DB (PUSH)
        };

        // Transfer registers — always iterate lowest register first
        let mut addr = start_addr;
        for i in 0..16u32 {
            if reg_list & (1 << i) != 0 {
                if load {
                    // LDM: read from memory → register
                    let val = self.mmu.read_u32(addr);
                    self.regs.write(i as usize, val);
                } else {
                    // STM: register → write to memory
                    let val = self.regs.read(i as usize);
                    self.mmu.write_u32(addr, val);
                }
                addr = addr.wrapping_add(4);
            }
        }

        // Write-back: update base register
        if write_back {
            let new_base = if up {
                base.wrapping_add(block_size)
            } else {
                base.wrapping_sub(block_size)
            };
            self.regs.write(rn, new_base);
        }
    }

    // ── Unimplemented handler ─────────────────────────────────────────

    fn log_unimplemented(&self, category: &str, instr: u32, pc: u32) {
        #[cfg(not(test))]
        {
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

    // ── Barrel shifter tests ──────────────────────────────────────────

    #[test]
    fn test_shift_lsl() {
        // MOV R1, #3    → E3A01003
        // MOV R0, R1, LSL #2 → E1A00101
        //   bits: cond=AL 000 opcode=1101(MOV) S=0 Rn=0 Rd=0 shift_amount=00010 type=00(LSL) 0 Rm=0001
        //   = E | 1A0 | 0 | 1 | 0 | 1
        //   = 0xE1A00101
        //   R0 = 3 << 2 = 12
        let program: Vec<u8> = [
            0xE3A01003u32.to_le_bytes(), // MOV R1, #3
            0xE1A00101u32.to_le_bytes(), // MOV R0, R1, LSL #2
        ].concat();

        let mut cpu = cpu_with_program(&program);
        cpu.step(); // MOV R1, #3
        cpu.step(); // MOV R0, R1, LSL #2
        assert_eq!(cpu.regs.read(0), 12, "3 << 2 = 12");
    }

    #[test]
    fn test_shift_lsr() {
        // MOV R1, #32   → E3A01020
        // MOV R0, R1, LSR #3 → E1A001A1
        //   shift_amount=00011 type=01(LSR) 0 Rm=R1
        //   R0 = 32 >> 3 = 4
        let program: Vec<u8> = [
            0xE3A01020u32.to_le_bytes(), // MOV R1, #32
            0xE1A001A1u32.to_le_bytes(), // MOV R0, R1, LSR #3
        ].concat();

        let mut cpu = cpu_with_program(&program);
        cpu.step();
        cpu.step();
        assert_eq!(cpu.regs.read(0), 4, "32 >> 3 = 4");
    }

    #[test]
    fn test_add_with_shift() {
        // R1 = 10, R2 = 3
        // ADD R0, R1, R2, LSL #1 → R0 = 10 + (3 << 1) = 16
        // E0810082
        //   cond=AL 000 opcode=0100(ADD) S=0 Rn=R1 Rd=R0 shift=00001 type=00(LSL) 0 Rm=R2
        let program: Vec<u8> = [
            0xE3A0100Au32.to_le_bytes(), // MOV R1, #10
            0xE3A02003u32.to_le_bytes(), // MOV R2, #3
            0xE0810082u32.to_le_bytes(), // ADD R0, R1, R2, LSL #1
        ].concat();

        let mut cpu = cpu_with_program(&program);
        cpu.step(); // MOV R1, #10
        cpu.step(); // MOV R2, #3
        cpu.step(); // ADD R0, R1, R2, LSL #1
        assert_eq!(cpu.regs.read(0), 16, "10 + (3 << 1) = 16");
    }

    // ── Load/Store tests ──────────────────────────────────────────────

    #[test]
    fn test_basic_str_ldr() {
        // STR R0, [R1]  — store R0 at address in R1
        // LDR R2, [R1]  — load from address in R1 into R2
        //
        // STR R0, [R1, #0] → E5810000
        //   cond=AL 01 I=0 P=1 U=1 B=0 W=0 L=0 Rn=R1 Rd=R0 offset=0
        // LDR R2, [R1, #0] → E5912000
        //   cond=AL 01 I=0 P=1 U=1 B=0 W=0 L=1 Rn=R1 Rd=R2 offset=0
        let program: Vec<u8> = [
            0xE3A000FFu32.to_le_bytes(), // MOV R0, #255
            0xE3A01C01u32.to_le_bytes(), // MOV R1, #256 (0x100)
            0xE5810000u32.to_le_bytes(), // STR R0, [R1]
            0xE5912000u32.to_le_bytes(), // LDR R2, [R1]
        ].concat();

        let mut cpu = cpu_with_program(&program);
        cpu.step(); // MOV R0, #255
        cpu.step(); // MOV R1, #256
        cpu.step(); // STR R0, [R1]

        // Verify memory was written
        assert_eq!(cpu.mmu.read_u32(0x100), 255, "Memory at 0x100 should be 255");

        cpu.step(); // LDR R2, [R1]
        assert_eq!(cpu.regs.read(2), 255, "R2 should be 255 after LDR");
    }

    #[test]
    fn test_str_pre_indexed_writeback() {
        // STR R0, [R1, #4]! — store R0 at R1+4, then R1 = R1+4
        //
        // E5A10004:
        //   cond=AL 01 I=0 P=1 U=1 B=0 W=1 L=0 Rn=R1 Rd=R0 offset=4
        let program: Vec<u8> = [
            0xE3A0002Au32.to_le_bytes(), // MOV R0, #42
            0xE3A01C01u32.to_le_bytes(), // MOV R1, #256 (0x100)
            0xE5A10004u32.to_le_bytes(), // STR R0, [R1, #4]!
        ].concat();

        let mut cpu = cpu_with_program(&program);
        cpu.step(); // MOV R0, #42
        cpu.step(); // MOV R1, #256
        cpu.step(); // STR R0, [R1, #4]!

        assert_eq!(cpu.mmu.read_u32(0x104), 42, "Memory at 0x104 should be 42");
        assert_eq!(cpu.regs.read(1), 0x104, "R1 should be updated to 0x104 (writeback)");
    }

    #[test]
    fn test_ldrb_strb() {
        // STRB R0, [R1] — store low byte of R0
        // LDRB R2, [R1] — load byte into R2
        //
        // STRB R0, [R1, #0] → E5C10000
        //   cond=AL 01 I=0 P=1 U=1 B=1 W=0 L=0 Rn=R1 Rd=R0 offset=0
        // LDRB R2, [R1, #0] → E5D12000
        //   cond=AL 01 I=0 P=1 U=1 B=1 W=0 L=1 Rn=R1 Rd=R2 offset=0
        let program: Vec<u8> = [
            0xE3A000FFu32.to_le_bytes(), // MOV R0, #255
            0xE3A01C02u32.to_le_bytes(), // MOV R1, #512 (0x200)
            0xE5C10000u32.to_le_bytes(), // STRB R0, [R1]
            0xE5D12000u32.to_le_bytes(), // LDRB R2, [R1]
        ].concat();

        let mut cpu = cpu_with_program(&program);
        cpu.step(); // MOV R0, #255
        cpu.step(); // MOV R1, #512
        cpu.step(); // STRB R0, [R1]

        assert_eq!(cpu.mmu.read_u8(0x200), 0xFF, "Byte at 0x200 should be 0xFF");
        // Only 1 byte written — next byte should be 0
        assert_eq!(cpu.mmu.read_u8(0x201), 0x00);

        cpu.step(); // LDRB R2, [R1]
        assert_eq!(cpu.regs.read(2), 0xFF, "R2 should be 0xFF after LDRB");
    }

    // ── LDM/STM (block data transfer) tests ───────────────────────────

    #[test]
    fn test_push_pop_stack() {
        // STMDB R13!, {R0, R1}  (PUSH R0, R1)
        //   cond=AL 100 P=1 U=0 S=0 W=1 L=0 Rn=R13 reg_list=0x0003
        //   = 0xE92D0003
        //
        // LDMIA R13!, {R2, R3}  (POP into R2, R3)
        //   cond=AL 100 P=0 U=1 S=0 W=1 L=1 Rn=R13 reg_list=0x000C
        //   = 0xE8BD000C
        let program: Vec<u8> = [
            0xE3A000AAu32.to_le_bytes(), // MOV R0, #0xAA
            0xE3A010BBu32.to_le_bytes(), // MOV R1, #0xBB
            0xE92D0003u32.to_le_bytes(), // STMDB R13!, {R0, R1}  (PUSH)
            0xE8BD000Cu32.to_le_bytes(), // LDMIA R13!, {R2, R3}  (POP)
        ].concat();

        let mut cpu = cpu_with_program(&program);
        cpu.regs.set_sp(0x1000); // Set SP (within 4KB test RAM)

        cpu.step(); // MOV R0, #0xAA
        cpu.step(); // MOV R1, #0xBB

        assert_eq!(cpu.regs.read(0), 0xAA);
        assert_eq!(cpu.regs.read(1), 0xBB);

        cpu.step(); // STMDB R13!, {R0, R1} — PUSH

        // SP should decrement by 8 (2 registers × 4 bytes)
        assert_eq!(cpu.regs.sp(), 0x1000 - 8, "SP should decrement by 8 after PUSH");
        // Memory: R0 at lower addr, R1 at higher (lowest-numbered reg at lowest addr)
        assert_eq!(cpu.mmu.read_u32(0x0FF8), 0xAA, "R0 value at SP");
        assert_eq!(cpu.mmu.read_u32(0x0FFC), 0xBB, "R1 value at SP+4");

        cpu.step(); // LDMIA R13!, {R2, R3} — POP

        // SP should be back to original
        assert_eq!(cpu.regs.sp(), 0x1000, "SP should be restored after POP");
        // R2 gets the value that was R0, R3 gets the value that was R1
        assert_eq!(cpu.regs.read(2), 0xAA, "R2 should be 0xAA (popped R0's value)");
        assert_eq!(cpu.regs.read(3), 0xBB, "R3 should be 0xBB (popped R1's value)");
    }

    #[test]
    fn test_stm_ldm_multiple() {
        // Store 4 registers, load them back into different registers
        // STMIA R5, {R0-R3}  (no writeback)
        //   cond=AL 100 P=0 U=1 S=0 W=0 L=0 Rn=R5 reg_list=0x000F
        //   = 0xE885000F
        // LDMIA R5, {R4, R6, R7, R8}  (no writeback)
        //   cond=AL 100 P=0 U=1 S=0 W=0 L=1 Rn=R5 reg_list=0x01D0
        //   = 0xE89501D0
        let program: Vec<u8> = [
            0xE3A0000Au32.to_le_bytes(), // MOV R0, #10
            0xE3A01014u32.to_le_bytes(), // MOV R1, #20
            0xE3A0201Eu32.to_le_bytes(), // MOV R2, #30
            0xE3A03028u32.to_le_bytes(), // MOV R3, #40
            0xE3A05C02u32.to_le_bytes(), // MOV R5, #0x200
            0xE885000Fu32.to_le_bytes(), // STMIA R5, {R0-R3}
            0xE89501D0u32.to_le_bytes(), // LDMIA R5, {R4, R6, R7, R8}
        ].concat();

        let mut cpu = cpu_with_program(&program);
        for _ in 0..7 { cpu.step(); }

        // R4 gets value from addr 0x200 (was R0 = 10)
        assert_eq!(cpu.regs.read(4), 10);
        // R6 gets value from addr 0x204 (was R1 = 20)
        assert_eq!(cpu.regs.read(6), 20);
        // R7 gets value from addr 0x208 (was R2 = 30)
        assert_eq!(cpu.regs.read(7), 30);
        // R8 gets value from addr 0x20C (was R3 = 40)
        assert_eq!(cpu.regs.read(8), 40);
    }

    // ── Multiply tests ────────────────────────────────────────────────

    #[test]
    fn test_mul() {
        // MUL R0, R1, R2  (R0 = R1 * R2 = 5 * 6 = 30)
        // ARM encoding: cond | 000000 | A=0 | S=0 | Rd | 0000 | Rs | 1001 | Rm
        //   Rd=R0, Rs=R2, Rm=R1
        //   = 0xE0000291
        let program: Vec<u8> = [
            0xE3A01005u32.to_le_bytes(), // MOV R1, #5
            0xE3A02006u32.to_le_bytes(), // MOV R2, #6
            0xE0000291u32.to_le_bytes(), // MUL R0, R1, R2
        ].concat();

        let mut cpu = cpu_with_program(&program);
        cpu.step(); // MOV R1, #5
        cpu.step(); // MOV R2, #6
        cpu.step(); // MUL R0, R1, R2
        assert_eq!(cpu.regs.read(0), 30, "5 * 6 = 30");
    }

    #[test]
    fn test_mla() {
        // MLA R0, R1, R2, R3  (R0 = R1 * R2 + R3 = 5 * 6 + 10 = 40)
        // ARM encoding: cond | 000000 | A=1 | S=0 | Rd | Rn | Rs | 1001 | Rm
        //   Rd=R0, Rn=R3, Rs=R2, Rm=R1
        //   = 0xE0203291
        let program: Vec<u8> = [
            0xE3A01005u32.to_le_bytes(), // MOV R1, #5
            0xE3A02006u32.to_le_bytes(), // MOV R2, #6
            0xE3A0300Au32.to_le_bytes(), // MOV R3, #10
            0xE0203291u32.to_le_bytes(), // MLA R0, R1, R2, R3
        ].concat();

        let mut cpu = cpu_with_program(&program);
        cpu.step(); // MOV R1, #5
        cpu.step(); // MOV R2, #6
        cpu.step(); // MOV R3, #10
        cpu.step(); // MLA R0, R1, R2, R3
        assert_eq!(cpu.regs.read(0), 40, "5 * 6 + 10 = 40");
    }

    // ── BX tests ─────────────────────────────────────────────────────

    #[test]
    fn test_bx_to_thumb() {
        // Set R0 = 0x101 (LSB set → switch to Thumb)
        // BX R0 → PC = 0x100, T flag set
        // BX R0 = 0xE12FFF10
        let program: Vec<u8> = [
            0xE3A00F40u32.to_le_bytes(), // MOV R0, #256 (0x100)
            0xE2800001u32.to_le_bytes(), // ADD R0, R0, #1  → R0 = 0x101
            0xE12FFF10u32.to_le_bytes(), // BX R0
        ].concat();

        let mut cpu = cpu_with_program(&program);
        cpu.step(); // MOV R0, #256
        cpu.step(); // ADD R0, R0, #1 → R0 = 0x101
        cpu.step(); // BX R0

        assert_eq!(cpu.regs.pc(), 0x100, "PC should be 0x100 (LSB cleared)");
        assert!(cpu.regs.is_thumb(), "T flag should be set (switched to Thumb mode)");
    }

    #[test]
    fn test_bx_stay_arm() {
        // Set R0 = 0x100 (LSB clear → stay ARM)
        // BX R0 → PC = 0x100, T flag clear
        let program: Vec<u8> = [
            0xE3A00F40u32.to_le_bytes(), // MOV R0, #256 (0x100)
            0xE12FFF10u32.to_le_bytes(), // BX R0
        ].concat();

        let mut cpu = cpu_with_program(&program);
        cpu.step(); // MOV R0, #256
        cpu.step(); // BX R0

        assert_eq!(cpu.regs.pc(), 0x100, "PC should be 0x100");
        assert!(!cpu.regs.is_thumb(), "T flag should be clear (staying ARM)");
    }

    // ── SWI tests ─────────────────────────────────────────────────────

    #[test]
    fn test_swi_exception() {
        // SWI #0x42  (syscall 0x42)
        // ARM encoding: cond=AL | 1111 | imm24=0x000042
        //   = 0xEF000042
        let program: Vec<u8> = [
            0xE3A00005u32.to_le_bytes(), // MOV R0, #5  (at addr 0x00)
            0xEF000042u32.to_le_bytes(), // SWI #0x42   (at addr 0x04)
        ].concat();

        let mut cpu = cpu_with_program(&program);
        cpu.step(); // MOV R0, #5

        // Before SWI: mode should be User (0x10)
        assert_eq!(cpu.regs.cpu_mode(), 0x10, "Should be in User mode before SWI");

        cpu.step(); // SWI #0x42

        // After SWI:
        // 1. Mode should be Supervisor (0x13)
        assert_eq!(cpu.regs.cpu_mode(), 0x13, "Should switch to Supervisor mode");
        // 2. LR should point to the instruction after SWI (0x04 + 4 = 0x08)
        assert_eq!(cpu.regs.lr(), 0x08, "LR should be return address (next instr)");
        // 3. IRQ should be disabled
        assert!(cpu.regs.irq_disabled(), "IRQ should be disabled");
        // 4. T flag should be clear (ARM mode)
        assert!(!cpu.regs.is_thumb(), "Should be in ARM mode");
        // 5. PC should be at SWI vector (0x08)
        assert_eq!(cpu.regs.pc(), 0x08, "PC should jump to SWI vector 0x08");
    }

    #[test]
    fn test_swi_preserves_spsr() {
        // Verify that the original CPSR is saved into SPSR_svc
        let program: Vec<u8> = [
            0xE3A00001u32.to_le_bytes(), // MOV R0, #1
            0xE3500001u32.to_le_bytes(), // CMP R0, #1  (sets Z flag)
            0xEF000001u32.to_le_bytes(), // SWI #1
        ].concat();

        let mut cpu = cpu_with_program(&program);
        cpu.step(); // MOV R0, #1
        cpu.step(); // CMP R0, #1 → sets Z flag

        // Capture CPSR before SWI (should have Z flag set + User mode)
        let cpsr_before = cpu.regs.cpsr();
        assert!(cpu.regs.flag_z(), "Z flag should be set before SWI");

        cpu.step(); // SWI #1

        // SPSR_svc should contain the pre-SWI CPSR
        assert_eq!(cpu.regs.spsr_svc(), cpsr_before, "SPSR_svc should preserve original CPSR");
        // Current CPSR should be different (SVC mode, IRQ disabled)
        assert_ne!(cpu.regs.cpsr(), cpsr_before, "Current CPSR should differ after SWI");
        assert_eq!(cpu.regs.cpu_mode(), 0x13, "Now in SVC mode");
    }

    #[test]
    fn test_bios_sys_write() {
        // Setup a sys_write (0x04) SWI
        // R0 = 1 (stdout)
        // R1 = 0x200 (string pointer)
        // R2 = 5 ("Hello" length)
        let program: Vec<u8> = [
            0xE3A00001u32.to_le_bytes(), // MOV R0, #1
            0xE3A01C02u32.to_le_bytes(), // MOV R1, #0x200
            0xE3A02005u32.to_le_bytes(), // MOV R2, #5
            0xEF000004u32.to_le_bytes(), // SWI #4
        ].concat();

        let mut cpu = cpu_with_program(&program);
        
        // Write "Hello" to RAM at 0x200
        cpu.mmu.write_u8(0x200, b'H');
        cpu.mmu.write_u8(0x201, b'e');
        cpu.mmu.write_u8(0x202, b'l');
        cpu.mmu.write_u8(0x203, b'l');
        cpu.mmu.write_u8(0x204, b'o');

        // Step through MOVs
        cpu.step(); // MOV R0
        cpu.step(); // MOV R1
        cpu.step(); // MOV R2

        // Step SWI — this will set PC to 0x08, mode to SVC
        let cpsr_before = cpu.regs.cpsr();
        cpu.step();
        assert_eq!(cpu.regs.pc(), 0x08, "PC should be at SWI vector");
        assert_eq!(cpu.regs.cpu_mode(), 0x13, "Should be in SVC mode");

        // Now step again — this should trigger handle_bios_syscall()
        cpu.step();

        // 1. R0 should contain the bytes written (5)
        assert_eq!(cpu.regs.read(0), 5, "R0 should contain bytes written");
        // 2. CPSR should be restored to pre-SWI state (User mode)
        assert_eq!(cpu.regs.cpsr(), cpsr_before, "CPSR should be restored");
        // 3. PC should be restored to next instruction (0x10)
        assert_eq!(cpu.regs.pc(), 0x10, "PC should return from exception");
    }

    // ── BLX tests ────────────────────────────────────────────────────

    #[test]
    fn test_blx_register() {
        // BLX R0: branch to R0, save return addr in LR, switch mode if LSB=1
        // BLX R0 = 0xE12FFF30
        let program: Vec<u8> = [
            0xE3A00F40u32.to_le_bytes(), // MOV R0, #256 (0x100)
            0xE2800001u32.to_le_bytes(), // ADD R0, R0, #1 → R0 = 0x101
            0xE12FFF30u32.to_le_bytes(), // BLX R0
        ].concat();

        let mut cpu = cpu_with_program(&program);
        cpu.step(); // MOV R0, #256
        cpu.step(); // ADD R0, R0, #1 → R0 = 0x101

        let pc_before_blx = cpu.regs.pc(); // PC = 0x08 (addr of BLX = 0x08, after advance)
        cpu.step(); // BLX R0

        // PC should jump to 0x100 (LSB cleared)
        assert_eq!(cpu.regs.pc(), 0x100, "PC should be 0x100");
        // T flag should be set (Thumb mode, because R0 had LSB=1)
        assert!(cpu.regs.is_thumb(), "T flag should be set");
        // LR should contain the return address (next instruction after BLX)
        // BLX was at addr 0x08, so return = 0x08 + 4 = 0x0C
        assert_eq!(cpu.regs.lr(), pc_before_blx.wrapping_add(4), "LR should be return address");
    }

    // ── Halfword transfer tests ─────────────────────────────────────

    #[test]
    fn test_strh_stores_halfword() {
        // STRH R0, [R1, #0]  (0xE1C100B0)
        let program: Vec<u8> = [0xE1C100B0u32.to_le_bytes()].concat();
        let mut cpu = cpu_with_program(&program);
        cpu.regs.write(0, 0xBEEF); // value to store
        cpu.regs.write(1, 0x200);  // base address
        
        cpu.step(); // Execute STRH

        assert_eq!(cpu.mmu.read_u16(0x200), 0xBEEF, "Halfword should be stored");
        assert_eq!(cpu.mmu.read_u8(0x202), 0, "Byte at +2 should be zero");
    }

    #[test]
    fn test_ldrsh_sign_extends() {
        // LDRSH R2, [R1, #0] (0xE1D120F0)
        let program: Vec<u8> = [0xE1D120F0u32.to_le_bytes()].concat();
        let mut cpu = cpu_with_program(&program);
        cpu.regs.write(1, 0x200); // base address
        cpu.mmu.write_u16(0x200, 0xFF80); // pre-load 0xFF80 (-128)

        cpu.step(); // Execute LDRSH

        // 0xFF80 as i16 = -128, sign-extended to 32 bits = 0xFFFFFF80
        assert_eq!(cpu.regs.read(2), 0xFFFF_FF80, "LDRSH should sign-extend 0xFF80 to 0xFFFFFF80");
    }

    #[test]
    fn test_ldrh_zero_extends() {
        // LDRH R2, [R1, #0] (0xE1D120B0)
        let program: Vec<u8> = [0xE1D120B0u32.to_le_bytes()].concat();
        let mut cpu = cpu_with_program(&program);
        cpu.regs.write(1, 0x200); // base address
        cpu.mmu.write_u16(0x200, 0xFF80); // pre-load

        cpu.step(); // Execute LDRH

        assert_eq!(cpu.regs.read(2), 0x0000_FF80, "LDRH should zero-extend 0xFF80");
    }

    #[test]
    fn test_ldrsb_sign_extends() {
        // LDRSB R2, [R1, #0] (0xE1D120D0)
        let program: Vec<u8> = [0xE1D120D0u32.to_le_bytes()].concat();
        let mut cpu = cpu_with_program(&program);
        cpu.regs.write(1, 0x200); // base address
        cpu.mmu.write_u8(0x200, 0x80); // pre-load unsigned 0x80 (-128)

        cpu.step(); // Execute LDRSB

        assert_eq!(cpu.regs.read(2), 0xFFFF_FF80, "LDRSB should sign-extend 0x80 to 0xFFFFFF80");
    }
}

