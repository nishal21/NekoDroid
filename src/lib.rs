use wasm_bindgen::prelude::*;
use std::sync::atomic::{AtomicU32, Ordering};

// ── CPU emulator modules ──────────────────────────────────────────────
pub mod memory;
pub mod cpu;
pub mod cp15;
pub mod android;

// ── Browser bindings ──────────────────────────────────────────────────
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

// ── Constants ─────────────────────────────────────────────────────────
const SCREEN_WIDTH: usize = 800;
const SCREEN_HEIGHT: usize = 600;
const FRAMEBUFFER_SIZE: usize = SCREEN_WIDTH * SCREEN_HEIGHT * 4; // RGBA

// ── Global state ──────────────────────────────────────────────────────
static CYCLE_COUNT: AtomicU32 = AtomicU32::new(0);

/// The VirtualCPU holds a framebuffer representing screen pixels (RGBA).
#[wasm_bindgen]
pub struct VirtualCPU {
    framebuffer: Vec<u8>,
    width: u32,
    height: u32,
    seed: u32,  // Simple PRNG state for random colors
}

#[wasm_bindgen]
impl VirtualCPU {
    /// Create a new VirtualCPU with an 800×600 RGBA framebuffer.
    #[wasm_bindgen(constructor)]
    pub fn new() -> VirtualCPU {
        let mut fb = vec![0u8; FRAMEBUFFER_SIZE];
        // Initialize to black with full alpha
        for pixel in fb.chunks_exact_mut(4) {
            pixel[0] = 0;   // R
            pixel[1] = 0;   // G
            pixel[2] = 0;   // B
            pixel[3] = 255; // A
        }
        log("🖥️ VirtualCPU created: 800×600 framebuffer allocated");
        VirtualCPU {
            framebuffer: fb,
            width: SCREEN_WIDTH as u32,
            height: SCREEN_HEIGHT as u32,
            seed: 42,
        }
    }

    /// Returns a pointer to the framebuffer for direct JS access.
    #[wasm_bindgen]
    pub fn framebuffer_ptr(&self) -> *const u8 {
        self.framebuffer.as_ptr()
    }

    /// Returns the framebuffer length in bytes.
    #[wasm_bindgen]
    pub fn framebuffer_len(&self) -> usize {
        self.framebuffer.len()
    }

    /// Returns screen width.
    #[wasm_bindgen]
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Returns screen height.
    #[wasm_bindgen]
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Simple xorshift32 PRNG — fast, no dependencies.
    fn next_random(&mut self) -> u32 {
        let mut x = self.seed;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.seed = x;
        x
    }

    /// Fills the framebuffer with random colored noise to simulate screen activity.
    /// Each call represents one "frame" of the virtual display.
    #[wasm_bindgen]
    pub fn render_noise(&mut self) {
        let mut seed = self.seed;
        for pixel in self.framebuffer.chunks_exact_mut(4) {
            // Inline xorshift32
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            pixel[0] = (seed & 0xFF) as u8;         // R
            pixel[1] = ((seed >> 8) & 0xFF) as u8;  // G
            pixel[2] = ((seed >> 16) & 0xFF) as u8; // B
            pixel[3] = 255;                          // A
        }
        self.seed = seed;
    }

    /// Fills the framebuffer with a colored gradient pattern.
    /// More visually interesting than pure noise.
    #[wasm_bindgen]
    pub fn render_gradient(&mut self, frame: u32) {
        for y in 0..self.height {
            for x in 0..self.width {
                let idx = ((y * self.width + x) * 4) as usize;
                let r = ((x.wrapping_add(frame)) % 256) as u8;
                let g = ((y.wrapping_add(frame.wrapping_mul(2))) % 256) as u8;
                let b = ((x.wrapping_add(y).wrapping_add(frame.wrapping_mul(3))) % 256) as u8;
                self.framebuffer[idx] = r;
                self.framebuffer[idx + 1] = g;
                self.framebuffer[idx + 2] = b;
                self.framebuffer[idx + 3] = 255;
            }
        }
    }

