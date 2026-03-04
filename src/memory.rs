// ── nekodroid: ARMv7 Memory Management Unit ────────────────────────────
//
// Flat byte-addressable RAM with little-endian read/write operations.
// Includes Memory-Mapped I/O (MMIO) for hardware peripherals.

/// Default RAM size: 16 MB (enough for basic ARM programs)
const DEFAULT_RAM_SIZE: usize = 16 * 1024 * 1024;

// ── MMIO Address Map ──────────────────────────────────────────────────
// VRAM — 800×600 RGBA framebuffer at 0x04000000
const VRAM_BASE: u32 = 0x0400_0000;
const VRAM_WIDTH: usize = 800;
const VRAM_HEIGHT: usize = 600;
const VRAM_SIZE: usize = VRAM_WIDTH * VRAM_HEIGHT * 4; // 1,920,000 bytes
const VRAM_END:  u32 = VRAM_BASE + VRAM_SIZE as u32;   // 0x041D4C00

// Virtual UART (serial port) — base address 0x10000000
const UART_BASE: u32 = 0x1000_0000;
const UART_TX:   u32 = UART_BASE;          // 0x10000000 — Write: transmit byte
const UART_RX:   u32 = UART_BASE + 4;      // 0x10000004 — Read: receive byte (stub)

// Input registers (read-only from CPU)
const INPUT_KEY:   u32 = 0x1000_0008; // Read: currently pressed keycode (0 = none)
const INPUT_TOUCH: u32 = 0x1000_000C; // Read: 1 if touching, 0 if not
const INPUT_COORD: u32 = 0x1000_0010; // Read: [Y: 16 bits][X: 16 bits]

// System timer (read-only from CPU)
const SYS_TIMER:   u32 = 0x1000_0014; // Read: frame count (incremented at ~60 Hz)

// Audio Processing Unit (R/W from CPU)
const AUDIO_CTRL:  u32 = 0x1000_0018; // R/W: Bit 0=Enable, Bits 1-2=Waveform (0=Square,1=Sine,2=Saw,3=Tri)
const AUDIO_FREQ:  u32 = 0x1000_001C; // R/W: Frequency in Hz

// End of peripheral register range
const PERIPH_END:  u32 = 0x1000_0020;

// Versatile PB Hardware Base Addresses
const VPB_VIC_BASE: u32   = 0x1014_0000; // Vectored Interrupt Controller
const VPB_TIMER_BASE: u32 = 0x101E_2000; // Dual Timer Module (SP804)
const VPB_UART0_BASE: u32 = 0x101F_1000; // PL011 UART
const VPB_UART0_FR: u32   = VPB_UART0_BASE + 0x18; // Flag Register

// Versatile PB peripheral window
const VPB_PERIPH_START: u32 = 0x1010_0000;
const VPB_PERIPH_END: u32   = 0x101F_FFFF;

