use wasm_bindgen::prelude::*;
use std::sync::atomic::{AtomicU32, Ordering};

// ── CPU emulator modules ──────────────────────────────────────────────
pub mod memory;
pub mod cpu;

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

/// Initializes the emulator with configurable RAM.
/// `ram_mb` is the RAM size in megabytes (e.g. 512, 1024, 2048).
/// Pass 0 for the default (128 MB).
#[wasm_bindgen]
pub fn init_emulator(ram_mb: u32) {
    log("🐱 nekodroid: Wasm CPU Emulator Initialized!");

    let ram_bytes = if ram_mb == 0 { 128 } else { ram_mb as usize } * 1024 * 1024;
    let mut arm_cpu = cpu::Cpu::new(ram_bytes);
    arm_cpu.regs.set_pc(0x0000_8000);
    arm_cpu.regs.set_sp((ram_bytes as u32).wrapping_sub(0x1_0000)); // SP near top of RAM
    log(&format!(
        "🔧 ARMv7 CPU ready — PC: {:#010X}, SP: {:#010X}, RAM: {} MB",
        arm_cpu.regs.pc(),
        arm_cpu.regs.sp(),
        ram_bytes / (1024 * 1024)
    ));
}

/// Executes one CPU cycle, returns the new count.
#[wasm_bindgen]
pub fn execute_cycle() -> u32 {
    let count = CYCLE_COUNT.fetch_add(1, Ordering::Relaxed) + 1;
    count
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
#[wasm_bindgen]
pub fn send_touch_event(x: i32, y: i32, is_down: bool) {
    let action = if is_down { "DOWN" } else { "UP" };
    log(&format!("👆 Touch {}: ({}, {})", action, x, y));
}

/// Receives a keyboard event from the browser.
/// `keycode` is the DOM KeyboardEvent.keyCode value.
#[wasm_bindgen]
pub fn send_key_event(keycode: i32) {
    log(&format!("⌨️ Key pressed: keycode={}", keycode));
}
