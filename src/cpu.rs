// ── nekodroid: ARMv7 CPU Emulator Core ─────────────────────────────────
//
// RegisterFile: 16 general-purpose registers + CPSR
// Cpu: owns RegisterFile + Mmu, orchestrates execution

use crate::memory::Mmu;
use crate::cp15::Cp15;

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
const MODE_FIQ: u32  = 0x11;
const MODE_IRQ: u32  = 0x12;
const MODE_SVC: u32  = 0x13;  // Supervisor mode
const MODE_ABT: u32  = 0x17;
const MODE_UND: u32  = 0x1B;
const MODE_SYS: u32  = 0x1F;

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
    /// Saved Program Status Register (Abort mode)
    spsr_abt: u32,
    /// Saved Program Status Register (Undefined mode)
    spsr_und: u32,
    /// Saved Program Status Register (IRQ mode)
    spsr_irq: u32,
    /// Saved Program Status Register (FIQ mode)
    spsr_fiq: u32,
    /// Banked R13 for SVC mode
    banked_sp_svc: u32,
    /// Banked R14 for SVC mode
    banked_lr_svc: u32,
    /// Banked R13 for ABT mode
    banked_sp_abt: u32,
    /// Banked R14 for ABT mode
    banked_lr_abt: u32,
    /// Banked R13 for UND mode
    banked_sp_und: u32,
    /// Banked R14 for UND mode
    banked_lr_und: u32,
    /// Banked R13 for IRQ mode
    banked_sp_irq: u32,
    /// Banked R14 for IRQ mode
    banked_lr_irq: u32,
    /// Banked R13 for FIQ mode
    banked_sp_fiq: u32,
    /// Banked R14 for FIQ mode
    banked_lr_fiq: u32,
    /// Pipeline offset added when reading R15 as an operand.
    /// ARM mode: +4 (so R15 reads as instruction_addr + 8, since advance_pc already added 4)
    /// Thumb mode: +2 (so R15 reads as instruction_addr + 4, since advance_pc already added 2)
    /// Set to 0 outside of instruction execution.
    pipeline_offset: u32,
}

impl RegisterFile {
    /// Creates a new register file with all registers zeroed.
    pub fn new() -> Self {
        RegisterFile {
            regs: [0u32; 16],
            cpsr: MODE_USER, // Start in User mode
            spsr_svc: 0,
            spsr_abt: 0,
            spsr_und: 0,
            spsr_irq: 0,
            spsr_fiq: 0,
            banked_sp_svc: 0,
            banked_lr_svc: 0,
            banked_sp_abt: 0,
            banked_lr_abt: 0,
            banked_sp_und: 0,
            banked_lr_und: 0,
            banked_sp_irq: 0,
            banked_lr_irq: 0,
            banked_sp_fiq: 0,
            banked_lr_fiq: 0,
            pipeline_offset: 0,
        }
    }

    fn save_banked_sp_lr(&mut self, mode: u32) {
        match mode {
            MODE_SVC => {
                self.banked_sp_svc = self.regs[REG_SP];
                self.banked_lr_svc = self.regs[REG_LR];
            }
            MODE_ABT => {
                self.banked_sp_abt = self.regs[REG_SP];
                self.banked_lr_abt = self.regs[REG_LR];
            }
            MODE_UND => {
                self.banked_sp_und = self.regs[REG_SP];
                self.banked_lr_und = self.regs[REG_LR];
            }
            MODE_IRQ => {
                self.banked_sp_irq = self.regs[REG_SP];
                self.banked_lr_irq = self.regs[REG_LR];
            }
            MODE_FIQ => {
                self.banked_sp_fiq = self.regs[REG_SP];
                self.banked_lr_fiq = self.regs[REG_LR];
            }
            _ => {}
        }
    }

    fn load_banked_sp_lr(&mut self, mode: u32) {
        match mode {
            MODE_SVC => {
                self.regs[REG_SP] = self.banked_sp_svc;
                self.regs[REG_LR] = self.banked_lr_svc;
            }
            MODE_ABT => {
                self.regs[REG_SP] = self.banked_sp_abt;
                self.regs[REG_LR] = self.banked_lr_abt;
            }
            MODE_UND => {
                self.regs[REG_SP] = self.banked_sp_und;
                self.regs[REG_LR] = self.banked_lr_und;
            }
            MODE_IRQ => {
                self.regs[REG_SP] = self.banked_sp_irq;
                self.regs[REG_LR] = self.banked_lr_irq;
            }
            MODE_FIQ => {
                self.regs[REG_SP] = self.banked_sp_fiq;
                self.regs[REG_LR] = self.banked_lr_fiq;
            }
            _ => {}
        }
    }

    // ── Register access ───────────────────────────────────────────────