    /// Renders a plasma effect — classic demoscene visual test.
    #[wasm_bindgen]
    pub fn render_plasma(&mut self, time: f64) {
        let w = self.width as f64;
        let h = self.height as f64;
        for y in 0..self.height {
            for x in 0..self.width {
                let idx = ((y * self.width + x) * 4) as usize;
                let fx = x as f64 / w;
                let fy = y as f64 / h;

                let v1 = ((fx * 10.0 + time).sin() + 1.0) * 0.5;
                let v2 = (((fy * 10.0 + time * 1.5).sin() + (fx * 10.0).cos()) * 0.5 + 0.5).min(1.0).max(0.0);
                let v3 = ((((fx - 0.5) * (fx - 0.5) + (fy - 0.5) * (fy - 0.5)).sqrt() * 10.0 - time * 2.0).sin() + 1.0) * 0.5;

                let r = ((v1 * 255.0) as u32).min(255) as u8;
                let g = ((v2 * 255.0) as u32).min(255) as u8;
                let b = ((v3 * 255.0) as u32).min(255) as u8;

                self.framebuffer[idx] = r;
                self.framebuffer[idx + 1] = g;
                self.framebuffer[idx + 2] = b;
                self.framebuffer[idx + 3] = 255;
            }
        }
    }
}

// ── Standalone functions ──────────────────────────────────────────────

/// Exports the Wasm linear memory so JS can read the framebuffer directly.
#[wasm_bindgen]
pub fn wasm_memory() -> JsValue {
    wasm_bindgen::memory()
}

/// Returns a pointer to the CPU's VRAM buffer (800×600 RGBA).
/// JS can use this with `wasm_memory()` to read pixel data directly.
#[wasm_bindgen]
pub fn get_vram_ptr() -> u32 {
    ARM_CPU.with(|cell| {
        let borrow = cell.borrow();
        match borrow.as_ref() {
            Some(cpu) => cpu.mmu.vram_ptr() as u32,
            None => 0,
        }
    })
}

/// Returns the VRAM buffer length in bytes (1,920,000 for 800×600 RGBA).
#[wasm_bindgen]
pub fn get_vram_len() -> u32 {
    ARM_CPU.with(|cell| {
        let borrow = cell.borrow();
        match borrow.as_ref() {
            Some(cpu) => cpu.mmu.vram_len() as u32,
            None => 0,
        }
    })
}

// ── Persistent ARM CPU ────────────────────────────────────────────────
// Wasm is single-threaded, so thread_local + RefCell is safe.

use std::cell::RefCell;

thread_local! {
    static ARM_CPU: RefCell<Option<cpu::Cpu>> = RefCell::new(None);
    static APK_RT: RefCell<Option<android::AppRuntime>> = RefCell::new(None);
}

const MAX_UPLOAD_BYTES: usize = 32 * 1024 * 1024; // 32 MiB APK/ROM soft cap


/// Initializes the emulator with configurable RAM.
/// `ram_mb` is megabytes. Pass 0 for default 512. Soft-caps at 1024 in browser-friendly mode
/// unless the caller explicitly requests more (max 4096).
#[wasm_bindgen]
pub fn init_emulator(ram_mb: u32) {
    log("nekodroid: Wasm CPU emulator initialized");

    let ram_mb = if ram_mb == 0 {
        512
    } else {
        ram_mb.min(4096)
    };
    if ram_mb > 1024 {
        log(&format!(
            "warning: allocating {ram_mb} MB RAM; large tabs may OOM in some browsers"
        ));
    }
    let ram_bytes = ram_mb as usize * 1024 * 1024;
    let mut arm_cpu = cpu::Cpu::new(ram_bytes);
    arm_cpu.regs.set_pc(0x0000_8000);
    arm_cpu.regs.set_sp((ram_bytes as u32).wrapping_sub(0x1_0000));

    log(&format!(
        "ARMv7 CPU ready — PC: {:#010X}, SP: {:#010X}, RAM: {} MB",
        arm_cpu.regs.pc(),
        arm_cpu.regs.sp(),
        ram_bytes / (1024 * 1024)
    ));

    ARM_CPU.with(|cell| {
        *cell.borrow_mut() = Some(arm_cpu);
    });
}

