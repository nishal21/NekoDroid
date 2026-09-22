// ── nekodroid: ARMv7 Memory Management Unit ────────────────────────────
//
// Flat byte-addressable RAM with little-endian read/write operations.
// Includes Memory-Mapped I/O (MMIO) for hardware peripherals.

use std::cell::{Cell, RefCell};

/// Default RAM size: 16 MB (enough for basic ARM programs)
const DEFAULT_RAM_SIZE: usize = 16 * 1024 * 1024;

// ── MMIO Address Map ──────────────────────────────────────────────────
// VRAM — 800×600 RGBA framebuffer at 0x04000000
pub const VRAM_BASE: u32 = 0x0400_0000;
pub const VRAM_WIDTH: usize = 800;
pub const VRAM_HEIGHT: usize = 600;
pub const VRAM_SIZE: usize = VRAM_WIDTH * VRAM_HEIGHT * 4; // 1,920,000 bytes
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

// ── Android-specific MMIO regions ─────────────────────────────────────
// Goldfish GPU / Framebuffer (for Android graphics)
const GOLDFISH_FB_BASE: u32 = 0x1F00_0000;
const GOLDFISH_FB_SIZE: u32 = 0x0100_0000; // 16MB for high-res framebuffer

// Goldfish Pipe (for host communication, used by Android)
const GOLDFISH_PIPE_BASE: u32 = 0x1D00_0000;
const GOLDFISH_PIPE_SIZE: u32 = 0x0010_0000;

// Android Binder IPC driver MMIO region
const BINDER_BASE: u32 = 0x0A00_0000; // Moved to avoid VRAM conflict
const BINDER_SIZE: u32 = 0x0010_0000;

// Goldfish RTC (Real-Time Clock for Android)
const GOLDFISH_RTC_BASE: u32 = 0x1010_1000;

// SD/MMC Card interface (for mounting Android system images)
const MMC_BASE: u32 = 0x1C00_0000;
const MMC_SIZE: u32 = 0x0001_0000;

// MMC Command register offsets
const MMC_REG_CMD: u32 = 0x00;      // Command register
const MMC_REG_ARG: u32 = 0x04;      // Argument (sector address)
const MMC_REG_RESP: u32 = 0x08;     // Response
const MMC_REG_STATUS: u32 = 0x0C;    // Status
const MMC_REG_DATA: u32 = 0x10;      // Data port
const MMC_REG_CTRL: u32 = 0x14;     // Control

// MMC Commands
const MMC_CMD_READ_SINGLE: u32 = 17;  // CMD17: READ_SINGLE_BLOCK
const MMC_CMD_READ_MULTIPLE: u32 = 18; // CMD18: READ_MULTIPLE_BLOCK
const MMC_CMD_STOP_TRANSMISSION: u32 = 12; // CMD12: STOP_TRANSMISSION
#[allow(dead_code)]
const MMC_CMD_WRITE_SINGLE: u32 = 24; // CMD24: WRITE_SINGLE_BLOCK
const MMC_CMD_SEND_STATUS: u32 = 13;  // CMD13: SEND_STATUS
const MMC_CMD_APP_CMD: u32 = 55;      // CMD55: APP_CMD
#[allow(dead_code)]
const MMC_CMD_SD_SEND_OP_COND: u32 = 41; // ACMD41: SD_SEND_OP_COND

// Goldfish Pipe (QEMU-compatible register subset)
const PIPE_REG_COMMAND: u32 = 0x00;
const PIPE_REG_STATUS: u32 = 0x04;
const PIPE_REG_CHANNEL: u32 = 0x08;
const PIPE_REG_SIZE: u32 = 0x0c;
const PIPE_REG_ADDRESS: u32 = 0x10;
const PIPE_REG_WAKES: u32 = 0x14;
const PIPE_REG_VERSION: u32 = 0x20;

const PIPE_CMD_OPEN: u32 = 1;
const PIPE_CMD_CLOSE: u32 = 2;
const PIPE_CMD_POLL: u32 = 3;
const PIPE_CMD_WRITE: u32 = 4;
const PIPE_CMD_READ: u32 = 5;