/// The Memory Management Unit — a flat byte-addressable memory bus
/// with Memory-Mapped I/O (MMIO) support.
///
/// MMIO ranges are intercepted before RAM access:
///   0x04000000–0x041D4BFF — VRAM (800×600 RGBA framebuffer)
///   0x10000000 — UART TX (write a byte to serial console)
///   0x10000004 — UART RX (read stub, returns 0)
///   0x10000008 — INPUT_KEY (read: current keycode)
///   0x1000000C — INPUT_TOUCH (read: 1 if touching)
///   0x10000010 — INPUT_COORD (read: [Y:16][X:16])
///   0x10000014 — SYS_TIMER (read: frame counter)
///   0x10000018 — AUDIO_CTRL (R/W: enable + waveform select)
///   0x1000001C — AUDIO_FREQ (R/W: frequency in Hz)
pub struct Mmu {
    ram: Vec<u8>,
    /// Video RAM — 800×600 RGBA framebuffer (1,920,000 bytes)
    vram: Vec<u8>,
    /// UART transmit buffer — accumulates characters until newline
    uart_tx_buffer: String,
    /// Currently pressed keycode (0 = no key)
    pub key_state: u32,
    /// Whether the screen is being touched/clicked
    pub touch_down: bool,
    /// Touch/click X coordinate (canvas pixels)
    pub touch_x: u16,
    /// Touch/click Y coordinate (canvas pixels)
    pub touch_y: u16,
    /// System timer — incremented once per frame (~60 Hz)
    pub sys_timer: u32,
    /// Audio control register — Bit 0: enable, Bits 1-2: waveform
    pub audio_ctrl: u32,
    /// Audio frequency register — tone frequency in Hz
    pub audio_freq: u32,
    /// SP804 Timer1 Load register
    pub timer1_load: u32,
    /// SP804 Timer1 Current Value register
    pub timer1_value: u32,
    /// SP804 Timer1 Control register
    pub timer1_ctrl: u32,
    /// PL190 VIC interrupt enable mask
    pub vic_int_enable: u32,
    /// PL190 VIC active interrupt status bits
    pub vic_int_status: u32,
    /// Physical IRQ wire from VIC to CPU
    pub irq_pending: bool,
}