/// Returns the CPU state as a JSON string for the debug panel.
/// Includes registers, flags, and disassembly of next 5 instructions.
#[wasm_bindgen]
pub fn get_cpu_state() -> String {
    ARM_CPU.with(|cell| {
        let mut borrow = cell.borrow_mut();
        match borrow.as_mut() {
            Some(cpu) => {
                let regs: Vec<String> = (0..16)
                    .map(|i| cpu.regs.read(i).to_string())
                    .collect();

                // Disassemble the next 5 instructions from PC
                let pc = cpu.regs.pc();
                let disasm: Vec<String> = (0..5)
                    .map(|i| {
                        let addr = pc.wrapping_add(i * 4);
                        let asm = cpu.disassemble_at(addr);
                        // Escape quotes for JSON
                        let escaped = asm.replace('"', "\\\"")
                            .replace('\\', "\\\\");
                        format!("\"0x{:08X}: {}\"", addr, escaped)
                    })
                    .collect();

                format!(
                    r#"{{"regs":[{}],"cpsr":{},"n":{},"z":{},"c":{},"v":{},"t":{},"cycles":{},"halted":{},"disasm":[{}]}}"#,
                    regs.join(","),
                    cpu.regs.cpsr(),
                    cpu.regs.flag_n(),
                    cpu.regs.flag_z(),
                    cpu.regs.flag_c(),
                    cpu.regs.flag_v(),
                    cpu.regs.is_thumb(),
                    CYCLE_COUNT.load(Ordering::Relaxed),
                    cpu.halted,
                    disasm.join(","),
                )
            }
            None => r#"{"error":"CPU not initialized"}"#.to_string(),
        }
    })
}

/// Steps the CPU by one instruction. Returns true if it executed.
#[wasm_bindgen]
pub fn step_cpu() -> bool {
    ARM_CPU.with(|cell| {
        let mut borrow = cell.borrow_mut();
        match borrow.as_mut() {
            Some(cpu) => {
                let ran = cpu.step();
                if ran {
                    CYCLE_COUNT.fetch_add(1, Ordering::Relaxed);
                }
                ran
            }
            None => false,
        }
    })
}

/// Runs the CPU for a specified number of instructions inside Wasm to avoid JS boundary overhead.
/// Ticks the system timer every `timer_interval` instructions.
/// Returns the actual number of instructions executed (will be less than `count` if halted).
#[wasm_bindgen]
pub fn run_batch(count: u32, timer_interval: u32) -> u32 {
    // Guard against divide-by-zero from host calls.
    let safe_interval = if timer_interval == 0 { 200_000 } else { timer_interval };

    ARM_CPU.with(|cell| {
        let mut borrow = cell.borrow_mut();
        if let Some(cpu) = borrow.as_mut() {
            let mut executed = 0;
            for i in 1..=count {
                if !cpu.step() {
                    break; // CPU halted
                }
                executed += 1;

                // Tick the internal hardware timer
                if i % safe_interval == 0 {
                    cpu.mmu.sys_timer = cpu.mmu.sys_timer.wrapping_add(1);
                }
            }
            CYCLE_COUNT.fetch_add(executed, Ordering::Relaxed);
            executed
        } else {
            0
        }
    })
}

/// Loads a demo ARM program for debugging.
/// This loads: MOV R0,#5 → MOV R1,#10 → ADD R2,R0,R1 → SUB R3,R2,#1 → CMP R3,#14 → loop back
#[wasm_bindgen]
pub fn load_demo_program() {
    ARM_CPU.with(|cell| {
        let mut borrow = cell.borrow_mut();
        if let Some(cpu) = borrow.as_mut() {
            let program: Vec<u8> = [
                0xE3A00005u32.to_le_bytes(), // 0x8000: MOV R0, #5
                0xE3A0100Au32.to_le_bytes(), // 0x8004: MOV R1, #10
                0xE0802001u32.to_le_bytes(), // 0x8008: ADD R2, R0, R1
                0xE2423001u32.to_le_bytes(), // 0x800C: SUB R3, R2, #1
                0xE353000Eu32.to_le_bytes(), // 0x8010: CMP R3, #14
                0x0A000000u32.to_le_bytes(), // 0x8014: BEQ +8 (skip next if equal)
                0xE3A04001u32.to_le_bytes(), // 0x8018: MOV R4, #1  (not equal path)
                0xEA000000u32.to_le_bytes(), // 0x801C: B +8 (skip to end)
                0xE3A04000u32.to_le_bytes(), // 0x8020: MOV R4, #0  (equal path)
                0xE1A00000u32.to_le_bytes(), // 0x8024: NOP (MOV R0, R0)
            ].concat();

            cpu.load_program(0x8000, &program);
            log("📦 Demo program loaded at 0x8000 (10 ARM instructions)");
            log("   MOV R0,#5 → MOV R1,#10 → ADD R2,R0,R1 → SUB R3,R2,#1 → CMP/BEQ logic");
        }
    });
}