const PIPE_ERROR_INVAL: i32 = -1;
const PIPE_ERROR_AGAIN: i32 = -2;
const PIPE_POLL_IN: u32 = 1;
const PIPE_POLL_OUT: u32 = 2;

// Android Logger (logcat) interface
const ANDROID_LOG_BASE: u32 = 0x1E00_0000;
const ANDROID_LOG_SIZE: u32 = 0x0001_0000;

// Logger register offsets
const LOG_REG_PRIO: u32 = 0x00;     // Priority (VERBOSE=2, DEBUG=3, INFO=4, WARN=5, ERROR=6)
const LOG_REG_TAG: u32 = 0x04;      // Tag string pointer (32-bit address)
const LOG_REG_MSG: u32 = 0x08;      // Message string pointer (32-bit address)
const LOG_REG_CTRL: u32 = 0x0C;     // Control: bit0=write trigger
const LOG_REG_STATUS: u32 = 0x10;   // Status: bit0=ready, bit1=buffer full

// Android log priorities
#[allow(dead_code)]
const LOG_PRIO_VERBOSE: u8 = 2;
#[allow(dead_code)]
const LOG_PRIO_DEBUG: u8 = 3;
#[allow(dead_code)]
const LOG_PRIO_INFO: u8 = 4;
#[allow(dead_code)]
const LOG_PRIO_WARN: u8 = 5;
#[allow(dead_code)]
const LOG_PRIO_ERROR: u8 = 6;
#[allow(dead_code)]
const LOG_PRIO_FATAL: u8 = 7;

// ASHMEM (Anonymous Shared Memory) for Android Binder IPC
const ASHMEM_BASE: u32 = 0x1B00_0000;
const ASHMEM_SIZE: u32 = 0x0001_0000;

// ASHMEM ioctl-like commands (simplified MMIO interface)
const ASHMEM_REG_CMD: u32 = 0x00;      // Command register
const ASHMEM_REG_SIZE: u32 = 0x04;     // Size register (set/get)
const ASHMEM_REG_PROT: u32 = 0x08;     // Protection mask
const ASHMEM_REG_PINNED: u32 = 0x0C;   // Pin status
const ASHMEM_REG_DATA_PTR: u32 = 0x10; // Data pointer (returned after create)
const ASHMEM_REG_STATUS: u32 = 0x14;   // Status: bit0=initialized

// ASHMEM commands
const ASHMEM_CMD_CREATE: u32 = 1;   // Create shared memory region
const ASHMEM_CMD_PIN: u32 = 2;      // Pin region
const ASHMEM_CMD_UNPIN: u32 = 3;    // Unpin region
const ASHMEM_CMD_PURGE: u32 = 4;    // Purge all caches

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
    
    // ── Android-specific state ─────────────────────────────────────────
    /// Android logcat logger ring buffer
    pub logger_buffer: Vec<LogEntry>,
    /// Goldfish framebuffer state (width, height, format)
    pub goldfish_fb_config: FbConfig,
    /// SD/MMC card mounted image data
    pub mmc_card_data: Option<Vec<u8>>,
    /// Goldfish RTC time value
    pub goldfish_rtc: u64,
    /// Binder transaction sequence counter
    pub binder_seq: u32,
    
    // ── SD/MMC Card Interface ──────────────────────────────────────────
    /// MMC command register
    pub mmc_cmd: u32,
    /// MMC argument register (sector address)
    pub mmc_arg: u32,
    /// MMC response register
    pub mmc_resp: u32,
    /// MMC status register
    pub mmc_status: u32,
    /// MMC data register (for sector reads)
    pub mmc_data: u32,
    /// Current sector being read
    pub mmc_current_sector: Cell<u32>,
    /// Sector data buffer (512 bytes for standard sector)
    pub mmc_sector_buffer: RefCell<[u8; 512]>,
    /// Byte offset within sector buffer
    pub mmc_buffer_offset: Cell<usize>,
    /// CMD18 multi-block transfer active until CMD12
    pub mmc_multi_active: Cell<bool>,

    // ── Goldfish Pipe ────────────────────────────────────────────────
    pub pipe_cmd: u32,
    pub pipe_status: i32,
    pub pipe_channel: u32,
    pub pipe_size: u32,
    pub pipe_address: u32,
    pub pipe_wakes: u32,
    /// Opened pipe channels (id -> service name)
    pub pipe_channels: Vec<String>,
    /// Host→guest scratch for pipe reads (e.g. ping replies)
    pub pipe_rx: Vec<u8>,

    // ── Android Logger ────────────────────────────────────────────────
    /// Logger device read position
    pub logger_read_pos: usize,
    /// Logger device entries limit
    pub logger_max_entries: usize,
    /// Android Logger priority register
    pub log_prio: u32,
    /// Android Logger tag pointer register
    pub log_tag_ptr: u32,
    /// Android Logger message pointer register
    pub log_msg_ptr: u32,
    /// Android Logger control register
    pub log_ctrl: u32,
    
    // ── ASHMEM (Anonymous Shared Memory) ─────────────────────────────
    /// ASHMEM command register
    pub ashmem_cmd: u32,
    /// ASHMEM size register
    pub ashmem_size: u32,
    /// ASHMEM protection mask
    pub ashmem_prot: u32,
    /// ASHMEM pinned status
    pub ashmem_pinned: u32,
    /// ASHMEM data pointer (address in emulator RAM)
    pub ashmem_data_ptr: u32,
    /// ASHMEM status register
    pub ashmem_status: u32,
    /// Shared memory regions pool
    pub ashmem_regions: Vec<SharedMemoryRegion>,
}