    /// Reads a general-purpose register (0–15).
    /// When reading R15 (PC), applies the pipeline offset so instructions
    /// see the architecturally correct PC value (instruction + 8 in ARM,
    /// instruction + 4 in Thumb).
    pub fn read(&self, reg: usize) -> u32 {
        let r = reg & 0xF;
        if r == REG_PC {
            self.regs[REG_PC].wrapping_add(self.pipeline_offset)
        } else {
            self.regs[r]
        }
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
        let old_mode = self.cpu_mode();
        let new_mode = val & CPSR_MODE_MASK;
        if old_mode != new_mode {
            self.save_banked_sp_lr(old_mode);
            self.load_banked_sp_lr(new_mode);
        }
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
        let new_cpsr = (self.cpsr & !CPSR_MODE_MASK) | (mode & CPSR_MODE_MASK);
        self.set_cpsr(new_cpsr);
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

    /// Sets SPSR for the provided target exception mode.
    pub fn set_spsr(&mut self, target_mode: u32, val: u32) {
        match target_mode {
            MODE_SVC => self.spsr_svc = val,
            MODE_ABT => self.spsr_abt = val,
            MODE_UND => self.spsr_und = val,
            MODE_IRQ => self.spsr_irq = val,
            MODE_FIQ => self.spsr_fiq = val,
            _ => {}
        }
    }

    /// Reads SPSR for the provided mode.
    pub fn spsr(&self, mode: u32) -> u32 {
        match mode {
            MODE_SVC => self.spsr_svc,
            MODE_ABT => self.spsr_abt,
            MODE_UND => self.spsr_und,
            MODE_IRQ => self.spsr_irq,
            MODE_FIQ => self.spsr_fiq,
            _ => 0,
        }
    }

    /// Writes LR in a specific banked mode without switching current CPU mode.
    pub fn set_lr_banked(&mut self, target_mode: u32, addr: u32) {
        if self.cpu_mode() == target_mode {
            self.regs[REG_LR] = addr;
            return;
        }
        match target_mode {
            MODE_SVC => self.banked_lr_svc = addr,
            MODE_ABT => self.banked_lr_abt = addr,
            MODE_UND => self.banked_lr_und = addr,
            MODE_IRQ => self.banked_lr_irq = addr,
            MODE_FIQ => self.banked_lr_fiq = addr,
            _ => self.regs[REG_LR] = addr,
        }
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
    /// System Control Coprocessor (CP15)
    pub cp15: Cp15,
    /// Whether the CPU is halted
    pub halted: bool,
    /// Set true when an exception is taken during an instruction.
    exception_raised: bool,
}

impl Cpu {
    /// Creates a new CPU with the given RAM size.
    pub fn new(ram_size: usize) -> Self {
        Cpu {
            regs: RegisterFile::new(),
            mmu: Mmu::new(ram_size),
            cp15: Cp15::new(),
            halted: false,
            exception_raised: false,
        }
    }

    /// Creates a new CPU with the default 16 MB RAM.
    pub fn default() -> Self {
        Cpu {
            regs: RegisterFile::new(),
            mmu: Mmu::default(),
            cp15: Cp15::new(),
            halted: false,
            exception_raised: false,
        }
    }

    /// Resets the CPU to initial state: all registers zeroed,
    /// SP set to top of RAM minus 64 KB, PC set to 0x8000.
    pub fn reset(&mut self) {
        self.regs = RegisterFile::new();
        self.cp15 = Cp15::new();
        self.halted = false;
        self.exception_raised = false;
        // Clear UART output buffer
        self.mmu.clear_uart_buffer();
        // Clear VRAM to black
        self.mmu.clear_vram();
        // Set SP to top of RAM minus 64 KB (matches init_emulator convention)
        self.regs.set_sp((self.mmu.ram_size() as u32).wrapping_sub(0x1_0000));
        // Set PC to standard boot address
        self.regs.set_pc(0x0000_8000);
    }

    /// Prepares the CPU to boot an ARM Linux kernel using the ATAGs protocol.
    pub fn boot_linux(&mut self, kernel_bytes: &[u8], machine_type: u32) {
        self.reset();

        let atag_base = 0x100u32;
        let mut offset = 0u32;

        // 1. ATAG_CORE
        self.mmu.write_u32(atag_base + offset, 2);
        self.mmu.write_u32(atag_base + offset + 4, 0x5441_0001);
        offset += 8;

        // 2. ATAG_MEM (RAM starts at 0x0)
        self.mmu.write_u32(atag_base + offset, 4);
        self.mmu.write_u32(atag_base + offset + 4, 0x5441_0002);
        self.mmu.write_u32(atag_base + offset + 8, self.mmu.ram_size() as u32);
        self.mmu.write_u32(atag_base + offset + 12, 0x0000_0000);
        offset += 16;

        // 3. ATAG_NONE
        self.mmu.write_u32(atag_base + offset, 0);
        self.mmu.write_u32(atag_base + offset + 4, 0x0000_0000);

        // 4. Load kernel at 0x8000
        self.load_program(0x8000, kernel_bytes);

        // 5. Linux boot register contract
        self.regs.write(0, 0);
        self.regs.write(1, machine_type);
        self.regs.write(2, atag_base);
        self.regs.set_pc(0x8000);

        #[cfg(not(test))]
        {
            crate::log(&format!(
                "🐧 Prepared Linux boot. Machine ID: {:#X}, ATAGs at: {:#X}",
                machine_type, atag_base
            ));
        }
    }

    /// Translates a virtual address to a physical address using CP15 translation tables.
    pub fn translate_address(&mut self, vaddr: u32) -> u32 {
        // 1. MMU enable bit (SCTLR.M)
        if self.cp15.c1_sctlr & 1 == 0 {
            return vaddr;
        }

        // 2. TTBR0 base (short-descriptor, first-level table)
        let table_base = self.cp15.c2_ttbr0 & 0xFFFF_C000;

        // 3. First-level index from VA[31:20]
        let table_index = vaddr >> 20;

        // 4. Descriptor address in first-level table
        let desc_addr = table_base | (table_index << 2);

        // 5. Read descriptor from physical memory (bypass translation)
        let descriptor = self.mmu.read_u32(desc_addr);

        // 6. Descriptor type
        let desc_type = descriptor & 0b11;

        if desc_type == 0b10 {
            // Section mapping (1 MB): PA = descriptor[31:20] : VA[19:0]
            let phys_base = descriptor & 0xFFF0_0000;
            let offset = vaddr & 0x000F_FFFF;
            return phys_base | offset;
        } else if desc_type == 0b01 {
            // Coarse Page Table mapping (Level 2 walk for 4KB pages)
            // 1. Base of L2 table from L1 descriptor bits [31:10]
            let l2_base = descriptor & 0xFFFF_FC00;

            // 2. L2 index from VA[19:12]
            let l2_index = (vaddr >> 12) & 0xFF;

            // 3. Address of L2 descriptor
            let l2_desc_addr = l2_base | (l2_index << 2);

            // 4. Read L2 descriptor from physical memory
            let l2_desc = self.mmu.read_u32(l2_desc_addr);

            // 5. Small page descriptor type: bits [1:0] == 0b10
            if (l2_desc & 0b11) == 0b10 {
                let phys_base = l2_desc & 0xFFFF_F000;
                let offset = vaddr & 0x0000_0FFF;
                return phys_base | offset;
            }

            #[cfg(not(test))]
            {
                crate::log(&format!(
                    "⚠️ MMU Fault: Unhandled L2 descriptor {:#010X} at vaddr {:#010X}",
                    l2_desc, vaddr
                ));
            }
            self.trigger_exception("Data Abort", MODE_ABT, 0x10, 8);
            return vaddr;
        } else {
            #[cfg(not(test))]
            {
                crate::log(&format!(
                    "⚠️ MMU Fault: Unhandled L1 descriptor type {} at vaddr {:#010X}",
                    desc_type, vaddr
                ));
            }
            self.trigger_exception("Data Abort", MODE_ABT, 0x10, 8);
            return vaddr;
        }
    }

    pub fn read_mem_u8(&mut self, vaddr: u32) -> u8 {
        let paddr = self.translate_address(vaddr);
        if self.exception_raised {
            return 0;
        }
        self.mmu.read_u8(paddr)
    }

    pub fn write_mem_u8(&mut self, vaddr: u32, val: u8) {
        let paddr = self.translate_address(vaddr);
        if self.exception_raised {
            return;
        }
        self.mmu.write_u8(paddr, val);
    }

    pub fn read_mem_u16(&mut self, vaddr: u32) -> u16 {
        let paddr = self.translate_address(vaddr);
        if self.exception_raised {
            return 0;
        }
        self.mmu.read_u16(paddr)
    }

    pub fn write_mem_u16(&mut self, vaddr: u32, val: u16) {
        let paddr = self.translate_address(vaddr);
        if self.exception_raised {
            return;
        }
        self.mmu.write_u16(paddr, val);
    }

    pub fn read_mem_u32(&mut self, vaddr: u32) -> u32 {
        let paddr = self.translate_address(vaddr);
        if self.exception_raised {
            return 0;
        }
        self.mmu.read_u32(paddr)
    }

    pub fn write_mem_u32(&mut self, vaddr: u32, val: u32) {
        let paddr = self.translate_address(vaddr);
        if self.exception_raised {
            return;
        }
        self.mmu.write_u32(paddr, val);
    }

    /// Fetches the next instruction word from memory at the current PC.
    pub fn fetch(&mut self) -> u32 {
        let pc = self.regs.pc();
        if self.regs.is_thumb() {
            self.read_mem_u16(pc) as u32
        } else {
            self.read_mem_u32(pc)
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
                // Check for Multiply (short & long): bits [27:24] = 0000, bits [7:4] = 1001
                if bits_27_25 == 0b000 && (instr & 0x0F00_00F0) == 0x0000_0090 {
                    let is_long = (instr >> 23) & 1 == 1;
                    let a = (instr >> 21) & 1 == 1;
                    let rs = (instr >> 8) & 0xF;
                    let rm = instr & 0xF;
                    if is_long {
                        let u = (instr >> 22) & 1 == 1;
                        let rd_hi = (instr >> 16) & 0xF;
                        let rd_lo = (instr >> 12) & 0xF;
                        let mnemonic = match (u, a) {
                            (false, false) => "UMULL",
                            (false, true)  => "UMLAL",
                            (true,  false) => "SMULL",
                            (true,  true)  => "SMLAL",
                        };
                        return format!("{}{} {}, {}, {}, {}", mnemonic, cs,
                            Self::reg_name(rd_lo), Self::reg_name(rd_hi),
                            Self::reg_name(rm), Self::reg_name(rs));
                    } else {
                        let rd = (instr >> 16) & 0xF;
                        let rn = (instr >> 12) & 0xF;
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
                    && (instr & 0x0F0000F0) != 0x00000090 {
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

    /// Triggers a hardware exception, switching modes and jumping to the vector table.
    pub fn trigger_exception(&mut self, exception_type: &str, target_mode: u32, vector_offset: u32, pc_adjustment: u32) {
        let cpsr = self.regs.cpsr();

        // 1. Save CPSR to target mode SPSR
        self.regs.set_spsr(target_mode, cpsr);

        // 2. Save return address into target mode LR bank
        let return_addr = self.regs.read(REG_PC).wrapping_sub(pc_adjustment);
        self.regs.set_lr_banked(target_mode, return_addr);

        // 3. Change mode, disable IRQ, optionally disable FIQ, force ARM state
        let mut new_cpsr = (cpsr & !CPSR_MODE_MASK) | target_mode;
        new_cpsr |= 0x80;
        if target_mode == MODE_FIQ {
            new_cpsr |= 0x40;
        }
        new_cpsr &= !(1 << 5);
        self.regs.set_cpsr(new_cpsr);

        // 4. Vector base from SCTLR.V (bit 13)
        let high_vectors = (self.cp15.c1_sctlr & (1 << 13)) != 0;
        let vector_base = if high_vectors { 0xFFFF_0000 } else { 0x0000_0000 };

        // 5. Branch to vector
        let vector = vector_base + vector_offset;
        self.regs.set_pc(vector);
        self.exception_raised = true;

        #[cfg(not(test))]
        {
            crate::log(&format!("⚡ Exception: {} -> Jumped to {:#010X}", exception_type, vector));
        }
    }

    /// Advances SP804 Timer1 by one CPU step.
    fn tick_sp804_timer(&mut self) {
        // Timer1Control bit 7: Enable
        if (self.mmu.timer1_ctrl & 0x80) != 0 {
            let (new_val, underflow) = self.mmu.timer1_value.overflowing_sub(1);
            self.mmu.timer1_value = new_val;

            if underflow {
                // Timer1Control bit 6: Periodic mode
                if (self.mmu.timer1_ctrl & 0x40) != 0 {
                    self.mmu.timer1_value = self.mmu.timer1_load;
                } else {
                    // Free-running mode wraps
                    self.mmu.timer1_value = 0xFFFF_FFFF;
                }

                // Timer1Control bit 5: interrupt enable
                if (self.mmu.timer1_ctrl & 0x20) != 0 {
                    self.mmu.vic_int_status |= 1 << 4; // Timer1 on VIC line 4
                    self.mmu.update_vic();
                }
            }
        }
    }

    // ── Fetch-Decode-Execute ──────────────────────────────────────────

    /// Executes one instruction cycle: fetch → decode → execute.
    /// Returns true if the CPU executed an instruction, false if halted.
    pub fn step(&mut self) -> bool {
        if self.halted {
            return false;
        }

        // Check for pending hardware IRQ before executing next instruction.
        if self.mmu.irq_pending {
            let cpsr = self.regs.cpsr();
            if (cpsr & 0x80) == 0 {
                self.trigger_exception("IRQ", MODE_IRQ, 0x18, 4);
                self.tick_sp804_timer();
                return true;
            }
        }

        self.exception_raised = false;

        // ── HLE BIOS Intercept ────────────────────────────────────────
        // If the PC has reached the SWI vector (0x08) AND we are in Supervisor mode,
        // intercept execution to handle the syscall in Rust instead of executing ARM code.
        if self.regs.pc() == SWI_VECTOR && self.regs.cpu_mode() == MODE_SVC {
            self.handle_bios_syscall();
            self.tick_sp804_timer();
            return true;
        }

        // ── FETCH ─────────────────────────────────────────────────────
        let instr = self.fetch();
        let pc_at_fetch = self.regs.pc();
        self.advance_pc();

        // ── THUMB MODE ────────────────────────────────────────────────
        // Thumb instructions are 16-bit and have their own decode table.
        // Skip the ARM condition check and decode entirely.
        if self.regs.is_thumb() {
            // Thumb pipeline: reading R15 returns current_instruction + 4
            self.regs.pipeline_offset = 2; // advance_pc added 2, so +2 more = +4 from fetch
            self.execute_thumb_instruction(instr as u16, pc_at_fetch);
            self.regs.pipeline_offset = 0;
            self.tick_sp804_timer();
            return true;
        }

        // ── CONDITION CHECK ───────────────────────────────────────────
        // ARM instructions bits [31:28] are the condition code.
        // If the condition is not met, the instruction is a NOP.
        if !self.check_condition(instr) {
            self.tick_sp804_timer();
            return true; // Instruction skipped, but CPU is not halted
        }

        // ARM pipeline: reading R15 returns current_instruction + 8
        self.regs.pipeline_offset = 4; // advance_pc added 4, so +4 more = +8 from fetch

        // Coprocessor register transfer (MCR/MRC): bits [27:24] == 1110 and bit[4] == 1
        let is_coproc_transfer = ((instr >> 24) & 0xF == 0b1110) && ((instr >> 4) & 1 == 1);
        if is_coproc_transfer {
            let opc1 = ((instr >> 21) & 0x7) as usize;
            let crn = ((instr >> 16) & 0xF) as usize;
            let rd = ((instr >> 12) & 0xF) as usize;
            let coproc = ((instr >> 8) & 0xF) as usize;
            let opc2 = ((instr >> 5) & 0x7) as usize;
            let crm = (instr & 0xF) as usize;

            let is_mrc = (instr >> 20) & 1 == 1;
            // Accept p15 and p10 encodings used by some toolchains/fixtures for CP15 ops.
            if coproc == 15 || coproc == 10 {
                if is_mrc {
                    let val = self.cp15.read_register(crn, crm, opc1, opc2);
                    self.regs.write(rd, val);
                } else {
                    let val = self.regs.read(rd);
                    self.cp15.write_register(crn, crm, opc1, opc2, val);
                }
            } else {
                self.log_unimplemented("Coprocessor Transfer", instr, pc_at_fetch);
            }

            self.regs.pipeline_offset = 0;
            self.tick_sp804_timer();
            return true;
        }

        // ── DECODE & EXECUTE ──────────────────────────────────────────
        // Top-level decode using bits [27:25]
        let bits_27_25 = (instr >> 25) & 0b111;

        match bits_27_25 {
            // 000 = Data Processing (register) / Multiply / Misc
            0b000 => {
                // Check for Multiply (short & long): bits [27:24] = 0000, bits [7:4] = 1001
                if (instr & 0x0F00_00F0) == 0x0000_0090 {
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

        self.regs.pipeline_offset = 0;
        self.tick_sp804_timer();

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

    // ── Multiply (MUL / MLA / UMULL / UMLAL / SMULL / SMLAL) ────────

    /// Decodes and executes all multiply instructions.
    ///
    /// Short multiply (bit 23 = 0):
    ///   cond | 000000 | A | S | Rd | Rn | Rs | 1001 | Rm
    ///   A (bit 21): 0 = MUL, 1 = MLA
    ///
    /// Long multiply (bit 23 = 1):
    ///   cond | 00001 | U | A | S | RdHi | RdLo | Rs | 1001 | Rm
    ///   U (bit 22): 0 = unsigned, 1 = signed
    ///   A (bit 21): 0 = multiply, 1 = multiply-accumulate
    fn execute_multiply(&mut self, instr: u32) {
        let is_long    = (instr >> 23) & 1 == 1;
        let set_flags  = (instr >> 20) & 1 == 1;
        let rs = ((instr >> 8) & 0xF) as usize;
        let rm = (instr & 0xF) as usize;

        let rm_val = self.regs.read(rm);
        let rs_val = self.regs.read(rs);

        if is_long {
            // Long multiply: UMULL / UMLAL / SMULL / SMLAL
            let signed     = (instr >> 22) & 1 == 1;
            let accumulate = (instr >> 21) & 1 == 1;
            let rd_hi = ((instr >> 16) & 0xF) as usize;
            let rd_lo = ((instr >> 12) & 0xF) as usize;

            let result: u64 = if signed {
                (rm_val as i32 as i64).wrapping_mul(rs_val as i32 as i64) as u64
            } else {
                (rm_val as u64).wrapping_mul(rs_val as u64)
            };

            let result = if accumulate {
                let hi = self.regs.read(rd_hi) as u64;
                let lo = self.regs.read(rd_lo) as u64;
                result.wrapping_add((hi << 32) | lo)
            } else {
                result
            };

            self.regs.write(rd_lo, result as u32);
            self.regs.write(rd_hi, (result >> 32) as u32);
            if set_flags {
                let n = (result >> 63) & 1 == 1;
                let z = result == 0;
                let mut cpsr = self.regs.cpsr;
                cpsr = (cpsr & !(1 << 31)) | ((n as u32) << 31);
                cpsr = (cpsr & !(1 << 30)) | ((z as u32) << 30);
                self.regs.cpsr = cpsr;
            }
        } else {
            // Short multiply: MUL / MLA
            let accumulate = (instr >> 21) & 1 == 1;
            let rd = ((instr >> 16) & 0xF) as usize;
            let rn = ((instr >> 12) & 0xF) as usize;

            let result = if accumulate {
                let rn_val = self.regs.read(rn);
                rm_val.wrapping_mul(rs_val).wrapping_add(rn_val)
            } else {
                rm_val.wrapping_mul(rs_val)
            };

            self.regs.write(rd, result);
            if set_flags {
                self.regs.update_nz(result);
            }
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
                let hw = self.read_mem_u16(addr);
                hw as i16 as i32 as u32
            } else if is_signed && !is_half {
                // LDRSB: load signed byte, sign-extend to 32 bits
                let b = self.read_mem_u8(addr);
                b as i8 as i32 as u32
            } else {
                // LDRH: load unsigned halfword, zero-extend
                self.read_mem_u16(addr) as u32
            };
            self.regs.write(rd, val);
        } else {
            // STRH: store halfword
            let val = self.regs.read(rd);
            self.write_mem_u16(addr, val as u16);
        }

        // Write-back or post-index
        if write_back || !pre_index {
            self.regs.write(rn, addr_offset);
        }
    }

    // ── Software Interrupt (SWI / SVC) ────────────────────────────────

    /// Executes a Software Interrupt (SWI / SVC) instruction.
    fn execute_swi(&mut self, instr: u32, pc_at_fetch: u32) {
        let _ = (instr, pc_at_fetch);
        self.trigger_exception("SWI", MODE_SVC, 0x08, 4);
    }

    // ── High-Level Emulation (HLE) BIOS ──────────────────────────────

    /// Intercepts execution at the SWI vector (0x08) to simulate an OS kernel.
    fn handle_bios_syscall(&mut self) {
        // The original SWI instruction is at LR - 4 (LR points to the instruction AFTER SWI)
        let swi_addr = self.regs.lr().wrapping_sub(4);
        let swi_instr = self.read_mem_u32(swi_addr);
        let syscall_num = swi_instr & 0x00FF_FFFF;

        match syscall_num {
            // Linux sys_write (fd, buf, count)
            0x04 => {
                let _fd = self.regs.read(0);
                let ptr = self.regs.read(1);
                let len = self.regs.read(2);

                let mut string_buf = String::new();
                for i in 0..len {
                    let b = self.read_mem_u8(ptr.wrapping_add(i));
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
                self.read_mem_u8(addr) as u32
            } else {
                self.read_mem_u32(addr)
            };
            self.regs.write(rd, val);
        } else {
            // STR: Rd → write to memory
            let val = self.regs.read(rd);
            if byte_transfer {
                self.write_mem_u8(addr, (val & 0xFF) as u8);
            } else {
                self.write_mem_u32(addr, val);
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
                    let val = self.read_mem_u32(addr);
                    self.regs.write(i as usize, val);
                } else {
                    // STM: register → write to memory
                    let val = self.regs.read(i as usize);
                    self.write_mem_u32(addr, val);
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

    // ── Thumb Instruction Decode ──────────────────────────────────────

    /// Decodes and executes a 16-bit Thumb instruction.
    ///
    /// Thumb instructions are grouped by the top 6 bits (instr >> 10).
    /// In Thumb mode the PC reads as current_instruction + 4 (not +8 like ARM).
    fn execute_thumb_instruction(&mut self, instr: u16, pc_at_fetch: u32) {
        match instr >> 10 {
            0..=7 => { // Formats 1 & 2: Shift by Immediate, Add/Subtract
                let op = (instr >> 11) & 0x3; // 0=LSL, 1=LSR, 2=ASR, 3=ADD/SUB

                if op == 0x3 { // Format 2: Add/Subtract
                    let i_bit = (instr >> 10) & 1 == 1; // 1 = Immediate, 0 = Register
                    let sub_bit = (instr >> 9) & 1 == 1; // 1 = SUB, 0 = ADD
                    let rn = ((instr >> 3) & 0x7) as usize;
                    let rd = (instr & 0x7) as usize;
                    let rn_val = self.regs.read(rn);

                    let operand = if i_bit {
                        ((instr >> 6) & 0x7) as u32 // 3-bit immediate
                    } else {
                        let rm = ((instr >> 6) & 0x7) as usize;
                        self.regs.read(rm)
                    };

                    if sub_bit { // SUB
                        let result = rn_val.wrapping_sub(operand);
                        self.regs.write(rd, result);
                        self.regs.update_nz(result);
                        self.regs.set_flag_c(rn_val >= operand);
                        let overflow = ((rn_val ^ operand) & (rn_val ^ result)) >> 31 != 0;
                        self.regs.set_flag_v(overflow);
                    } else { // ADD
                        let result = rn_val.wrapping_add(operand);
                        self.regs.write(rd, result);
                        self.regs.update_nz(result);
                        self.regs.set_flag_c(result < rn_val);
                        let overflow = (!((rn_val ^ operand)) & (rn_val ^ result)) >> 31 != 0;
                        self.regs.set_flag_v(overflow);
                    }
                } else { // Format 1: Shift by Immediate
                    let shift_amount = ((instr >> 6) & 0x1F) as u32;
                    let rm = ((instr >> 3) & 0x7) as usize;
                    let rd = (instr & 0x7) as usize;
                    let rm_val = self.regs.read(rm);

                    let result = Self::shift_operand(rm_val, op as u8, shift_amount);
                    self.regs.write(rd, result);
                    self.regs.update_nz(result);
                }
            }
            0b010000 => { // Format 5: Data Processing (ALU operations)
                let op = (instr >> 6) & 0xF;
                let rm = ((instr >> 3) & 0x7) as usize;
                let rd = (instr & 0x7) as usize;
                let rd_val = self.regs.read(rd);
                let rm_val = self.regs.read(rm);
                match op {
                    0x0 => { // AND
                        let result = rd_val & rm_val;
                        self.regs.write(rd, result);
                        self.regs.update_nz(result);
                    }
                    0x1 => { // EOR
                        let result = rd_val ^ rm_val;
                        self.regs.write(rd, result);
                        self.regs.update_nz(result);
                    }
                    0x2 => { // LSL
                        let result = Self::shift_operand(rd_val, 0, rm_val & 0xFF);
                        self.regs.write(rd, result);
                        self.regs.update_nz(result);
                    }
                    0x3 => { // LSR
                        let result = Self::shift_operand(rd_val, 1, rm_val & 0xFF);
                        self.regs.write(rd, result);
                        self.regs.update_nz(result);
                    }
                    0x4 => { // ASR
                        let result = Self::shift_operand(rd_val, 2, rm_val & 0xFF);
                        self.regs.write(rd, result);
                        self.regs.update_nz(result);
                    }
                    0x8 => { // TST (test — AND but result discarded)
                        let result = rd_val & rm_val;
                        self.regs.update_nz(result);
                    }
                    0xA => { // CMP (compare — SUB but result discarded)
                        let result = rd_val.wrapping_sub(rm_val);
                        self.regs.update_nz(result);
                        self.regs.set_flag_c(rd_val >= rm_val);
                        let overflow = ((rd_val ^ rm_val) & (rd_val ^ result)) >> 31 != 0;
                        self.regs.set_flag_v(overflow);
                    }
                    0xC => { // ORR
                        let result = rd_val | rm_val;
                        self.regs.write(rd, result);
                        self.regs.update_nz(result);
                    }
                    0xF => { // MVN (Move NOT)
                        let result = !rm_val;
                        self.regs.write(rd, result);
                        self.regs.update_nz(result);
                    }
                    _ => self.log_unimplemented("Thumb ALU", instr as u32, pc_at_fetch),
                }
            }
            8..=15 => { // Format 3: Move/Compare/Add/Subtract Immediate (top 3 bits = 001)
                let op = (instr >> 11) & 0x3;
                let rd = ((instr >> 8) & 0x7) as usize;
                let imm8 = (instr & 0xFF) as u32;
                let rd_val = self.regs.read(rd);
                match op {
                    0x0 => { // MOV Rd, #imm8
                        self.regs.write(rd, imm8);
                        self.regs.update_nz(imm8);
                    }
                    0x1 => { // CMP Rd, #imm8
                        let result = rd_val.wrapping_sub(imm8);
                        self.regs.update_nz(result);
                        self.regs.set_flag_c(rd_val >= imm8);
                        let overflow = ((rd_val ^ imm8) & (rd_val ^ result)) >> 31 != 0;
                        self.regs.set_flag_v(overflow);
                    }
                    0x2 => { // ADD Rd, #imm8
                        let result = rd_val.wrapping_add(imm8);
                        self.regs.write(rd, result);
                        self.regs.update_nz(result);
                        self.regs.set_flag_c(result < rd_val);
                        let overflow = (!(rd_val ^ imm8) & (rd_val ^ result)) >> 31 != 0;
                        self.regs.set_flag_v(overflow);
                    }
                    0x3 => { // SUB Rd, #imm8
                        let result = rd_val.wrapping_sub(imm8);
                        self.regs.write(rd, result);
                        self.regs.update_nz(result);
                        self.regs.set_flag_c(rd_val >= imm8);
                        let overflow = ((rd_val ^ imm8) & (rd_val ^ result)) >> 31 != 0;
                        self.regs.set_flag_v(overflow);
                    }
                    _ => unreachable!(),
                }
            }
            20..=23 => { // Format 7/8: Load/Store with Register Offset
                let op = (instr >> 9) & 0x7;
                let rm = ((instr >> 6) & 0x7) as usize;
                let rn = ((instr >> 3) & 0x7) as usize;
                let rd = (instr & 0x7) as usize;

                let base_addr = self.regs.read(rn);
                let offset = self.regs.read(rm);
                let addr = base_addr.wrapping_add(offset);

                match op {
                    0b000 => { // STR Rd, [Rn, Rm]
                        let val = self.regs.read(rd);
                        self.write_mem_u32(addr, val);
                    }
                    0b001 => { // STRB Rd, [Rn, Rm]
                        let val = (self.regs.read(rd) & 0xFF) as u8;
                        self.write_mem_u8(addr, val);
                    }
                    0b010 => { // LDR Rd, [Rn, Rm]
                        let val = self.read_mem_u32(addr);
                        self.regs.write(rd, val);
                    }
                    0b011 => { // LDRB Rd, [Rn, Rm]
                        let val = self.read_mem_u8(addr) as u32;
                        self.regs.write(rd, val);
                    }
                    0b100 => { // STRH Rd, [Rn, Rm]
                        let val = (self.regs.read(rd) & 0xFFFF) as u16;
                        self.write_mem_u16(addr, val);
                    }
                    0b101 => { // LDRSB Rd, [Rn, Rm]
                        let val = self.read_mem_u8(addr) as i8 as i32 as u32;
                        self.regs.write(rd, val);
                    }
                    0b110 => { // LDRH Rd, [Rn, Rm]
                        let val = self.read_mem_u16(addr) as u32;
                        self.regs.write(rd, val);
                    }
                    0b111 => { // LDRSH Rd, [Rn, Rm]
                        let val = self.read_mem_u16(addr) as i16 as i32 as u32;
                        self.regs.write(rd, val);
                    }
                    _ => unreachable!(),
                }
            }
            24..=31 => { // Format 9: Load/Store with Immediate Offset (top 3 bits = 011)
                let b_bit = (instr >> 12) & 1 == 1; // 1 = Byte, 0 = Word
                let l_bit = (instr >> 11) & 1 == 1; // 1 = Load, 0 = Store
                let imm5  = ((instr >> 6) & 0x1F) as u32;
                let rn    = ((instr >> 3) & 0x7) as usize;
                let rd    = (instr & 0x7) as usize;

                let base_addr = self.regs.read(rn);

                if b_bit { // Byte transfer
                    let addr = base_addr.wrapping_add(imm5); // Offset is imm5
                    if l_bit { // LDRB
                        let val = self.read_mem_u8(addr) as u32;
                        self.regs.write(rd, val);
                    } else { // STRB
                        let val = (self.regs.read(rd) & 0xFF) as u8;
                        self.write_mem_u8(addr, val);
                    }
                } else { // Word transfer
                    let addr = base_addr.wrapping_add(imm5 << 2); // Offset is imm5 * 4
                    if l_bit { // LDR
                        let val = self.read_mem_u32(addr);
                        self.regs.write(rd, val);
                    } else { // STR
                        let val = self.regs.read(rd);
                        self.write_mem_u32(addr, val);
                    }
                }
            }
            32..=35 => { // Format 10: Halfword Load/Store with Immediate Offset
                let l_bit = (instr >> 11) & 1 == 1; // 1 = LDRH, 0 = STRH
                let imm5 = ((instr >> 6) & 0x1F) as u32;
                let rn = ((instr >> 3) & 0x7) as usize;
                let rd = (instr & 0x7) as usize;

                // Offset is imm5 * 2
                let offset = imm5 << 1;
                let base_addr = self.regs.read(rn);
                let addr = base_addr.wrapping_add(offset);

                if l_bit { // LDRH Rd, [Rn, #imm]
                    let val = self.read_mem_u16(addr) as u32;
                    self.regs.write(rd, val);
                } else { // STRH Rd, [Rn, #imm]
                    let val = (self.regs.read(rd) & 0xFFFF) as u16;
                    self.write_mem_u16(addr, val);
                }
            }
            36..=39 => { // Format 11: SP-Relative Load/Store (top 4 bits = 1001)
                let l_bit = (instr >> 11) & 1 == 1; // 1 = Load, 0 = Store
                let rd = ((instr >> 8) & 0x7) as usize;
                let imm8 = (instr & 0xFF) as u32;

                let offset = imm8 << 2; // Offset is imm8 * 4
                let sp_val = self.regs.read(13); // R13 = SP
                let addr = sp_val.wrapping_add(offset);

                if l_bit { // LDR Rd, [SP, #imm]
                    let val = self.read_mem_u32(addr);
                    self.regs.write(rd, val);
                } else { // STR Rd, [SP, #imm]
                    let val = self.regs.read(rd);
                    self.write_mem_u32(addr, val);
                }
            }
            44..=47 => { // Format 14: PUSH / POP (top 4 bits = 1011)
                let l_bit = (instr >> 11) & 1 == 1; // 1 = POP, 0 = PUSH
                let r_bit = (instr >> 8) & 1 == 1;  // PUSH LR or POP PC
                let reg_list = (instr & 0xFF) as u32;

                if l_bit { // POP — equivalent to LDMIA SP!, {reg_list, PC?}
                    let mut arm_reg_list = reg_list;
                    if r_bit { arm_reg_list |= 1 << 15; } // Add PC (R15)
                    let dummy_arm = 0xE8BD0000 | arm_reg_list;
                    self.execute_block_data_transfer(dummy_arm);
                } else { // PUSH — equivalent to STMDB SP!, {reg_list, LR?}
                    let mut arm_reg_list = reg_list;
                    if r_bit { arm_reg_list |= 1 << 14; } // Add LR (R14)
                    let dummy_arm = 0xE92D0000 | arm_reg_list;
                    self.execute_block_data_transfer(dummy_arm);
                }
            }
            52..=55 => { // Format 16: Conditional Branch (top 4 bits = 1101)
                let cond = ((instr >> 8) & 0xF) as u32;

                // SWI intercept: cond == 0xF means this is a Thumb SWI, not a branch
                if cond == 0xF {
                    let swi_num = (instr & 0xFF) as u32;
                    let dummy_swi = 0xEF000000 | swi_num;
                    self.execute_swi(dummy_swi, pc_at_fetch);
                    return;
                }

                // Reuse ARM condition checker by placing cond in bits [31:28]
                let dummy_instr = cond << 28;
                if self.check_condition(dummy_instr) {
                    let imm8 = (instr & 0xFF) as u32;
                    // Sign extend 8-bit immediate to 32 bits, shift left by 1
                    let offset = if imm8 & 0x80 != 0 {
                        (imm8 | 0xFFFFFF00) << 1
                    } else {
                        imm8 << 1
                    };
                    let target = pc_at_fetch.wrapping_add(4).wrapping_add(offset);
                    self.regs.set_pc(target);
                }
            }
            0b111000 | 0b111001 => { // Format 18: Unconditional Branch (top 5 bits = 11100)
                let offset11 = instr & 0x07FF;
                // Sign extend the 11-bit offset to 32 bits, then shift left by 1
                let offset = if offset11 & 0x0400 != 0 {
                    ((offset11 as u32) | 0xFFFFF800) << 1
                } else {
                    (offset11 as u32) << 1
                };
                // Add offset to PC (Thumb PC reads as pc_at_fetch + 4)
                let target = pc_at_fetch.wrapping_add(4).wrapping_add(offset);
                self.regs.set_pc(target);
            }
            60..=61 => { // Format 19: BL (Prefix)
                let offset_11 = (instr & 0x7FF) as u32;
                // Sign extend 11 bits to 32 bits, then shift left by 12
                let offset = if offset_11 & 0x400 != 0 {
                    (offset_11 | 0xFFFFF800) << 12
                } else {
                    offset_11 << 12
                };
                // Target high = PC + offset (Thumb PC reads as pc_at_fetch + 4)
                let target_high = pc_at_fetch.wrapping_add(4).wrapping_add(offset);
                // Store intermediate value in LR
                self.regs.set_lr(target_high);
            }
            62..=63 => { // Format 19: BL (Suffix)
                let offset_11 = (instr & 0x7FF) as u32;
                let target_high = self.regs.lr();
                // Add the low 11 bits (shifted left by 1) to the high target
                let target = target_high.wrapping_add(offset_11 << 1);

                // Save return address in LR: instruction AFTER this suffix (suffix PC + 2)
                // Set bit 0 to 1 to indicate we are returning to Thumb mode
                self.regs.set_lr(pc_at_fetch.wrapping_add(2) | 1);
                self.regs.set_pc(target);
            }
            _ => self.log_unimplemented("Thumb", instr as u32, pc_at_fetch),
        }
    }

    // ── Unimplemented handler ─────────────────────────────────────────

    fn log_unimplemented(&mut self, category: &str, instr: u32, pc: u32) {
        #[cfg(not(test))]
        {
            let _ = (category, instr, pc);
        }
        self.trigger_exception("Undefined Instruction", MODE_UND, 0x04, 4);
    }
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests;

