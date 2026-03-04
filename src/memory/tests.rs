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