/// Android shared memory region (ASHMEM)
#[derive(Debug, Clone)]
pub struct SharedMemoryRegion {
    pub id: u32,
    pub size: usize,
    pub data: Vec<u8>,
    pub prot_mask: u32,
    pub pinned: bool,
    pub mapped_addr: Option<u32>, // If mapped, the virtual address
}

/// Android logcat entry
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub priority: u8,
    pub tag: String,
    pub message: String,
}

/// Goldfish framebuffer configuration
#[derive(Debug, Clone, Copy)]
pub struct FbConfig {
    pub width: u32,
    pub height: u32,
    pub format: u32, // 0=RGBA8888, 1=RGB565, etc.
    pub enabled: bool,
}

impl Default for FbConfig {
    fn default() -> Self {
        Self {
            width: 800,
            height: 600,
            format: 0, // RGBA8888
            enabled: false,
        }
    }
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
            // Android fields
            logger_buffer: Vec::with_capacity(1024),
            goldfish_fb_config: FbConfig::default(),
            mmc_card_data: None,
            goldfish_rtc: 0,
            binder_seq: 0,
            // SD/MMC fields
            mmc_cmd: 0,
            mmc_arg: 0,
            mmc_resp: 0,
            mmc_status: 0,
            mmc_data: 0,
            mmc_current_sector: Cell::new(0),
            mmc_sector_buffer: RefCell::new([0u8; 512]),
            mmc_buffer_offset: Cell::new(0),
            mmc_multi_active: Cell::new(false),
            // Goldfish Pipe
            pipe_cmd: 0,
            pipe_status: 0,
            pipe_channel: 0,
            pipe_size: 0,
            pipe_address: 0,
            pipe_wakes: 0,
            pipe_channels: Vec::new(),
            pipe_rx: Vec::new(),
            // Android Logger fields
            logger_read_pos: 0,
            logger_max_entries: 1024,
            log_prio: 0,
            log_tag_ptr: 0,
            log_msg_ptr: 0,
            log_ctrl: 0,
            // ASHMEM fields
            ashmem_cmd: 0,
            ashmem_size: 0,
            ashmem_prot: 0,
            ashmem_pinned: 0,
            ashmem_data_ptr: 0,
            ashmem_status: 0,
            ashmem_regions: Vec::new(),
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
        Self::is_uart(addr) || Self::is_vpb_periph(addr) || Self::is_android_periph(addr)
    }

    // ── Android MMIO detection ────────────────────────────────────────

    /// Returns true if address is in Android-specific MMIO regions.
    fn is_android_periph(addr: u32) -> bool {
        // Goldfish GPU framebuffer
        (addr >= GOLDFISH_FB_BASE && addr < GOLDFISH_FB_BASE + GOLDFISH_FB_SIZE) ||
        // Goldfish Pipe
        (addr >= GOLDFISH_PIPE_BASE && addr < GOLDFISH_PIPE_BASE + GOLDFISH_PIPE_SIZE) ||
        // Binder IPC
        (addr >= BINDER_BASE && addr < BINDER_BASE + BINDER_SIZE) ||
        // SD/MMC card
        (addr >= MMC_BASE && addr < MMC_BASE + MMC_SIZE) ||
        // Goldfish RTC
        (addr >= GOLDFISH_RTC_BASE && addr < GOLDFISH_RTC_BASE + 0x1000) ||
        // Android Logger (logcat)
        (addr >= ANDROID_LOG_BASE && addr < ANDROID_LOG_BASE + ANDROID_LOG_SIZE) ||
        // ASHMEM (Anonymous Shared Memory)
        (addr >= ASHMEM_BASE && addr < ASHMEM_BASE + ASHMEM_SIZE)
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

    // ── SD/MMC Command Execution ─────────────────────────────────────

    /// Executes an MMC command that was written to the CMD register.
    /// This handles sector reads from the mounted system image.
    fn execute_mmc_command(&mut self) {
        let cmd = self.mmc_cmd & 0x3F; // Extract command index (6 bits)
        let arg = self.mmc_arg; // Sector address

        match cmd {
            MMC_CMD_STOP_TRANSMISSION => {
                self.mmc_multi_active.set(false);
                self.mmc_resp = 0;
            }
            MMC_CMD_READ_SINGLE => {
                self.mmc_multi_active.set(false);
                self.load_mmc_sector_cmd(arg);
            }
            MMC_CMD_READ_MULTIPLE => {
                self.mmc_multi_active.set(true);
                self.load_mmc_sector_cmd(arg);
            }
            MMC_CMD_SEND_STATUS => {
                self.mmc_resp = if self.mmc_card_data.is_some() { 0x00000001 } else { 0x00000000 };
            }
            MMC_CMD_APP_CMD => {
                self.mmc_resp = 0;
            }
            _ => {
                self.mmc_resp = 0x80000000; // Error flag
            }
        }
    }

    fn load_mmc_sector(&self, arg: u32) {
        self.mmc_current_sector.set(arg);
        self.mmc_buffer_offset.set(0);
        let mut buf = self.mmc_sector_buffer.borrow_mut();
        if let Some(ref card_data) = self.mmc_card_data {
            let sector_offset = (arg as usize) * 512;
            if sector_offset + 512 <= card_data.len() {
                buf.copy_from_slice(&card_data[sector_offset..sector_offset + 512]);
                // resp updated via Cell-less field — only safe from &mut callers;
                // for &self auto-advance we leave resp at 0 on success.
            } else {
                buf.fill(0);
                self.mmc_multi_active.set(false);
            }
        } else {
            buf.fill(0);
            self.mmc_multi_active.set(false);
        }
        drop(buf);
    }

    fn load_mmc_sector_cmd(&mut self, arg: u32) {
        self.load_mmc_sector(arg);
        if let Some(ref card_data) = self.mmc_card_data {
            let sector_offset = (arg as usize) * 512;
            if sector_offset + 512 <= card_data.len() {
                self.mmc_resp = 0;
            } else {
                self.mmc_resp = 0x80000000;
                self.mmc_multi_active.set(false);
            }
        } else {
            self.mmc_resp = 0x80000000;
            self.mmc_multi_active.set(false);
        }
    }

    /// Reads 4 bytes from the MMC data port, advancing the sector buffer.
    /// Under CMD18, auto-loads the next LBA when a sector is fully consumed.
    fn read_mmc_data_word(&self) -> u32 {
        let offset = self.mmc_buffer_offset.get();
        if offset >= 512 {
            return 0;
        }
        let word = {
            let buf = self.mmc_sector_buffer.borrow();
            let b0 = buf[offset] as u32;
            let b1 = buf.get(offset + 1).copied().unwrap_or(0) as u32;
            let b2 = buf.get(offset + 2).copied().unwrap_or(0) as u32;
            let b3 = buf.get(offset + 3).copied().unwrap_or(0) as u32;
            b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)
        };
        self.mmc_buffer_offset.set(offset + 4);

        if self.mmc_buffer_offset.get() >= 512 && self.mmc_multi_active.get() {
            let next = self.mmc_current_sector.get().wrapping_add(1);
            self.load_mmc_sector(next);
        }
        word
    }

    fn execute_pipe_command(&mut self) {
        match self.pipe_cmd {
            PIPE_CMD_OPEN => {
                let len = self.pipe_size.min(256) as usize;
                let mut name = String::new();
                for i in 0..len {
                    let b = self.read_u8(self.pipe_address.wrapping_add(i as u32));
                    if b == 0 {
                        break;
                    }
                    name.push(b as char);
                }
                if name.is_empty() {
                    self.pipe_status = PIPE_ERROR_INVAL;
                } else {
                    self.pipe_channels.push(name);
                    self.pipe_channel = self.pipe_channels.len() as u32; // 1-based id
                    self.pipe_status = 0;
                    self.pipe_wakes = PIPE_POLL_OUT;
                }
            }
            PIPE_CMD_CLOSE => {
                let id = self.pipe_channel as usize;
                if id == 0 || id > self.pipe_channels.len() {
                    self.pipe_status = PIPE_ERROR_INVAL;
                } else {
                    self.pipe_channels[id - 1].clear();
                    self.pipe_status = 0;
                    self.pipe_wakes = 0;
                }
            }
            PIPE_CMD_POLL => {
                let id = self.pipe_channel as usize;
                if id == 0 || id > self.pipe_channels.len() || self.pipe_channels[id - 1].is_empty() {
                    self.pipe_status = PIPE_ERROR_INVAL;
                } else {
                    let mut wake = PIPE_POLL_OUT;
                    if !self.pipe_rx.is_empty() {
                        wake |= PIPE_POLL_IN;
                    }
                    self.pipe_wakes = wake;
                    self.pipe_status = 0;
                }
            }
            PIPE_CMD_WRITE => {
                // Host accepts guest writes; ping services get a canned reply.
                let id = self.pipe_channel as usize;
                if id == 0 || id > self.pipe_channels.len() || self.pipe_channels[id - 1].is_empty() {
                    self.pipe_status = PIPE_ERROR_INVAL;
                    return;
                }
                let len = self.pipe_size.min(4096) as usize;
                let mut buf = vec![0u8; len];
                for i in 0..len {
                    buf[i] = self.read_u8(self.pipe_address.wrapping_add(i as u32));
                }
                let svc = self.pipe_channels[id - 1].as_str();
                if svc.contains("qemud") || svc.contains("pipe") {
                    self.pipe_rx.extend_from_slice(b"ok\n");
                    self.pipe_wakes = PIPE_POLL_IN | PIPE_POLL_OUT;
                }
                let _ = buf;
                self.pipe_status = 0;
            }
            PIPE_CMD_READ => {
                let want = self.pipe_size.min(4096) as usize;
                if self.pipe_rx.is_empty() {
                    self.pipe_status = PIPE_ERROR_AGAIN;
                    return;
                }
                let n = want.min(self.pipe_rx.len());
                let chunk: Vec<u8> = self.pipe_rx.drain(..n).collect();
                for (i, b) in chunk.iter().enumerate() {
                    self.write_u8(self.pipe_address.wrapping_add(i as u32), *b);
                }
                self.pipe_size = n as u32;
                self.pipe_status = 0;
                if self.pipe_rx.is_empty() {
                    self.pipe_wakes = PIPE_POLL_OUT;
                }
            }
            _ => {
                self.pipe_status = PIPE_ERROR_INVAL;
            }
        }
    }

    /// Executes a log write when the logger control register is triggered.
    /// Reads tag and message strings from RAM and adds a LogEntry to the buffer.
    fn execute_log_write(&mut self) {
        // Read priority (lowest byte)
        let priority = (self.log_prio & 0xFF) as u8;
        
        // Read tag string from RAM (tag_ptr)
        let tag = self.read_string_from_ram(self.log_tag_ptr, 32);
        
        // Read message string from RAM (msg_ptr)
        let message = self.read_string_from_ram(self.log_msg_ptr, 256);
        
        // Create log entry
        let entry = LogEntry {
            priority,
            tag,
            message,
        };
        
        // Add to buffer, removing oldest if at capacity
        if self.logger_buffer.len() >= self.logger_max_entries {
            self.logger_buffer.remove(0);
        }
        self.logger_buffer.push(entry);
    }

    /// Reads a null-terminated string from RAM at the given address.
    /// Returns at most max_len characters.
    fn read_string_from_ram(&self, addr: u32, max_len: usize) -> String {
        let mut result = String::with_capacity(max_len);
        let mut offset = 0u32;
        
        while offset < max_len as u32 {
            let byte = self.read_u8(addr.wrapping_add(offset));
            if byte == 0 {
                break;
            }
            result.push(byte as char);
            offset += 1;
        }
        
        result
    }

    // ── ASHMEM Command Execution ──────────────────────────────────────

    /// Executes an ASHMEM command when the CMD register is written.
    /// Handles CREATE, PIN, UNPIN, and PURGE operations.
    fn execute_ashmem_command(&mut self) {
        let cmd = self.ashmem_cmd;
        let size = self.ashmem_size as usize;
        let prot = self.ashmem_prot;

        match cmd {
            ASHMEM_CMD_CREATE => {
                // Create a new shared memory region
                let id = self.ashmem_regions.len() as u32;
                let region = SharedMemoryRegion {
                    id,
                    size,
                    data: vec![0u8; size],
                    prot_mask: prot,
                    pinned: false,
                    mapped_addr: None,
                };
                self.ashmem_regions.push(region);
                // Return region info via registers
                self.ashmem_data_ptr = id; // Use ID as data pointer for now
                self.ashmem_status = 0x1; // Initialized flag set
            }
            ASHMEM_CMD_PIN => {
                // Pin a region (mark as locked/unswappable)
                let region_id = self.ashmem_data_ptr as usize;
                if region_id < self.ashmem_regions.len() {
                    self.ashmem_regions[region_id].pinned = true;
                    self.ashmem_pinned = 1;
                }
            }
            ASHMEM_CMD_UNPIN => {
                // Unpin a region
                let region_id = self.ashmem_data_ptr as usize;
                if region_id < self.ashmem_regions.len() {
                    self.ashmem_regions[region_id].pinned = false;
                    self.ashmem_pinned = 0;
                }
            }
            ASHMEM_CMD_PURGE => {
                // Purge all unpinned regions (free their data but keep metadata)
                for region in &mut self.ashmem_regions {
                    if !region.pinned {
                        region.data.clear();
                        region.data.shrink_to_fit();
                    }
                }
            }
            _ => {}
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
        // Android-specific MMIO devices
        if Self::is_android_periph(addr) {
            // Goldfish GPU - framebuffer info registers
            if addr >= GOLDFISH_FB_BASE && addr < GOLDFISH_FB_BASE + 0x100 {
                return match addr - GOLDFISH_FB_BASE {
                    0x00 => self.goldfish_fb_config.width,    // FB_WIDTH
                    0x04 => self.goldfish_fb_config.height,   // FB_HEIGHT
                    0x08 => self.goldfish_fb_config.format,   // FB_FORMAT
                    0x0C => if self.goldfish_fb_config.enabled { 1 } else { 0 }, // FB_ENABLED
                    0x10 => VRAM_BASE, // FB_ADDR (physical address of framebuffer)
                    _ => 0,
                };
            }
            // SD/MMC card interface - full command register set
            if addr >= MMC_BASE && addr < MMC_BASE + MMC_SIZE {
                let reg_offset = addr - MMC_BASE;
                return match reg_offset {
                    MMC_REG_CMD => self.mmc_cmd,
                    MMC_REG_ARG => self.mmc_arg,
                    MMC_REG_RESP => self.mmc_resp,
                    MMC_REG_STATUS => {
                        // bit0=present, bit1=ready, bit2=data avail, bit3=multi active
                        let card_present = if self.mmc_card_data.is_some() { 1 } else { 0 };
                        let ready = if self.mmc_card_data.is_some() { 1 } else { 0 };
                        let data_avail = if self.mmc_buffer_offset.get() < 512 { 1 } else { 0 };
                        let multi = if self.mmc_multi_active.get() { 1 } else { 0 };
                        card_present | (ready << 1) | (data_avail << 2) | (multi << 3)
                    }
                    MMC_REG_DATA => self.read_mmc_data_word(),
                    MMC_REG_CTRL => 0, // Control register - stub
                    _ => 0,
                };
            }
            // Goldfish RTC
            if addr >= GOLDFISH_RTC_BASE && addr < GOLDFISH_RTC_BASE + 0x1000 {
                return match addr - GOLDFISH_RTC_BASE {
                    0x00 => (self.goldfish_rtc & 0xFFFFFFFF) as u32, // Time low
                    0x04 => (self.goldfish_rtc >> 32) as u32,        // Time high
                    _ => 0,
                };
            }
            // Binder IPC - return protocol version/status
            if addr >= BINDER_BASE && addr < BINDER_BASE + 0x100 {
                return match addr - BINDER_BASE {
                    0x00 => 0x00000001, // BINDER_VERSION
                    _ => 0,
                };
            }
            // Goldfish Pipe — QEMU-compatible probe + open/close/poll/rw
            if addr >= GOLDFISH_PIPE_BASE && addr < GOLDFISH_PIPE_BASE + GOLDFISH_PIPE_SIZE {
                let off = addr - GOLDFISH_PIPE_BASE;
                return match off {
                    PIPE_REG_COMMAND => self.pipe_cmd,
                    PIPE_REG_STATUS => self.pipe_status as u32,
                    PIPE_REG_CHANNEL => self.pipe_channel,
                    PIPE_REG_SIZE => self.pipe_size,
                    PIPE_REG_ADDRESS => self.pipe_address,
                    PIPE_REG_WAKES => self.pipe_wakes,
                    PIPE_REG_VERSION => 1,
                    _ => 0,
                };
            }
            // Android Logger (logcat) interface
            if addr >= ANDROID_LOG_BASE && addr < ANDROID_LOG_BASE + ANDROID_LOG_SIZE {
                let reg_offset = addr - ANDROID_LOG_BASE;
                return match reg_offset {
                    LOG_REG_PRIO => self.log_prio,
                    LOG_REG_TAG => self.log_tag_ptr,
                    LOG_REG_MSG => self.log_msg_ptr,
                    LOG_REG_CTRL => self.log_ctrl,
                    LOG_REG_STATUS => {
                        // bit0 = ready, bit1 = buffer full
                        let ready = 1;
                        let buffer_full = if self.logger_buffer.len() >= self.logger_max_entries { 1 } else { 0 };
                        ready | (buffer_full << 1)
                    }
                    _ => 0,
                };
            }
            // ASHMEM (Anonymous Shared Memory) interface
            if addr >= ASHMEM_BASE && addr < ASHMEM_BASE + ASHMEM_SIZE {
                let reg_offset = addr - ASHMEM_BASE;
                return match reg_offset {
                    ASHMEM_REG_CMD => self.ashmem_cmd,
                    ASHMEM_REG_SIZE => self.ashmem_size,
                    ASHMEM_REG_PROT => self.ashmem_prot,
                    ASHMEM_REG_PINNED => self.ashmem_pinned,
                    ASHMEM_REG_DATA_PTR => self.ashmem_data_ptr,
                    ASHMEM_REG_STATUS => self.ashmem_status,
                    _ => 0,
                };
            }
        }

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
            // Android-specific device writes
            if Self::is_android_periph(addr) {
                // Goldfish GPU - enable/disable framebuffer
                if addr >= GOLDFISH_FB_BASE && addr < GOLDFISH_FB_BASE + 0x100 {
                    match addr - GOLDFISH_FB_BASE {
                        0x0C => self.goldfish_fb_config.enabled = val != 0,
                        0x14 => self.goldfish_fb_config.width = val,
                        0x18 => self.goldfish_fb_config.height = val,
                        _ => {}
                    }
                }
                // SD/MMC - full command interface
                if addr >= MMC_BASE && addr < MMC_BASE + MMC_SIZE {
                    let reg_offset = addr - MMC_BASE;
                    match reg_offset {
                        MMC_REG_CMD => {
                            self.mmc_cmd = val;
                            self.execute_mmc_command();
                        }
                        MMC_REG_ARG => self.mmc_arg = val,
                        MMC_REG_CTRL => { /* Control register - no operation */ }
                        _ => {}
                    }
                    return;
                }
                // Goldfish Pipe
                if addr >= GOLDFISH_PIPE_BASE && addr < GOLDFISH_PIPE_BASE + GOLDFISH_PIPE_SIZE {
                    let off = addr - GOLDFISH_PIPE_BASE;
                    match off {
                        PIPE_REG_COMMAND => {
                            self.pipe_cmd = val;
                            self.execute_pipe_command();
                        }
                        PIPE_REG_CHANNEL => self.pipe_channel = val,
                        PIPE_REG_SIZE => self.pipe_size = val,
                        PIPE_REG_ADDRESS => self.pipe_address = val,
                        PIPE_REG_WAKES => self.pipe_wakes = val,
                        _ => {}
                    }
                    return;
                }
                // Goldfish RTC - update time
                if addr >= GOLDFISH_RTC_BASE && addr < GOLDFISH_RTC_BASE + 0x1000 {
                    match addr - GOLDFISH_RTC_BASE {
                        0x00 => self.goldfish_rtc = (self.goldfish_rtc & 0xFFFFFFFF_00000000) | (val as u64),
                        0x04 => self.goldfish_rtc = (self.goldfish_rtc & 0x00000000_FFFFFFFF) | ((val as u64) << 32),
                        _ => {}
                    }
                    return;
                }
                // Android Logger (logcat) - write log entry
                if addr >= ANDROID_LOG_BASE && addr < ANDROID_LOG_BASE + ANDROID_LOG_SIZE {
                    let reg_offset = addr - ANDROID_LOG_BASE;
                    match reg_offset {
                        LOG_REG_PRIO => self.log_prio = val,
                        LOG_REG_TAG => self.log_tag_ptr = val,
                        LOG_REG_MSG => self.log_msg_ptr = val,
                        LOG_REG_CTRL => {
                            self.log_ctrl = val;
                            // Trigger log write on bit 0 set
                            if val & 0x1 != 0 {
                                self.execute_log_write();
                                // Clear trigger bit after execution
                                self.log_ctrl &= !0x1;
                            }
                        }
                        _ => {}
                    }
                }
                // ASHMEM (Anonymous Shared Memory) - create/manage shared regions
                if addr >= ASHMEM_BASE && addr < ASHMEM_BASE + ASHMEM_SIZE {
                    let reg_offset = addr - ASHMEM_BASE;
                    match reg_offset {
                        ASHMEM_REG_CMD => {
                            self.ashmem_cmd = val;
                            self.execute_ashmem_command();
                        }
                        ASHMEM_REG_SIZE => self.ashmem_size = val,
                        ASHMEM_REG_PROT => self.ashmem_prot = val,
                        ASHMEM_REG_PINNED => self.ashmem_pinned = val,
                        ASHMEM_REG_DATA_PTR => self.ashmem_data_ptr = val,
                        _ => {}
                    }
                }
                // Binder IPC - increment transaction counter
                if addr >= BINDER_BASE && addr < BINDER_BASE + BINDER_SIZE {
                    self.binder_seq = self.binder_seq.wrapping_add(1);
                }
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