/// Returns the current cycle count.
#[wasm_bindgen]
pub fn get_cycle_count() -> u32 {
    CYCLE_COUNT.load(Ordering::Relaxed)
}

// ── Input event handlers ──────────────────────────────────────────────

/// Receives a touch/mouse event from the browser.
/// `x` and `y` are canvas-relative pixel coordinates.
/// `is_down` is true for press/move-while-pressed, false for release.
/// Writes directly to the CPU's MMIO input registers.
#[wasm_bindgen]
pub fn send_touch_event(x: i32, y: i32, is_down: bool) {
    ARM_CPU.with(|cell| {
        if let Some(cpu) = cell.borrow_mut().as_mut() {
            cpu.mmu.touch_down = is_down;
            // Only update coordinates if it's a valid touch inside the canvas
            if x >= 0 && y >= 0 {
                cpu.mmu.touch_x = x as u16;
                cpu.mmu.touch_y = y as u16;
            }
        }
    });
}

/// Receives a keyboard event from the browser.
/// `keycode` is the DOM KeyboardEvent.keyCode value.
/// `is_down` is true for keydown, false for keyup.
/// Writes directly to the CPU's MMIO key register.
#[wasm_bindgen]
pub fn send_key_event(keycode: i32, is_down: bool) {
    ARM_CPU.with(|cell| {
        if let Some(cpu) = cell.borrow_mut().as_mut() {
            cpu.mmu.key_state = if is_down { keycode as u32 } else { 0 };
        }
    });
}

/// Returns the current AUDIO_CTRL register value.
/// Bit 0 = enable, Bits 1-2 = waveform (0=Square, 1=Sine, 2=Sawtooth, 3=Triangle).
#[wasm_bindgen]
pub fn get_audio_ctrl() -> u32 {
    ARM_CPU.with(|cell| {
        cell.borrow().as_ref().map_or(0, |cpu| cpu.mmu.audio_ctrl)
    })
}

/// Returns the current AUDIO_FREQ register value (frequency in Hz).
#[wasm_bindgen]
pub fn get_audio_freq() -> u32 {
    ARM_CPU.with(|cell| {
        cell.borrow().as_ref().map_or(0, |cpu| cpu.mmu.audio_freq)
    })
}

/// Parses a hex string (e.g. "e3a00005 e3a0100a") and loads it as ARM machine code
/// at address 0x8000. Supports space/newline separation or continuous hex.
/// Resets the PC to 0x8000 and cycle count to 0.
#[wasm_bindgen]
pub fn load_custom_hex(hex_string: &str) -> bool {
    ARM_CPU.with(|cell| {
        let mut borrow = cell.borrow_mut();
        if let Some(cpu) = borrow.as_mut() {
            // Strip whitespace and parse hex
            let clean: String = hex_string
                .chars()
                .filter(|c| c.is_ascii_hexdigit())
                .collect();

            if clean.len() % 8 != 0 {
                log(&format!("❌ Invalid hex: {} chars (must be multiple of 8)", clean.len()));
                return false;
            }

            let mut bytes = Vec::new();
            for chunk in clean.as_bytes().chunks(8) {
                let hex_str = std::str::from_utf8(chunk).unwrap_or("");
                match u32::from_str_radix(hex_str, 16) {
                    Ok(word) => bytes.extend_from_slice(&word.to_le_bytes()),
                    Err(_) => {
                        log(&format!("❌ Invalid hex word: {}", hex_str));
                        return false;
                    }
                }
            }

            let instr_count = bytes.len() / 4;
            cpu.load_program(0x8000, &bytes);
            CYCLE_COUNT.store(0, Ordering::Relaxed);
            log(&format!("📦 Custom program loaded at 0x8000 ({} instructions)", instr_count));
            true
        } else {
            false
        }
    })
}

