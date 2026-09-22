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
        let mut mmu = Mmu::new(256);
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
        let mut mmu = Mmu::new(256);
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
        let mut mmu = Mmu::new(256);
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

    // ── Android MMIO Devices ──────────────────────────────────────────

    #[test]
    fn test_goldfish_gpu_framebuffer_registers() {
        let mut mmu = Mmu::new(256);

        // Default framebuffer config
        assert_eq!(mmu.read_u32(0x1F00_0000), 800);  // FB_WIDTH
        assert_eq!(mmu.read_u32(0x1F00_0004), 600);  // FB_HEIGHT
        assert_eq!(mmu.read_u32(0x1F00_0008), 0);    // FB_FORMAT (RGBA8888)
        assert_eq!(mmu.read_u32(0x1F00_000C), 0);    // FB_ENABLED (false)
        assert_eq!(mmu.read_u32(0x1F00_0010), 0x0400_0000); // FB_ADDR (VRAM_BASE)

        // Enable framebuffer
        mmu.write_u32(0x1F00_000C, 1);
        assert!(mmu.goldfish_fb_config.enabled);
        assert_eq!(mmu.read_u32(0x1F00_000C), 1);

        // Change resolution
        mmu.write_u32(0x1F00_0014, 1280); // Set width
        mmu.write_u32(0x1F00_0018, 720);  // Set height
        assert_eq!(mmu.goldfish_fb_config.width, 1280);
        assert_eq!(mmu.goldfish_fb_config.height, 720);
    }

    #[test]
    fn test_mmc_card_status() {
        let mut mmu = Mmu::new(256);

        // No card mounted - status should indicate no card (bit0=0, bit1=0)
        // Status register is at offset 0x0C
        assert_eq!(mmu.read_u32(0x1C00_000C) & 0x3, 0);

        // Mount a system image (simulate) - at least 2 sectors (1024 bytes)
        mmu.mmc_card_data = Some(vec![0u8; 1024]);

        // Card present and ready - status should be 0x3 (bit0=1 present, bit1=1 ready)
        assert_eq!(mmu.read_u32(0x1C00_000C) & 0x3, 0x3);
    }

    #[test]
    fn test_mmc_sector_read() {
        let mut mmu = Mmu::new(256);
        
        // Create test system image with pattern in first sector
        let mut test_image = vec![0u8; 1024];
        for i in 0..512 {
            test_image[i] = (i % 256) as u8;
        }
        mmu.mmc_card_data = Some(test_image);
        
        // Issue CMD17 (READ_SINGLE) to read sector 0
        mmu.write_u32(0x1C00_0004, 0); // Set argument (sector 0)
        mmu.write_u32(0x1C00_0000, 17); // Issue CMD17
        
        // Check response is success (0)
        assert_eq!(mmu.mmc_resp, 0);
        
        // Read data from data register - should get first 4 bytes
        let data = mmu.read_u32(0x1C00_0010);
        assert_eq!(data & 0xFF, 0); // First byte
        assert_eq!((data >> 8) & 0xFF, 1); // Second byte
        assert_eq!((data >> 16) & 0xFF, 2); // Third byte
        assert_eq!((data >> 24) & 0xFF, 3); // Fourth byte
    }

    #[test]
    fn test_mmc_cmd18_read_multiple_first_sector() {
        let mut mmu = Mmu::new(256);
        let mut test_image = vec![0u8; 1024];
        for i in 0..512 {
            test_image[i] = 0xA0;
        }
        for i in 512..1024 {
            test_image[i] = 0xB0;
        }
        mmu.mmc_card_data = Some(test_image);
        mmu.write_u32(0x1C00_0004, 0);
        mmu.write_u32(0x1C00_0000, 18); // CMD18
        assert_eq!(mmu.mmc_resp, 0);
        let data = mmu.read_u32(0x1C00_0010);
        assert_eq!(data & 0xFF, 0xA0);
        // Next LBA via ARG bump + CMD18 again
        mmu.write_u32(0x1C00_0004, 1);
        mmu.write_u32(0x1C00_0000, 18);
        assert_eq!(mmu.mmc_resp, 0);
        let data2 = mmu.read_u32(0x1C00_0010);
        assert_eq!(data2 & 0xFF, 0xB0);
    }

    #[test]
    fn test_mmc_read_beyond_end() {
        let mut mmu = Mmu::new(256);
        
        // Small image - only 1 sector
        mmu.mmc_card_data = Some(vec![0u8; 512]);
        
        // Try to read sector 10 (way beyond end)
        mmu.write_u32(0x1C00_0004, 10); // Set argument (sector 10)
        mmu.write_u32(0x1C00_0000, 17); // Issue CMD17
        
        // Should return error response
        assert_eq!(mmu.mmc_resp, 0x80000000);
    }

    #[test]
    fn test_goldfish_rtc() {
        let mut mmu = Mmu::new(256);

        // Default RTC is 0
        assert_eq!(mmu.read_u32(0x1010_1000), 0); // Time low
        assert_eq!(mmu.read_u32(0x1010_1004), 0); // Time high

        // Set RTC time (64-bit)
        mmu.write_u32(0x1010_1000, 0x12345678); // Time low
        mmu.write_u32(0x1010_1004, 0x9ABCDEF0); // Time high

        assert_eq!(mmu.read_u32(0x1010_1000), 0x12345678);
        assert_eq!(mmu.read_u32(0x1010_1004), 0x9ABCDEF0);
        assert_eq!(mmu.goldfish_rtc, 0x9ABCDEF0_12345678);
    }

    #[test]
    fn test_binder_version() {
        let mut mmu = Mmu::new(256);

        // Binder protocol version (simplified MMIO surface)
        assert_eq!(mmu.read_u32(0x0A00_0000), 7);
    }

    #[test]
    fn test_binder_transaction_counter() {
        let mut mmu = Mmu::new(256);

        // Initial sequence is 0
        assert_eq!(mmu.binder_seq, 0);

        // Unknown binder offset still bumps seq (legacy probe)
        mmu.write_u32(0x0A00_0040, 0);
        assert_eq!(mmu.binder_seq, 1);

        mmu.write_u32(0x0A00_0044, 0);
        assert_eq!(mmu.binder_seq, 2);
    }

    #[test]
    fn test_binder_write_read_stub() {
        let mut mmu = Mmu::new(4096);
        mmu.write_u32(0x500, 0x1111_0001);
        mmu.write_u32(0x504, 0x2222_0002);
        mmu.write_u32(0x0A00_0004, 8); // write size
        mmu.write_u32(0x0A00_0008, 0x500); // write buf
        mmu.write_u32(0x0A00_000C, 16); // read capacity
        mmu.write_u32(0x0A00_0010, 0x600); // read buf
        mmu.write_u32(0x0A00_0014, 1); // DO_WRITE_READ
        assert_eq!(mmu.binder_status, 0);
        assert_eq!(mmu.binder_tx_count, 2);
        assert_eq!(mmu.read_u32(0x600), 0x7201_0001);
        assert_eq!(mmu.read_u32(0x604), 0x7201_000c);
    }

    #[test]
    fn test_android_logger_registers() {
        let mut mmu = Mmu::new(256);

        // Write to logger registers
        mmu.write_u32(0x1E00_0000, 4); // LOG_REG_PRIO = INFO (4)
        mmu.write_u32(0x1E00_0004, 0x8000); // LOG_REG_TAG = pointer
        mmu.write_u32(0x1E00_0008, 0x9000); // LOG_REG_MSG = pointer

        // Verify writes
        assert_eq!(mmu.read_u32(0x1E00_0000), 4);
        assert_eq!(mmu.read_u32(0x1E00_0004), 0x8000);
        assert_eq!(mmu.read_u32(0x1E00_0008), 0x9000);

        // Check status - should be ready
        let status = mmu.read_u32(0x1E00_0010); // LOG_REG_STATUS
        assert_eq!(status & 0x1, 1); // bit0 = ready
    }

    #[test]
    fn test_android_logger_write_entry() {
        let mut mmu = Mmu::new(65536); // 64KB RAM for strings

        // Store tag string at 0x8000
        let tag_str = b"TestTag\0";
        mmu.load_bytes(0x8000, tag_str);

        // Store message string at 0x9000
        let msg_str = b"Hello from Android!\0";
        mmu.load_bytes(0x9000, msg_str);

        // Set up logger registers
        mmu.write_u32(0x1E00_0000, 4); // LOG_PRIO_INFO
        mmu.write_u32(0x1E00_0004, 0x8000); // tag pointer
        mmu.write_u32(0x1E00_0008, 0x9000); // message pointer

        // Trigger log write (set bit 0 of control register)
        assert_eq!(mmu.logger_buffer.len(), 0);
        mmu.write_u32(0x1E00_000C, 0x1); // LOG_REG_CTRL - trigger

        // Verify log entry was created
        assert_eq!(mmu.logger_buffer.len(), 1);
        let entry = &mmu.logger_buffer[0];
        assert_eq!(entry.priority, 4); // INFO
        assert_eq!(entry.tag, "TestTag");
        assert_eq!(entry.message, "Hello from Android!");

        // Control register should have trigger bit cleared
        assert_eq!(mmu.log_ctrl & 0x1, 0);
    }

    #[test]
    fn test_android_logger_buffer_full() {
        // Need enough RAM to store strings at addresses 0x1000-0x3000
        let mut mmu = Mmu::new(16384); // 16KB RAM

        // Set small buffer capacity
        mmu.logger_max_entries = 3;

        // Add entries until full
        for i in 0..5 {
            let tag = format!("Tag{}\0", i);
            let msg = format!("Message{}\0", i);
            
            // Store strings in RAM at different locations
            let tag_addr = 0x1000 + (i * 32) as u32;
            let msg_addr = 0x2000 + (i * 64) as u32;
            mmu.load_bytes(tag_addr, tag.as_bytes());
            mmu.load_bytes(msg_addr, msg.as_bytes());
            
            mmu.log_prio = 4;
            mmu.log_tag_ptr = tag_addr;
            mmu.log_msg_ptr = msg_addr;
            mmu.execute_log_write();
        }

        // Buffer should only have 3 entries (the last 3)
        assert_eq!(mmu.logger_buffer.len(), 3);
        assert_eq!(mmu.logger_buffer[0].tag, "Tag2");
        assert_eq!(mmu.logger_buffer[2].tag, "Tag4");
    }

    // ── ASHMEM (Anonymous Shared Memory) Tests ───────────────────────

    #[test]
    fn test_ashmem_create_region() {
        let mut mmu = Mmu::new(256);

        // Set size and trigger create
        mmu.write_u32(0x1B00_0004, 4096); // ASHMEM_REG_SIZE = 4KB
        mmu.write_u32(0x1B00_0000, 1);    // ASHMEM_CMD_CREATE

        // Verify region created
        assert_eq!(mmu.ashmem_regions.len(), 1);
        assert_eq!(mmu.ashmem_regions[0].size, 4096);
        assert_eq!(mmu.ashmem_regions[0].data.len(), 4096);
        assert!(!mmu.ashmem_regions[0].pinned);

        // Verify status updated
        assert_eq!(mmu.ashmem_status, 0x1); // Initialized flag
        assert_eq!(mmu.ashmem_data_ptr, 0); // Region ID 0
    }

    #[test]
    fn test_ashmem_pin_unpin() {
        let mut mmu = Mmu::new(256);

        // Create a region
        mmu.write_u32(0x1B00_0004, 1024);
        mmu.write_u32(0x1B00_0000, 1); // CREATE

        // Pin the region
        mmu.write_u32(0x1B00_0010, 0); // Set data_ptr to region 0
        mmu.write_u32(0x1B00_0000, 2); // ASHMEM_CMD_PIN

        assert!(mmu.ashmem_regions[0].pinned);
        assert_eq!(mmu.ashmem_pinned, 1);

        // Unpin the region
        mmu.write_u32(0x1B00_0000, 3); // ASHMEM_CMD_UNPIN

        assert!(!mmu.ashmem_regions[0].pinned);
        assert_eq!(mmu.ashmem_pinned, 0);
    }

    #[test]
    fn test_ashmem_multiple_regions() {
        let mut mmu = Mmu::new(256);

        // Create multiple regions
        for i in 0..3 {
            let size = 1024 * (i + 1);
            mmu.write_u32(0x1B00_0004, size);
            mmu.write_u32(0x1B00_0000, 1); // CREATE
            assert_eq!(mmu.ashmem_data_ptr, i as u32);
        }

        assert_eq!(mmu.ashmem_regions.len(), 3);
        assert_eq!(mmu.ashmem_regions[0].size, 1024);
        assert_eq!(mmu.ashmem_regions[1].size, 2048);
        assert_eq!(mmu.ashmem_regions[2].size, 3072);
    }

    #[test]
    fn test_ashmem_purge_unpinned() {
        let mut mmu = Mmu::new(256);

        // Create two regions
        mmu.write_u32(0x1B00_0004, 1024);
        mmu.write_u32(0x1B00_0000, 1); // CREATE region 0

        mmu.write_u32(0x1B00_0004, 1024);
        mmu.write_u32(0x1B00_0000, 1); // CREATE region 1

        // Pin region 0
        mmu.write_u32(0x1B00_0010, 0);
        mmu.write_u32(0x1B00_0000, 2); // PIN region 0

        // Purge all unpinned regions
        mmu.write_u32(0x1B00_0000, 4); // PURGE

        // Region 0 should still have data (pinned)
        assert_eq!(mmu.ashmem_regions[0].data.len(), 1024);

        // Region 1 should have been purged (unpinned)
        assert!(mmu.ashmem_regions[1].data.is_empty());
    }

    #[test]
    fn test_mmc_data_port_advances() {
        let mut mmu = Mmu::new(256);
        let mut test_image = vec![0u8; 512];
        for i in 0..512 {
            test_image[i] = (i % 256) as u8;
        }
        mmu.mmc_card_data = Some(test_image);
        mmu.write_u32(0x1C00_0004, 0);
        mmu.write_u32(0x1C00_0000, 17);
        let w0 = mmu.read_u32(0x1C00_0010);
        let w1 = mmu.read_u32(0x1C00_0010);
        assert_eq!(w0 & 0xFF, 0);
        assert_eq!(w1 & 0xFF, 4);
        assert_eq!(mmu.mmc_buffer_offset.get(), 8);
    }

    #[test]
    fn test_mmc_cmd18_auto_advance_and_stop() {
        let mut mmu = Mmu::new(256);
        let mut test_image = vec![0u8; 1024];
        for i in 0..512 {
            test_image[i] = 0x11;
        }
        for i in 512..1024 {
            test_image[i] = 0x22;
        }
        mmu.mmc_card_data = Some(test_image);
        mmu.write_u32(0x1C00_0004, 0);
        mmu.write_u32(0x1C00_0000, 18); // CMD18
        assert!(mmu.mmc_multi_active.get());
        // Drain first sector (128 words)
        for _ in 0..128 {
            let _ = mmu.read_u32(0x1C00_0010);
        }
        assert_eq!(mmu.mmc_current_sector.get(), 1);
        let next = mmu.read_u32(0x1C00_0010);
        assert_eq!(next & 0xFF, 0x22);
        mmu.write_u32(0x1C00_0000, 12); // CMD12 stop
        assert!(!mmu.mmc_multi_active.get());
        assert_eq!(mmu.mmc_resp, 0);
    }

    #[test]
    fn test_goldfish_pipe_version_and_open() {
        let mut mmu = Mmu::new(4096);
        assert_eq!(mmu.read_u32(0x1D00_0020), 1); // VERSION

        let name = b"pipe:qemud:boot-properties\0";
        mmu.load_bytes(0x100, name);
        mmu.write_u32(0x1D00_0010, 0x100); // ADDRESS
        mmu.write_u32(0x1D00_000C, name.len() as u32); // SIZE
        mmu.write_u32(0x1D00_0000, 1); // OPEN
        assert_eq!(mmu.pipe_status, 0);
        assert_eq!(mmu.pipe_channel, 1);
        assert_eq!(mmu.pipe_channels[0], "pipe:qemud:boot-properties");

        mmu.write_u32(0x1D00_0000, 3); // POLL
        assert_eq!(mmu.pipe_status, 0);
        assert!(mmu.pipe_wakes & 2 != 0); // OUT

        mmu.write_u32(0x1D00_0000, 2); // CLOSE
        assert_eq!(mmu.pipe_status, 0);
        assert!(mmu.pipe_channels[0].is_empty());
    }

    #[test]
    fn test_goldfish_pipe_write_read_roundtrip() {
        let mut mmu = Mmu::new(4096);
        let name = b"pipe:qemud:test\0";
        mmu.load_bytes(0x200, name);
        mmu.write_u32(0x1D00_0010, 0x200);
        mmu.write_u32(0x1D00_000C, name.len() as u32);
        mmu.write_u32(0x1D00_0000, 1); // OPEN

        let payload = b"ping";
        mmu.load_bytes(0x300, payload);
        mmu.write_u32(0x1D00_0010, 0x300);
        mmu.write_u32(0x1D00_000C, payload.len() as u32);
        mmu.write_u32(0x1D00_0000, 4); // WRITE
        assert_eq!(mmu.pipe_status, 0);
        assert!(!mmu.pipe_rx.is_empty());

        mmu.write_u32(0x1D00_0010, 0x400);
        mmu.write_u32(0x1D00_000C, 8);
        mmu.write_u32(0x1D00_0000, 5); // READ
        assert_eq!(mmu.pipe_status, 0);
        assert_eq!(mmu.read_u8(0x400), b'o');
        assert_eq!(mmu.read_u8(0x401), b'k');
    }
