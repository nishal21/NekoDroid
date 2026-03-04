    use super::*;

    // ── Basic Read/Write (Little-Endian) ──────────────────────────────

    #[test]
    fn test_read_write_u8() {
        let mut mmu = Mmu::new(256);
        mmu.write_u8(0x10, 0xAB);
        assert_eq!(mmu.read_u8(0x10), 0xAB);
    }

    #[test]
    fn test_read_write_u16_little_endian() {
        let mut mmu = Mmu::new(256);
        mmu.write_u16(0x20, 0xBEEF);
        assert_eq!(mmu.read_u8(0x20), 0xEF); // low byte first
        assert_eq!(mmu.read_u8(0x21), 0xBE); // high byte second
        assert_eq!(mmu.read_u16(0x20), 0xBEEF);
    }

    #[test]
    fn test_read_write_u32_little_endian() {
        let mut mmu = Mmu::new(256);
        mmu.write_u32(0x30, 0xDEADBEEF);
        assert_eq!(mmu.read_u8(0x30), 0xEF);
        assert_eq!(mmu.read_u8(0x31), 0xBE);
        assert_eq!(mmu.read_u8(0x32), 0xAD);
        assert_eq!(mmu.read_u8(0x33), 0xDE);
        assert_eq!(mmu.read_u32(0x30), 0xDEADBEEF);
    }

    #[test]
    fn test_out_of_bounds_reads_zero() {
        let mmu = Mmu::new(256);
        // Reading past RAM size should return 0, not panic
        assert_eq!(mmu.read_u8(0x1000), 0);
        assert_eq!(mmu.read_u16(0x1000), 0);
        assert_eq!(mmu.read_u32(0x1000), 0);
    }

    #[test]
    fn test_load_bytes() {
        let mut mmu = Mmu::new(512);
        mmu.load_bytes(0x100, &[0x01, 0x02, 0x03, 0x04]);
        // Little-endian: 0x01 at lowest address → least significant byte
        assert_eq!(mmu.read_u32(0x100), 0x04030201);
    }

    // ── MMIO / UART ──────────────────────────────────────────────────

    #[test]
    fn test_uart_tx_buffer() {
        let mut mmu = Mmu::new(256);
        mmu.write_u8(0x1000_0000, b'H');
        mmu.write_u8(0x1000_0000, b'i');
        assert_eq!(mmu.uart_buffer(), "Hi");
        // Newline flushes the buffer
        mmu.write_u8(0x1000_0000, b'\n');
        assert_eq!(mmu.uart_buffer(), "");
    }

    #[test]
    fn test_uart_tx_does_not_write_ram() {
        let mut mmu = Mmu::new(0x2000_0000); // large enough to cover UART address if it were RAM
        mmu.write_u8(0x1000_0000, b'X');
        // UART writes should be intercepted, not stored in RAM
        assert_eq!(mmu.read_u8(0x1000_0000), 0);
    }

    #[test]
    fn test_uart_rx_returns_zero() {
        let mmu = Mmu::new(256);
        assert_eq!(mmu.read_u8(0x1000_0004), 0);
        assert_eq!(mmu.read_u32(0x1000_0004), 0);
    }

    #[test]
    fn test_uart_write_u32_only_sends_low_byte() {
        let mut mmu = Mmu::new(256);
        mmu.write_u32(0x1000_0000, 0x41); // 0x41 = 'A'
        assert_eq!(mmu.uart_buffer(), "A");
    }

    #[test]
    fn test_vpb_uart0_dr_alias_write() {
        let mut mmu = Mmu::new(256);
        mmu.write_u32(0x101F_1000, 0x42); // 'B' to PL011 DR
        assert_eq!(mmu.uart_buffer(), "B");
    }

    #[test]
    fn test_vpb_uartfr_returns_not_full() {
        let mmu = Mmu::new(256);
        assert_eq!(mmu.read_u32(0x101F_1018), 0);
    }

    #[test]
    fn test_sp804_timer() {
        let mut mmu = Mmu::new(256);

        mmu.write_u32(VPB_TIMER_BASE + 0x00, 10);
        assert_eq!(mmu.read_u32(VPB_TIMER_BASE + 0x04), 10);

        mmu.write_u32(VPB_TIMER_BASE + 0x08, 0x80);

        let mut cpu = crate::cpu::Cpu::new(4096);
        cpu.mmu.timer1_load = mmu.timer1_load;
        cpu.mmu.timer1_value = mmu.timer1_value;
        cpu.mmu.timer1_ctrl = mmu.timer1_ctrl;

        cpu.mmu.write_u32(0, 0xE1A0_0000); // NOP
        cpu.regs.set_pc(0);

        for _ in 0..5 {
            cpu.step();
        }

        assert_eq!(cpu.mmu.timer1_value, 5);
    }

    #[test]
    fn test_vic_enable_and_clear() {
        let mut mmu = Mmu::new(256);

        mmu.vic_int_status = 1 << 4;
        mmu.update_vic();
        assert!(!mmu.irq_pending, "IRQ should be low when line is not enabled");

        mmu.write_u32(VPB_VIC_BASE + 0x010, 1 << 4); // VICIntEnable
        assert_eq!(mmu.vic_int_enable & (1 << 4), 1 << 4);
        assert!(mmu.irq_pending, "IRQ should go high when active line is enabled");

        mmu.write_u32(VPB_VIC_BASE + 0x014, 1 << 4); // VICIntEnClear
        assert_eq!(mmu.vic_int_enable & (1 << 4), 0);
        assert!(!mmu.irq_pending, "IRQ should drop when line is disabled");
    }

    #[test]
    fn test_timer_intclr_clears_vic_line4() {
        let mut mmu = Mmu::new(256);

        mmu.vic_int_status = 1 << 4;
        mmu.write_u32(VPB_VIC_BASE + 0x010, 1 << 4); // enable line 4
        assert!(mmu.irq_pending);

        mmu.write_u32(VPB_TIMER_BASE + 0x0C, 1); // Timer1IntClr
        assert_eq!(mmu.vic_int_status & (1 << 4), 0);
        assert!(!mmu.irq_pending);
    }

    // ── VRAM ─────────────────────────────────────────────────────────

    #[test]
    fn test_vram_write_read_pixel() {
        let mut mmu = Mmu::new(256);
        // Write an RGBA pixel (0xAABBGGRR in LE → R,G,B,A bytes)
        let vram_base: u32 = 0x0400_0000;
        mmu.write_u32(vram_base, 0xFF0000FF); // Red pixel: R=0xFF, G=0x00, B=0x00, A=0xFF
        assert_eq!(mmu.read_u32(vram_base), 0xFF0000FF);
        assert_eq!(mmu.read_u8(vram_base), 0xFF);     // R
        assert_eq!(mmu.read_u8(vram_base + 1), 0x00); // G
        assert_eq!(mmu.read_u8(vram_base + 2), 0x00); // B
        assert_eq!(mmu.read_u8(vram_base + 3), 0xFF); // A
    }

    #[test]
    fn test_vram_does_not_write_ram() {
        let mut mmu = Mmu::new(0x0500_0000); // large enough to cover VRAM range
        let vram_base: u32 = 0x0400_0000;
        mmu.write_u32(vram_base, 0xDEADBEEF);
        // VRAM writes should be intercepted by the VRAM buffer, not stored in RAM
        // Reading via the raw ram vector should still be 0
        let ram_offset = vram_base as usize;
        assert_eq!(mmu.ram[ram_offset], 0);
        // But reading through MMU should return the VRAM value
        assert_eq!(mmu.read_u32(vram_base), 0xDEADBEEF);
    }

    #[test]
    fn test_vram_pixel_at_offset() {
        let mut mmu = Mmu::new(256);
        let vram_base: u32 = 0x0400_0000;
        // Pixel at (100, 50): offset = (50 * 800 + 100) * 4 = 160,400
        let pixel_addr = vram_base + (50 * 800 + 100) * 4;
        mmu.write_u32(pixel_addr, 0xFF00FF00); // Green pixel
        assert_eq!(mmu.read_u32(pixel_addr), 0xFF00FF00);
    }

    #[test]
    fn test_vram_clear_on_reset() {
        let mut mmu = Mmu::new(256);
        let vram_base: u32 = 0x0400_0000;
        mmu.write_u32(vram_base, 0xFFFFFFFF);
        assert_eq!(mmu.read_u32(vram_base), 0xFFFFFFFF);
        mmu.clear_vram();
        // After clear: R=0, G=0, B=0, A=255 → 0xFF000000
        assert_eq!(mmu.read_u32(vram_base), 0xFF000000);
    }

    // ── Input MMIO ───────────────────────────────────────────────────

    #[test]
    fn test_input_key_register() {
        let mut mmu = Mmu::new(256);
        assert_eq!(mmu.read_u32(0x1000_0008), 0); // no key pressed
        mmu.key_state = 65; // 'A' keycode
        assert_eq!(mmu.read_u32(0x1000_0008), 65);
        mmu.key_state = 0; // released
        assert_eq!(mmu.read_u32(0x1000_0008), 0);
    }

    #[test]
    fn test_input_touch_register() {
        let mut mmu = Mmu::new(256);
        assert_eq!(mmu.read_u32(0x1000_000C), 0); // not touching
        mmu.touch_down = true;
        assert_eq!(mmu.read_u32(0x1000_000C), 1);
        mmu.touch_down = false;
        assert_eq!(mmu.read_u32(0x1000_000C), 0);
    }

    #[test]
    fn test_input_coord_register() {
        let mut mmu = Mmu::new(256);
        mmu.touch_x = 400;
        mmu.touch_y = 300;
        let coord = mmu.read_u32(0x1000_0010);
        assert_eq!(coord & 0xFFFF, 400);         // X in low 16 bits
        assert_eq!((coord >> 16) & 0xFFFF, 300);  // Y in high 16 bits
    }

    #[test]
    fn test_sys_timer_register() {
        let mut mmu = Mmu::new(256);
        assert_eq!(mmu.read_u32(0x1000_0014), 0);
        mmu.sys_timer = 42;
        assert_eq!(mmu.read_u32(0x1000_0014), 42);
    }

    #[test]
    fn test_input_registers_not_writable() {
        let mut mmu = Mmu::new(0x2000_0000);
        // Writing to input registers should be ignored
        mmu.write_u32(0x1000_0008, 0xDEAD);
        mmu.write_u32(0x1000_000C, 0xBEEF);
        mmu.write_u32(0x1000_0010, 0xFACE);
        mmu.write_u32(0x1000_0014, 0xCAFE);
        // All should still be 0 (only the host can set them externally)
        assert_eq!(mmu.key_state, 0);
        assert_eq!(mmu.touch_down, false);
        assert_eq!(mmu.sys_timer, 0);
    }

    // ── Audio MMIO ───────────────────────────────────────────────────

    #[test]
    fn test_audio_registers_read_write() {
        let mut mmu = Mmu::new(256);

        // Initially zero
        assert_eq!(mmu.read_u32(0x1000_0018), 0); // AUDIO_CTRL
        assert_eq!(mmu.read_u32(0x1000_001C), 0); // AUDIO_FREQ

        // CPU writes AUDIO_CTRL: enable + sine waveform (bit0=1, bits1-2=01 → 0x03)
        mmu.write_u32(0x1000_0018, 0x03);
        assert_eq!(mmu.audio_ctrl, 0x03);
        assert_eq!(mmu.read_u32(0x1000_0018), 0x03);

        // CPU writes AUDIO_FREQ: 440 Hz
        mmu.write_u32(0x1000_001C, 440);
        assert_eq!(mmu.audio_freq, 440);
        assert_eq!(mmu.read_u32(0x1000_001C), 440);

        // Overwrite with new values
        mmu.write_u32(0x1000_0018, 0x05); // enable + sawtooth (bits1-2=10)
        mmu.write_u32(0x1000_001C, 880);
        assert_eq!(mmu.audio_ctrl, 0x05);
        assert_eq!(mmu.audio_freq, 880);

        // Disable audio (write 0)
        mmu.write_u32(0x1000_0018, 0);
        assert_eq!(mmu.audio_ctrl, 0);
        assert_eq!(mmu.read_u32(0x1000_0018), 0);
    }