/// Loads a raw binary payload (e.g., compiled C code) into RAM at 0x8000.
#[wasm_bindgen]
pub fn load_rom(bytes: &[u8]) -> bool {
    ARM_CPU.with(|cell| {
        let mut borrow = cell.borrow_mut();
        if let Some(cpu) = borrow.as_mut() {
            // Reset CPU state
            cpu.reset();
            // Load the binary at the standard boot address
            cpu.load_program(0x8000, bytes);
            // Reset cycle count
            CYCLE_COUNT.store(0, std::sync::atomic::Ordering::Relaxed);
            log(&format!("💿 ROM loaded successfully: {} bytes at 0x8000", bytes.len()));
            true
        } else {
            false
        }
    })
}

/// Loads and boots a Linux kernel zImage/Image.
#[wasm_bindgen]
pub fn boot_linux_kernel(kernel_bytes: &[u8], initrd_bytes: &[u8]) -> bool {
    ARM_CPU.with(|cell| {
        let mut borrow = cell.borrow_mut();
        if let Some(cpu) = borrow.as_mut() {
            let initrd = if initrd_bytes.is_empty() { None } else { Some(initrd_bytes) };
            cpu.boot_linux(kernel_bytes, initrd, 0x00E2);
            CYCLE_COUNT.store(0, Ordering::Relaxed);
            true
        } else {
            false
        }
    })
}

/// Returns the current CP15 MIDR value from the emulated CPU.
#[wasm_bindgen]
pub fn get_cp15_midr() -> u32 {
    ARM_CPU.with(|cell| {
        let borrow = cell.borrow();
        match borrow.as_ref() {
            Some(cpu) => cpu.cp15.c0_midr,
            None => 0,
        }
    })
}

// ── Android APK & System Image Support ────────────────────────────────

/// Mounts an Android system image (ext4 partition) for the emulator.
/// This makes the Android framework available to loaded APKs.
#[wasm_bindgen]
pub fn mount_system_image(image_bytes: &[u8]) -> bool {
    ARM_CPU.with(|cell| {
        let mut borrow = cell.borrow_mut();
        if let Some(cpu) = borrow.as_mut() {
            cpu.mmu.mmc_card_data = Some(image_bytes.to_vec());
            log(&format!("🤖 Android system image mounted: {} bytes", image_bytes.len()));
            true
        } else {
            log("❌ Cannot mount system image: CPU not initialized");
            false
        }
    })
}

/// Loads an APK into the HLE Dalvik runtime (does not boot a full Android OS).
#[wasm_bindgen]
pub fn load_apk(apk_bytes: &[u8]) -> bool {
    if apk_bytes.len() > MAX_UPLOAD_BYTES {
        log(&format!(
            "APK rejected: {} bytes exceeds {} byte limit",
            apk_bytes.len(),
            MAX_UPLOAD_BYTES
        ));
        return false;
    }
    match android::AppRuntime::load(apk_bytes) {
        Ok(rt) => {
            log(&format!(
                "APK loaded: {} bytes, package={}, classes={}",
                apk_bytes.len(),
                rt.apk.package_name,
                rt.apk.dex.classes.len()
            ));
            APK_RT.with(|cell| *cell.borrow_mut() = Some(rt));
            true
        }
        Err(e) => {
            log(&format!("APK load failed: {e}"));
            false
        }
    }
}

/// JSON info for the currently loaded APK/HLE session.
#[wasm_bindgen]
pub fn get_apk_info() -> String {
    APK_RT.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|rt| rt.info_json())
            .unwrap_or_else(|| r#"{"error":"no apk loaded"}"#.into())
    })
}

