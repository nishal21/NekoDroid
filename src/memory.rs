// ── nekodroid: ARMv7 Memory Management Unit ────────────────────────────
//
// Flat byte-addressable RAM with little-endian read/write operations.
// Includes Memory-Mapped I/O (MMIO) for hardware peripherals.

/// Default RAM size: 16 MB (enough for basic ARM programs)
const DEFAULT_RAM_SIZE: usize = 16 * 1024 * 1024;

// ── MMIO Address Map ──────────────────────────────────────────────────
// Virtual UART (serial port) — base address 0x10000000
const UART_BASE: u32 = 0x1000_0000;
const UART_TX:   u32 = UART_BASE;          // 0x10000000 — Write: transmit byte
const UART_RX:   u32 = UART_BASE + 4;      // 0x10000004 — Read: receive byte (stub)
const UART_END:  u32 = UART_BASE + 8;      // End of UART register range

/// The Memory Management Unit — a flat byte-addressable memory bus
/// with Memory-Mapped I/O (MMIO) support.
///
/// MMIO ranges are intercepted before RAM access:
///   0x10000000 — UART TX (write a byte to serial console)
///   0x10000004 — UART RX (read stub, returns 0)
pub struct Mmu {
    ram: Vec<u8>,
    /// UART transmit buffer — accumulates characters until newline
    uart_tx_buffer: String,
}

impl Mmu {
    /// Creates a new MMU with the given RAM size in bytes.
    pub fn new(size: usize) -> Self {
        Mmu {
            ram: vec![0u8; size],
            uart_tx_buffer: String::new(),
        }
    }

    /// Creates a new MMU with the default 16 MB of RAM.
    pub fn default() -> Self {
        Self::new(DEFAULT_RAM_SIZE)
    }

    /// Returns the total RAM size in bytes.
    pub fn ram_size(&self) -> usize {
        self.ram.len()
    }

    // ── MMIO detection ────────────────────────────────────────────────

    /// Returns true if the address falls within the UART MMIO range.
    fn is_uart(addr: u32) -> bool {
        addr >= UART_BASE && addr < UART_END
    }

    // ── UART TX ───────────────────────────────────────────────────────

    /// Handles a write to the UART TX register.
    /// Appends the byte as a char to the buffer.
    /// On newline (\n), flushes the buffer to the JS console.
    fn uart_write_byte(&mut self, val: u8) {
        let ch = val as char;
        if ch == '\n' {
            // Flush the buffer
            #[cfg(not(test))]
            {
                crate::log(&format!("📟 UART: {}", self.uart_tx_buffer));
            }
            #[cfg(test)]
            {
                // In tests, we just clear — tests check the buffer before flush
            }
            self.uart_tx_buffer.clear();
        } else {
            self.uart_tx_buffer.push(ch);
        }
    }

    /// Returns the current UART TX buffer contents (for testing/debugging).
    pub fn uart_buffer(&self) -> &str {
        &self.uart_tx_buffer
    }

    /// Clears the UART TX buffer (used on CPU reset).
    pub fn clear_uart_buffer(&mut self) {
        self.uart_tx_buffer.clear();
    }

    // ── Read operations (little-endian) ───────────────────────────────

    /// Reads a single byte from the given address.
    /// MMIO addresses are intercepted before RAM access.
    pub fn read_u8(&self, addr: u32) -> u8 {
        // MMIO: UART RX returns 0 (no incoming data)
        if Self::is_uart(addr) {
            return 0;
        }
        let a = addr as usize;
        if a < self.ram.len() {
            self.ram[a]
        } else {
            0 // Out-of-bounds reads return 0 (unmapped memory)
        }
    }

    /// Reads a 16-bit value (little-endian) from the given address.
    pub fn read_u16(&self, addr: u32) -> u16 {
        // MMIO intercept
        if Self::is_uart(addr) {
            return 0;
        }
        let lo = self.read_u8(addr) as u16;
        let hi = self.read_u8(addr.wrapping_add(1)) as u16;
        lo | (hi << 8)
    }

    /// Reads a 32-bit value (little-endian) from the given address.
    pub fn read_u32(&self, addr: u32) -> u32 {
        // MMIO intercept
        if Self::is_uart(addr) {
            return 0;
        }
        let b0 = self.read_u8(addr) as u32;
        let b1 = self.read_u8(addr.wrapping_add(1)) as u32;
        let b2 = self.read_u8(addr.wrapping_add(2)) as u32;
        let b3 = self.read_u8(addr.wrapping_add(3)) as u32;
        b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)
    }

    // ── Write operations (little-endian) ──────────────────────────────

    /// Writes a single byte to the given address.
    /// MMIO addresses are intercepted and routed to the appropriate device.
    pub fn write_u8(&mut self, addr: u32, val: u8) {
        // MMIO: UART TX register
        if addr == UART_TX {
            self.uart_write_byte(val);
            return;
        }
        // MMIO: other UART registers — ignore writes
        if Self::is_uart(addr) {
            return;
        }
        let a = addr as usize;
        if a < self.ram.len() {
            self.ram[a] = val;
        }
        // Out-of-bounds writes are silently ignored
    }

    /// Writes a 16-bit value (little-endian) to the given address.
    pub fn write_u16(&mut self, addr: u32, val: u16) {
        if Self::is_uart(addr) {
            // UART TX: write the low byte only
            if addr == UART_TX {
                self.uart_write_byte((val & 0xFF) as u8);
            }
            return;
        }
        self.write_u8(addr, (val & 0xFF) as u8);
        self.write_u8(addr.wrapping_add(1), ((val >> 8) & 0xFF) as u8);
    }

    /// Writes a 32-bit value (little-endian) to the given address.
    pub fn write_u32(&mut self, addr: u32, val: u32) {
        if Self::is_uart(addr) {
            // UART TX: write the low byte only
            if addr == UART_TX {
                self.uart_write_byte((val & 0xFF) as u8);
            }
            return;
        }
        self.write_u8(addr, (val & 0xFF) as u8);
        self.write_u8(addr.wrapping_add(1), ((val >> 8) & 0xFF) as u8);
        self.write_u8(addr.wrapping_add(2), ((val >> 16) & 0xFF) as u8);
        self.write_u8(addr.wrapping_add(3), ((val >> 24) & 0xFF) as u8);
    }

    // ── Bulk operations ───────────────────────────────────────────────

    /// Loads a byte slice into memory starting at the given address.
    /// Used for loading binary images (kernel, programs) into RAM.
    pub fn load_bytes(&mut self, addr: u32, data: &[u8]) {
        let start = addr as usize;
        let end = start + data.len();
        if end <= self.ram.len() {
            self.ram[start..end].copy_from_slice(data);
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests;