impl Mmu {
    /// Creates a new MMU with the given RAM size in bytes.
    pub fn new(size: usize) -> Self {
        let mut vram = vec![0u8; VRAM_SIZE];
        // Initialize VRAM to black with full alpha
        for pixel in vram.chunks_exact_mut(4) {
            pixel[3] = 255; // A = 0xFF, RGB = 0 (black)
        }
        Mmu {
            ram: vec![0u8; size],
            vram,
            uart_tx_buffer: String::new(),
            key_state: 0,
            touch_down: false,
            touch_x: 0,
            touch_y: 0,
            sys_timer: 0,
            audio_ctrl: 0,
            audio_freq: 0,
            timer1_load: 0,
            timer1_value: 0,
            timer1_ctrl: 0,
            vic_int_enable: 0,
            vic_int_status: 0,
            irq_pending: false,
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

    /// Returns true if the address falls within the VRAM MMIO range.
    fn is_vram(addr: u32) -> bool {
        addr >= VRAM_BASE && addr < VRAM_END
    }

    /// Returns true if the address falls within legacy MMIO range.
    fn is_uart(addr: u32) -> bool {
        addr >= UART_BASE && addr < PERIPH_END
    }

    /// Returns true if the address falls within Versatile PB peripheral range.
    fn is_vpb_periph(addr: u32) -> bool {
        addr >= VPB_PERIPH_START && addr <= VPB_PERIPH_END
    }

    /// Returns true if address is in any emulated peripheral range.
    fn is_periph(addr: u32) -> bool {
        Self::is_uart(addr) || Self::is_vpb_periph(addr)
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

    /// Handles writes to Versatile PB PL011 UART DR register.
    fn vpb_uart_write_byte(&mut self, val: u8) {
        let ch = val as char;
        if ch == '\n' {
            #[cfg(not(test))]
            {
                crate::log(&format!("🐧 KERNEL: {}", self.uart_tx_buffer));
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

    /// Recomputes VIC output wire based on active+enabled interrupts.
    pub fn update_vic(&mut self) {
        self.irq_pending = (self.vic_int_status & self.vic_int_enable) != 0;
    }

    // ── VRAM access ───────────────────────────────────────────────────

    /// Returns a pointer to the VRAM buffer for direct Wasm memory access.
    pub fn vram_ptr(&self) -> *const u8 {
        self.vram.as_ptr()
    }

    /// Returns the VRAM buffer length in bytes.
    pub fn vram_len(&self) -> usize {
        self.vram.len()
    }

    /// Clears VRAM to black (used on CPU reset).
    pub fn clear_vram(&mut self) {
        for pixel in self.vram.chunks_exact_mut(4) {
            pixel[0] = 0;   // R
            pixel[1] = 0;   // G
            pixel[2] = 0;   // B
            pixel[3] = 255; // A
        }
    }

    // ── Read operations (little-endian) ───────────────────────────────

    /// Reads a single byte from the given address.
    /// MMIO addresses are intercepted before RAM access.
    pub fn read_u8(&self, addr: u32) -> u8 {
        // MMIO: VRAM read
        if Self::is_vram(addr) {
            let offset = (addr - VRAM_BASE) as usize;
            return self.vram[offset];
        }
        // MMIO: Peripheral registers — byte reads return the low byte of the 32-bit register
        if Self::is_periph(addr) {
            // Align to register boundary and read full u32, then extract the requested byte
            let aligned = addr & !3;
            let byte_offset = (addr & 3) as usize;
            let word = self.read_periph_u32(aligned);
            return ((word >> (byte_offset * 8)) & 0xFF) as u8;
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
        // VRAM and UART are handled by read_u8 dispatch
        let lo = self.read_u8(addr) as u16;
        let hi = self.read_u8(addr.wrapping_add(1)) as u16;
        lo | (hi << 8)
    }

    /// Reads a 32-bit value (little-endian) from the given address.
    pub fn read_u32(&self, addr: u32) -> u32 {
        // Fast path: VRAM-aligned 32-bit read
        if Self::is_vram(addr) && Self::is_vram(addr.wrapping_add(3)) {
            let offset = (addr - VRAM_BASE) as usize;
            return u32::from_le_bytes([
                self.vram[offset],
                self.vram[offset + 1],
                self.vram[offset + 2],
                self.vram[offset + 3],
            ]);
        }
        // Peripheral register reads
        if Self::is_periph(addr) {
            return self.read_periph_u32(addr);
        }
        let b0 = self.read_u8(addr) as u32;
        let b1 = self.read_u8(addr.wrapping_add(1)) as u32;
        let b2 = self.read_u8(addr.wrapping_add(2)) as u32;
        let b3 = self.read_u8(addr.wrapping_add(3)) as u32;
        b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)
    }

    /// Reads a peripheral MMIO register as a 32-bit value.
    fn read_periph_u32(&self, addr: u32) -> u32 {
        if Self::is_vpb_periph(addr) {
            if addr >= VPB_VIC_BASE && addr < VPB_VIC_BASE + 0x1000 {
                return match addr - VPB_VIC_BASE {
                    0x000 => self.vic_int_status, // VICIRQStatus
                    0x010 => self.vic_int_enable, // VICIntEnable
                    _ => 0,
                };
            }
            if addr >= VPB_TIMER_BASE && addr < VPB_TIMER_BASE + 0x20 {
                return match addr - VPB_TIMER_BASE {
                    0x00 => self.timer1_load,
                    0x04 => self.timer1_value,
                    0x08 => self.timer1_ctrl,
                    _ => 0,
                };
            }
            if addr == VPB_UART0_FR {
                // UARTFR: TXFF (bit 5) clear => transmitter not full.
                return 0;
            }
            if addr >= VPB_VIC_BASE && addr < VPB_VIC_BASE + 0x1000 {
                return 0;
            }
            if addr >= VPB_TIMER_BASE && addr < VPB_TIMER_BASE + 0x1000 {
                return 0;
            }
            return 0;
        }

        match addr {
            UART_TX => 0,     // TX is write-only
            UART_RX => 0,     // RX stub: no incoming data
            INPUT_KEY => self.key_state,
            INPUT_TOUCH => if self.touch_down { 1 } else { 0 },
            INPUT_COORD => ((self.touch_y as u32) << 16) | (self.touch_x as u32),
            SYS_TIMER => self.sys_timer,
            AUDIO_CTRL => self.audio_ctrl,
            AUDIO_FREQ => self.audio_freq,
            _ => 0,           // Unknown register
        }
    }

    // ── Write operations (little-endian) ──────────────────────────────

    /// Writes a single byte to the given address.
    /// MMIO addresses are intercepted and routed to the appropriate device.
    pub fn write_u8(&mut self, addr: u32, val: u8) {
        // MMIO: VRAM write
        if Self::is_vram(addr) {
            let offset = (addr - VRAM_BASE) as usize;
            self.vram[offset] = val;
            return;
        }
        // Versatile PB PL011 UART0 DR write (used by Linux early printk)
        if addr == VPB_UART0_BASE {
            self.vpb_uart_write_byte(val);
            return;
        }

        // MMIO: UART TX register
        if addr == UART_TX {
            self.uart_write_byte(val);
            return;
        }
        // MMIO: Audio registers (writable)
        if addr == AUDIO_CTRL {
            self.audio_ctrl = (self.audio_ctrl & 0xFFFFFF00) | (val as u32);
            return;
        }
        if addr == AUDIO_FREQ {
            self.audio_freq = (self.audio_freq & 0xFFFFFF00) | (val as u32);
            return;
        }
        // MMIO: all other peripheral registers — ignore writes
        if Self::is_periph(addr) {
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
        if Self::is_periph(addr) {
            if addr == VPB_UART0_BASE {
                self.vpb_uart_write_byte((val & 0xFF) as u8);
                return;
            }
            // UART TX: write the low byte only
            if addr == UART_TX {
                self.uart_write_byte((val & 0xFF) as u8);
                return;
            }
            // Audio registers (writable, 16-bit)
            if addr == AUDIO_CTRL {
                self.audio_ctrl = val as u32;
                return;
            }
            if addr == AUDIO_FREQ {
                self.audio_freq = val as u32;
                return;
            }
            return;
        }
        self.write_u8(addr, (val & 0xFF) as u8);
        self.write_u8(addr.wrapping_add(1), ((val >> 8) & 0xFF) as u8);
    }

    /// Writes a 32-bit value (little-endian) to the given address.
    pub fn write_u32(&mut self, addr: u32, val: u32) {
        // Fast path: VRAM-aligned 32-bit write (most common for pixel writes)
        if Self::is_vram(addr) && Self::is_vram(addr.wrapping_add(3)) {
            let offset = (addr - VRAM_BASE) as usize;
            let bytes = val.to_le_bytes();
            self.vram[offset] = bytes[0];
            self.vram[offset + 1] = bytes[1];
            self.vram[offset + 2] = bytes[2];
            self.vram[offset + 3] = bytes[3];
            return;
        }
        if Self::is_periph(addr) {
            if addr == VPB_UART0_BASE {
                self.vpb_uart_write_byte((val & 0xFF) as u8);
                return;
            }
            if addr >= VPB_VIC_BASE && addr < VPB_VIC_BASE + 0x1000 {
                match addr - VPB_VIC_BASE {
                    0x010 => {
                        self.vic_int_enable |= val;
                        self.update_vic();
                    }
                    0x014 => {
                        self.vic_int_enable &= !val;
                        self.update_vic();
                    }
                    _ => {}
                }
                return;
            }
            if addr >= VPB_TIMER_BASE && addr < VPB_TIMER_BASE + 0x20 {
                match addr - VPB_TIMER_BASE {
                    0x00 => {
                        self.timer1_load = val;
                        self.timer1_value = val;
                    }
                    0x04 => self.timer1_value = val,
                    0x08 => self.timer1_ctrl = val,
                    0x0C => {
                        // Timer1IntClr - clear Timer1 interrupt line (VIC line 4)
                        self.vic_int_status &= !(1 << 4);
                        self.update_vic();
                    }
                    _ => {}
                }
                return;
            }
            // UART TX: write the low byte only
            if addr == UART_TX {
                self.uart_write_byte((val & 0xFF) as u8);
                return;
            }
            // Audio registers (writable, 32-bit)
            if addr == AUDIO_CTRL {
                self.audio_ctrl = val;
                return;
            }
            if addr == AUDIO_FREQ {
                self.audio_freq = val;
                return;
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