/// Launch the loaded APK entry (main or onCreate) on the HLE VM.
#[wasm_bindgen]
pub fn launch_apk() -> bool {
    APK_RT.with(|cell| {
        let mut borrow = cell.borrow_mut();
        match borrow.as_mut() {
            Some(rt) => match rt.launch() {
                Ok(()) => {
                    log("APK launched on HLE Dalvik VM");
                    true
                }
                Err(e) => {
                    log(&format!("APK launch failed: {e}"));
                    false
                }
            },
            None => {
                log("launch_apk: no APK loaded");
                false
            }
        }
    })
}

/// Step the HLE Dalvik VM once. Returns false if halted or no APK.
#[wasm_bindgen]
pub fn step_dalvik() -> bool {
    APK_RT.with(|cell| {
        let mut borrow = cell.borrow_mut();
        match borrow.as_mut() {
            Some(rt) => rt.step().unwrap_or(false),
            None => false,
        }
    })
}

/// Run up to `count` Dalvik VM steps. Returns steps executed.
#[wasm_bindgen]
pub fn run_dalvik_batch(count: u32) -> u32 {
    APK_RT.with(|cell| {
        let mut borrow = cell.borrow_mut();
        match borrow.as_mut() {
            Some(rt) => rt.run_batch(count).unwrap_or(0),
            None => 0,
        }
    })
}

/// HLE logcat-style lines from the Dalvik session (plus canvas text).
#[wasm_bindgen]
pub fn get_dalvik_logs() -> String {
    APK_RT.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|rt| rt.logs_text())
            .unwrap_or_default()
    })
}

/// Draw HLE canvas_text into the CPU VRAM as a simple bitmap message.
#[wasm_bindgen]
pub fn blit_dalvik_text_to_vram() -> bool {
    APK_RT.with(|apk| {
        let text = apk
            .borrow()
            .as_ref()
            .map(|rt| rt.canvas_text())
            .unwrap_or_default();
        if text.is_empty() {
            return false;
        }
        ARM_CPU.with(|cell| {
            let mut borrow = cell.borrow_mut();
            let Some(cpu) = borrow.as_mut() else {
                return false;
            };
            draw_text_vram(&mut cpu.mmu, &text);
            true
        })
    })
}

fn draw_text_vram(mmu: &mut memory::Mmu, text: &str) {
    // Fill dark background
    let w = memory::VRAM_WIDTH;
    let h = memory::VRAM_HEIGHT;
    for y in 0..h {
        for x in 0..w {
            let off = (y * w + x) * 4;
            if off + 3 < mmu.vram_len() {
                // direct via write path
                let addr = memory::VRAM_BASE + off as u32;
                mmu.write_u8(addr, 12);
                mmu.write_u8(addr + 1, 14);
                mmu.write_u8(addr + 2, 22);
                mmu.write_u8(addr + 3, 255);
            }
        }
    }
    // Simple 8x8 block font for ASCII: draw each char as a filled cell pattern
    let mut cx = 16usize;
    let mut cy = 24usize;
    for ch in text.chars() {
        if ch == '\n' {
            cy += 16;
            cx = 16;
            continue;
        }
        if cx + 8 >= w {
            cy += 16;
            cx = 16;
        }
        if cy + 8 >= h {
            break;
        }
        let glyph = ch as u8;
        for gy in 0..8 {
            for gx in 0..8 {
                let on = ((glyph.wrapping_mul(31).wrapping_add(gx as u8 * 3).wrapping_add(gy as u8))
                    % 7)
                    < 3
                    || (gx == 0 || gy == 0 || gx == 7 || gy == 7);
                if on {
                    let x = cx + gx;
                    let y = cy + gy;
                    let addr = memory::VRAM_BASE + ((y * w + x) * 4) as u32;
                    mmu.write_u8(addr, 240);
                    mmu.write_u8(addr + 1, 240);
                    mmu.write_u8(addr + 2, 245);
                    mmu.write_u8(addr + 3, 255);
                }
            }
        }
        // Also punch readable ASCII as a second row of brighter pixels keyed by char code
        let bar = (glyph as usize % 40) + 1;
        for i in 0..bar.min(8) {
            let addr = memory::VRAM_BASE + (((cy + 10) * w + cx + i) * 4) as u32;
            mmu.write_u8(addr, 80);
            mmu.write_u8(addr + 1, 200);
            mmu.write_u8(addr + 2, 120);
            mmu.write_u8(addr + 3, 255);
        }
        cx += 10;
    }
}


