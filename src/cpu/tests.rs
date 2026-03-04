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

    // ── Thumb tests ───────────────────────────────────────────────────

    #[test]
    fn test_thumb_basic_branch() {
        // Thumb B +0: encoding 0xE000 → top 5 bits = 11100, offset11 = 0
        // Target = PC_fetch(0) + 4 + (0 << 1) = 4
        // 0xE000 in little-endian = [0x00, 0xE0]
        // 0xE7FE in little-endian = [0xFE, 0xE7] (B -2, infinite loop)
        let mut cpu = cpu_with_program(&[0x00, 0xE0, 0xFE, 0xE7]);
        cpu.regs.set_thumb(true);

        cpu.step(); // Execute B +0 at addr 0 → target = 0 + 4 + 0 = 4
        assert_eq!(cpu.regs.pc(), 4, "B +0 should set PC to fetch_addr(0) + 4");
    }

    #[test]
    fn test_thumb_branch_backward() {
        // Place two instructions:
        //   0x0000: B +0 (0xE000) — jumps to addr 4
        //   0x0002: NOP placeholder
        //   0x0004: B -4 (0xE7FD) — jumps back to addr 2
        //   offset11 = 0x7FD = -3, (-3 << 1) = -6, target = 4 + 4 - 6 = 2
        let mut cpu = cpu_with_program(&[0x00, 0xE0, 0x00, 0x00, 0xFD, 0xE7]);
        cpu.regs.set_thumb(true);

        cpu.step(); // Execute B +0 at addr 0 → PC = 4
        assert_eq!(cpu.regs.pc(), 4);

        cpu.step(); // Execute B -4 at addr 4 → target = 4 + 4 - 6 = 2
        assert_eq!(cpu.regs.pc(), 2, "Backward branch should jump to addr 2");
    }

    #[test]
    fn test_thumb_alu_and() {
        // Set up R0 = 0xFF, R1 = 0x0F, then execute Thumb AND R0, R1
        // AND R0, R1: format 010000 0000 001 000 = 0x4008
        let mut cpu = cpu_with_program(&[0x08, 0x40]);
        cpu.regs.set_thumb(true);
        cpu.regs.write(0, 0xFF);
        cpu.regs.write(1, 0x0F);

        cpu.step();
        assert_eq!(cpu.regs.read(0), 0x0F, "AND 0xFF, 0x0F = 0x0F");
        assert!(!cpu.regs.flag_z());
        assert!(!cpu.regs.flag_n());
    }

    #[test]
    fn test_thumb_alu_eor() {
        // EOR R0, R1: format 010000 0001 001 000 = 0x4048
        let mut cpu = cpu_with_program(&[0x48, 0x40]);
        cpu.regs.set_thumb(true);
        cpu.regs.write(0, 0xFF);
        cpu.regs.write(1, 0xFF);

        cpu.step();
        assert_eq!(cpu.regs.read(0), 0, "EOR 0xFF, 0xFF = 0");
        assert!(cpu.regs.flag_z(), "Z flag should be set for zero result");
    }

    #[test]
    fn test_thumb_alu_orr() {
        // ORR R0, R1: format 010000 1100 001 000 = 0x4308
        let mut cpu = cpu_with_program(&[0x08, 0x43]);
        cpu.regs.set_thumb(true);
        cpu.regs.write(0, 0xF0);
        cpu.regs.write(1, 0x0F);

        cpu.step();
        assert_eq!(cpu.regs.read(0), 0xFF, "ORR 0xF0, 0x0F = 0xFF");
    }

    #[test]
    fn test_thumb_alu_mvn() {
        // MVN R0, R1: format 010000 1111 001 000 = 0x43C8
        let mut cpu = cpu_with_program(&[0xC8, 0x43]);
        cpu.regs.set_thumb(true);
        cpu.regs.write(1, 0x00000000);

        cpu.step();
        assert_eq!(cpu.regs.read(0), 0xFFFFFFFF, "MVN 0 = 0xFFFFFFFF");
        assert!(cpu.regs.flag_n(), "N flag should be set for negative result");
    }

    #[test]
    fn test_thumb_alu_cmp() {
        // CMP R0, R1: format 010000 1010 001 000 = 0x4288
        let mut cpu = cpu_with_program(&[0x88, 0x42]);
        cpu.regs.set_thumb(true);
        cpu.regs.write(0, 5);
        cpu.regs.write(1, 5);

        cpu.step();
        assert!(cpu.regs.flag_z(), "CMP 5, 5 should set Z flag");
        assert!(cpu.regs.flag_c(), "CMP equal should set C flag (no borrow)");
        assert!(!cpu.regs.flag_v(), "CMP equal should clear V flag");
    }

    #[test]
    fn test_thumb_alu_tst() {
        // TST R0, R1: format 010000 1000 001 000 = 0x4208
        let mut cpu = cpu_with_program(&[0x08, 0x42]);
        cpu.regs.set_thumb(true);
        cpu.regs.write(0, 0xF0);
        cpu.regs.write(1, 0x0F);

        cpu.step();
        assert!(cpu.regs.flag_z(), "TST 0xF0, 0x0F should be zero");
        // R0 should be unchanged (TST doesn't store)
        assert_eq!(cpu.regs.read(0), 0xF0);
    }

    #[test]
    fn test_thumb_imm_alu() {
        // MOV R0, #10 = 0x200A
        // ADD R0, #5  = 0x3005
        // SUB R0, #2  = 0x3802
        // CMP R0, #13 = 0x280D
        let mut cpu = cpu_with_program(&[
            0x0A, 0x20, // MOV R0, #10
            0x05, 0x30, // ADD R0, #5
            0x02, 0x38, // SUB R0, #2
            0x0D, 0x28, // CMP R0, #13
        ]);
        cpu.regs.set_thumb(true);

        cpu.step(); // MOV R0, #10
        assert_eq!(cpu.regs.read(0), 10);

        cpu.step(); // ADD R0, #5 → 15
        assert_eq!(cpu.regs.read(0), 15);

        cpu.step(); // SUB R0, #2 → 13
        assert_eq!(cpu.regs.read(0), 13);

        cpu.step(); // CMP R0, #13 → equal
        assert_eq!(cpu.regs.read(0), 13, "CMP should not modify Rd");
        assert!(cpu.regs.flag_z(), "CMP 13, 13 should set Z flag");
        assert!(!cpu.regs.flag_n(), "CMP 13, 13 should clear N flag");
    }

    #[test]
    fn test_thumb_cond_branch() {
        // MOV R0, #5  = 0x2005
        // CMP R0, #5  = 0x2805
        // BEQ +2      = 0xD001 (cond=0x0=EQ, offset=1, 1<<1=+2 bytes)
        // MOV R1, #1  = 0x2101 (should be skipped)
        // MOV R2, #2  = 0x2202 (branch lands here)
        let mut cpu = cpu_with_program(&[
            0x05, 0x20, // 0x0000: MOV R0, #5
            0x05, 0x28, // 0x0002: CMP R0, #5
            0x01, 0xD0, // 0x0004: BEQ +2 → target = 4 + 4 + 2 = 10
            0x01, 0x21, // 0x0006: MOV R1, #1 (skipped)
            0x02, 0x22, // 0x0008: MOV R2, #2 (skipped)
            0x03, 0x23, // 0x000A: MOV R3, #3 (branch target)
        ]);
        cpu.regs.set_thumb(true);

        cpu.step(); // MOV R0, #5
        assert_eq!(cpu.regs.read(0), 5);

        cpu.step(); // CMP R0, #5 → Z=true
        assert!(cpu.regs.flag_z());

        cpu.step(); // BEQ +2 → branch taken, PC = 10
        assert_eq!(cpu.regs.pc(), 10, "BEQ should branch to addr 10");

        cpu.step(); // MOV R3, #3 (at addr 10)
        assert_eq!(cpu.regs.read(3), 3, "Should execute instruction at branch target");
        assert_eq!(cpu.regs.read(1), 0, "R1 should be 0 — MOV R1,#1 was skipped");
    }

    #[test]
    fn test_thumb_ldr_str_imm() {
        // STR R0, [R1, #4]: 011 0 0 00001 001 000 = 0x6048
        //   B=0 (word), L=0 (store), imm5=1, Rn=1, Rd=0 → offset = 1*4 = 4
        // LDR R0, [R1, #4]: 011 0 1 00001 001 000 = 0x6848
        //   B=0 (word), L=1 (load), imm5=1, Rn=1, Rd=0 → offset = 1*4 = 4
        let mut cpu = cpu_with_program(&[
            0x48, 0x60, // STR R0, [R1, #4]
            0x48, 0x68, // LDR R0, [R1, #4]
        ]);
        cpu.regs.set_thumb(true);
        cpu.regs.write(0, 0xDEADBEEF);
        cpu.regs.write(1, 0x200);

        cpu.step(); // STR R0, [R1, #4] → write 0xDEADBEEF to addr 0x204
        assert_eq!(cpu.mmu.read_u32(0x204), 0xDEADBEEF, "STR should write to [R1+4]");

        cpu.regs.write(0, 0); // Clear R0
        cpu.step(); // LDR R0, [R1, #4] → read from addr 0x204
        assert_eq!(cpu.regs.read(0), 0xDEADBEEF, "LDR should load from [R1+4]");
    }

    #[test]
    fn test_thumb_push_pop() {
        // PUSH {R0, R1} = 0xB403  (1011 0 10 0 00000011)
        // POP  {R2, R3} = 0xBC0C  (1011 1 10 0 00001100)
        let mut cpu = cpu_with_program(&[
            0x03, 0xB4, // PUSH {R0, R1}
            0x0C, 0xBC, // POP  {R2, R3}
        ]);
        cpu.regs.set_thumb(true);
        cpu.regs.set_sp(0x1000);
        cpu.regs.write(0, 10);
        cpu.regs.write(1, 20);

        cpu.step(); // PUSH {R0, R1} — STMDB SP!, {R0, R1}
        assert_eq!(cpu.regs.sp(), 0x0FF8, "SP should decrement by 8 (2 registers)");
        assert_eq!(cpu.mmu.read_u32(0x0FF8), 10, "R0 at lowest address");
        assert_eq!(cpu.mmu.read_u32(0x0FFC), 20, "R1 at next address");

        cpu.regs.write(0, 0); // Clear R0
        cpu.regs.write(1, 0); // Clear R1

        cpu.step(); // POP {R2, R3} — LDMIA SP!, {R2, R3}
        assert_eq!(cpu.regs.read(2), 10, "R2 should get value originally in R0");
        assert_eq!(cpu.regs.read(3), 20, "R3 should get value originally in R1");
        assert_eq!(cpu.regs.sp(), 0x1000, "SP should be restored to original");
    }

    #[test]
    fn test_thumb_sp_relative_ldr_str() {
        // STR R0, [SP, #4]: 1001 0 000 00000001 = 0x9001
        // LDR R0, [SP, #4]: 1001 1 000 00000001 = 0x9801
        let mut cpu = cpu_with_program(&[
            0x01, 0x90, // STR R0, [SP, #4]
            0x01, 0x98, // LDR R0, [SP, #4]
        ]);
        cpu.regs.set_thumb(true);
        cpu.regs.set_sp(0x200);
        cpu.regs.write(0, 0xCAFEBABE);

        cpu.step(); // STR R0, [SP, #4]
        assert_eq!(cpu.mmu.read_u32(0x204), 0xCAFEBABE, "STR should write to [SP+4]");

        cpu.regs.write(0, 0); // Clear R0
        cpu.step(); // LDR R0, [SP, #4]
        assert_eq!(cpu.regs.read(0), 0xCAFEBABE, "LDR should load from [SP+4]");
    }

    #[test]
    fn test_thumb_ldr_str_reg_and_halfword() {
        // Part 1: Format 8 register-offset halfword
        // STRH R0, [R1, R2]:  0101 000 Rm Rn Rd → op=0b100 → 0101 100 010 001 000 = 0x5288
        //   bits: 0101 1 00 010 001 000 = 0x5248
        //   Actually: top=0101, op[11:9]=100, Rm=010, Rn=001, Rd=000
        //   15..10 = 010110, 9..6 = 0010, 5..3 = 001, 2..0 = 000
        //   0101 100 010 001 000 = 0x5A48  wait let me recalculate
        //
        // Encoding: [15:12]=0101, [11:9]=op, [8:6]=Rm, [5:3]=Rn, [2:0]=Rd
        // STRH R0, [R1, R2]: op=100, Rm=R2(010), Rn=R1(001), Rd=R0(000)
        //   0101 100 010 001 000 = 0b0101_1000_1000_1000 = 0x5288
        //   Let me be precise: 0101 100 010 001 000
        //   0101 = 5, 1000 = 8, 1000 = 8, 1000... wait
        //   bit15=0 bit14=1 bit13=0 bit12=1 bit11=1 bit10=0 bit9=0 bit8=0 bit7=1 bit6=0 bit5=0 bit4=0 bit3=1 bit2=0 bit1=0 bit0=0
        //   Nope. Let me just lay it out:
        //   [15:12] = 0101
        //   [11:9]  = 100  (op for STRH)
        //   [8:6]   = 010  (Rm = R2)
        //   [5:3]   = 001  (Rn = R1)
        //   [2:0]   = 000  (Rd = R0)
        //   = 0101_100_010_001_000 = 0b0101100010001000 = 0x5888
        //   Hmm: 0101 1000 1000 1000 = 0x5888
        //
        // LDRSH R3, [R1, R2]: op=111, Rm=R2(010), Rn=R1(001), Rd=R3(011)
        //   [15:12] = 0101
        //   [11:9]  = 111
        //   [8:6]   = 010
        //   [5:3]   = 001
        //   [2:0]   = 011
        //   = 0101_111_010_001_011 = 0b0101111010001011 = 0x5E8B
        //
        // Part 2: Format 10 halfword immediate offset
        // STRH R0, [R1, #2]: L=0, imm5=1 (offset=1*2=2), Rn=R1, Rd=R0
        //   [15:12]=1000, [11]=0, [10:6]=00001, [5:3]=001, [2:0]=000
        //   = 1000_0_00001_001_000 = 0b1000000001001000 = 0x8048
        //
        // LDRH R4, [R1, #2]: L=1, imm5=1 (offset=1*2=2), Rn=R1, Rd=R4
        //   [15:12]=1000, [11]=1, [10:6]=00001, [5:3]=001, [2:0]=100
        //   = 1000_1_00001_001_100 = 0b1000100001001100 = 0x884C

        let mut cpu = cpu_with_program(&[
            0x88, 0x58, // STRH R0, [R1, R2]   — 0x5888
            0x8B, 0x5E, // LDRSH R3, [R1, R2]  — 0x5E8B
            0x48, 0x80, // STRH R0, [R1, #2]   — 0x8048
            0x4C, 0x88, // LDRH R4, [R1, #2]   — 0x884C
        ]);
        cpu.regs.set_thumb(true);
        cpu.regs.write(0, 0xFF80); // value to store (halfword = 0xFF80, sign-extends to negative)
        cpu.regs.write(1, 0x100);  // base address
        cpu.regs.write(2, 4);      // offset register

        // Step 1: STRH R0, [R1, R2] — store 0xFF80 at address 0x104
        cpu.step();
        assert_eq!(cpu.mmu.read_u16(0x104), 0xFF80, "STRH reg should write halfword to [R1+R2]");

        // Step 2: LDRSH R3, [R1, R2] — load from 0x104, sign-extend 0xFF80 → 0xFFFFFF80
        cpu.step();
        assert_eq!(cpu.regs.read(3), 0xFFFFFF80, "LDRSH should sign-extend 0xFF80");

        // Step 3: STRH R0, [R1, #2] — store 0xFF80 at address 0x102
        cpu.step();
        assert_eq!(cpu.mmu.read_u16(0x102), 0xFF80, "STRH imm should write halfword to [R1+2]");

        // Step 4: LDRH R4, [R1, #2] — load from 0x102, zero-extend → 0x0000FF80
        cpu.step();
        assert_eq!(cpu.regs.read(4), 0xFF80, "LDRH imm should zero-extend 0xFF80");
    }

    #[test]
    fn test_thumb_format_1_2_alu() {
        // MOV R1, #10:       Format 3 MOV: 0010 0 001 00001010 = 0x210A
        // ADD R0, R1, #5:    Format 2 ADD imm: 00011 1 0 101 001 000 = 0x1D48
        //   op=3, i=1, sub=0, imm3=5(101), Rn=R1(001), Rd=R0(000)
        // LSL R2, R0, #1:    Format 1 LSL: 00000 00001 000 010 = 0x0042
        //   op=0, shift_amount=1(00001), Rm=R0(000), Rd=R2(010)
        let mut cpu = cpu_with_program(&[
            0x0A, 0x21, // MOV R1, #10    — 0x210A
            0x48, 0x1D, // ADD R0, R1, #5 — 0x1D48
            0x42, 0x00, // LSL R2, R0, #1 — 0x0042
        ]);
        cpu.regs.set_thumb(true);

        cpu.step(); // MOV R1, #10
        assert_eq!(cpu.regs.read(1), 10, "MOV R1, #10");

        cpu.step(); // ADD R0, R1, #5
        assert_eq!(cpu.regs.read(0), 15, "ADD R0, R1, #5 should give 15");

        cpu.step(); // LSL R2, R0, #1
        assert_eq!(cpu.regs.read(2), 30, "LSL R2, R0, #1 should give 30");
    }

    #[test]
    fn test_thumb_bl_long_branch() {
        // BL with offset: Prefix 0xF000 (offset_hi = 0), Suffix 0xF804 (offset_lo = 4)
        // Target = (PC+4 + 0<<12) + (4<<1) = 0x1004 + 8 = 0x100C
        let mut cpu = Cpu::new(8192); // Need >0x1000 bytes of RAM
        cpu.load_program(0x1000, &[
            0x00, 0xF0, // Prefix: 0xF000
            0x04, 0xF8, // Suffix: 0xF804
        ]);
        cpu.regs.set_thumb(true);
        cpu.regs.set_pc(0x1000);

        cpu.step(); // Execute Prefix — sets LR to intermediate target
        assert_eq!(cpu.regs.lr(), 0x1004, "Prefix should set LR to PC+4 + (0<<12)");

        cpu.step(); // Execute Suffix — jumps to target, saves return address
        assert_eq!(cpu.regs.pc(), 0x100C, "BL target should be 0x100C");
        assert_eq!(cpu.regs.lr(), 0x1005, "LR should be return addr 0x1004 | 1 for Thumb");
    }