/// Boots an Android kernel with system image and optional APK.
/// This is the main entry point for running Android apps.
/// 
/// # Arguments
/// * `kernel_bytes` - The Android kernel zImage (e.g., goldfish_defconfig)
/// * `system_img` - The Android system image (ext4 format)
/// * `ramdisk_img` - Optional initramfs (can be empty)
/// * `apk_bytes` - Optional APK to auto-launch after boot (can be empty)
#[wasm_bindgen]
pub fn boot_android(kernel_bytes: &[u8], system_img: &[u8], ramdisk_img: &[u8], apk_bytes: &[u8]) -> bool {
    ARM_CPU.with(|cell| {
        let mut borrow = cell.borrow_mut();
        if let Some(cpu) = borrow.as_mut() {
            // Mount system image first
            if !system_img.is_empty() {
                cpu.mmu.mmc_card_data = Some(system_img.to_vec());
                log(&format!("🤖 Android system mounted: {} MB", system_img.len() / (1024*1024)));
            }
            
            // Set up Android-specific boot parameters
            let initrd = if ramdisk_img.is_empty() { None } else { Some(ramdisk_img) };
            
            // Boot with Android machine ID (goldfish = 0x46F, versatile = 0x183)
            // Android emulator uses goldfish machine ID
            cpu.boot_linux(kernel_bytes, initrd, 0x046F);
            
            // Set Android-specific boot args registers
            // R1 = machine type (already set by boot_linux)
            // R2 = atags pointer (already set)
            
            CYCLE_COUNT.store(0, Ordering::Relaxed);
            
            if !apk_bytes.is_empty() {
                log(&format!("📦 APK queued for launch: {} bytes", apk_bytes.len()));
                // TODO: Auto-launch APK after system boot completes
                // This requires detecting when init finishes and launching the app
            }
            
            log("🚀 Android boot initiated (Goldfish machine: 0x46F)");
            true
        } else {
            log("❌ Cannot boot Android: CPU not initialized");
            false
        }
    })
}

/// Returns the Android logger buffer as a formatted string for display.
/// Useful for seeing logcat output from the emulated Android system.
#[wasm_bindgen]
pub fn get_android_logs() -> String {
    ARM_CPU.with(|cell| {
        let borrow = cell.borrow();
        match borrow.as_ref() {
            Some(cpu) => {
                let mut output = String::new();
                for entry in &cpu.mmu.logger_buffer {
                    let level = match entry.priority {
                        2 => "V",
                        3 => "D",
                        4 => "I",
                        5 => "W",
                        6 => "E",
                        _ => "?",
                    };
                    output.push_str(&format!("{} [{}] {}\n", level, entry.tag, entry.message));
                }
                output
            }
            None => "Android logger not available (CPU not initialized)".to_string(),
        }
    })
}

/// Forward host touch into the HLE APK runtime (MotionEvent stubs).
#[wasm_bindgen]
pub fn dalvik_touch(x: i32, y: i32, is_down: bool) {
    APK_RT.with(|cell| {
        if let Some(rt) = cell.borrow_mut().as_mut() {
            rt.vm.host.touch_x = x;
            rt.vm.host.touch_y = y;
            rt.vm.host.touch_down = is_down;
        }
    });
}

/// Clears the Android logger buffer.
#[wasm_bindgen]
pub fn clear_android_logs() {
    ARM_CPU.with(|cell| {
        if let Some(cpu) = cell.borrow_mut().as_mut() {
            cpu.mmu.logger_buffer.clear();
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_boot_linux_kernel_wrapper_uses_machine_id_00e2() {
        ARM_CPU.with(|cell| {
            *cell.borrow_mut() = Some(cpu::Cpu::new(16 * 1024 * 1024));
        });

        let kernel = [0x00, 0x00, 0xA0, 0xE3]; // MOV R0, #0
        assert!(boot_linux_kernel(&kernel, &[]));

        ARM_CPU.with(|cell| {
            let borrow = cell.borrow();
            let cpu = borrow.as_ref().expect("CPU should be initialized");
            assert_eq!(cpu.regs.read(1), 0x00E2);
        });
    }
}
