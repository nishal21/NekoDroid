# nekodroid — Development Log

> A record of every development session: what was built, what was discovered, and what's next.

---

## Session 1: Project Initialization
**Date:** 2026-03-03  
**Role:** Principal Systems Engineer

### What We Built
- Vite + vanilla-ts project scaffold
- Rust library crate (`cargo init --lib`)
- `Cargo.toml` configured for Wasm: `cdylib` + `wasm-bindgen = "0.2"`
- `vite.config.ts` with `vite-plugin-wasm` + `vite-plugin-top-level-await`
- Comprehensive `README.md` (Nesting Doll architecture, 6-phase roadmap, contributor guide)

### Discoveries
- **Vite 8 is in beta** — stayed on stable Vite 7.3.1

---

## Session 2: Wasm Bridge Proof-of-Concept
**Date:** 2026-03-03  
**Role:** WebAssembly Build Engineer

### What We Built
- `init_emulator()` — logs to browser console from Rust/Wasm
- `execute_cycle()` — returns incrementing cycle counter
- Installed `wasm-pack`, compiled with `wasm-pack build --target web`
- TypeScript frontend importing Wasm module, wiring execute/burst/reset buttons
- Verified: single cycle, burst (100 cycles in ~151ms)

### Discoveries
- **Rust 2024 edition denies `static mut` references.** The `#[deny(static_mut_refs)]` lint blocks the common `static mut` pattern. Fix: use `std::sync::atomic::AtomicU32` with `Ordering::Relaxed`.
- **`wasm-pack` first install compiles 256 crates** (~8 min). Subsequent builds are fast (~1–2s).
- **`pkg/` output:** `nekodroid.js` (5.2 KB) + `nekodroid_bg.wasm` (16 KB)

---

## Session 3: Framebuffer & Canvas Rendering
**Date:** 2026-03-03  
**Role:** Graphics and Systems Programmer

### What We Built
- `VirtualCPU` struct with 800×600 RGBA framebuffer (1,920,000 bytes)
- Three render modes: `render_noise()` (xorshift PRNG), `render_gradient()`, `render_plasma()` (demoscene-style)
- Raw framebuffer pointer exported to JS via `framebuffer_ptr()`
- `wasm_memory()` function exporting Wasm linear memory to TypeScript
- `<canvas id="screen" width="800" height="600">` in `index.html`
- `requestAnimationFrame` render loop reading Wasm memory → `ImageData` → canvas
- Dark cyberpunk UI with FPS counter, frame/cycle metrics, mode switching, pause/resume

### Performance
- Noise mode: ~21 FPS (full-screen PRNG per pixel)
- Gradient mode: ~46 FPS (arithmetic per pixel)
- Plasma mode: ~5–15 FPS (trig functions per pixel)

### Discoveries
- **Borrow checker vs iteration + method calls.** Cannot call `self.next_random()` while iterating `self.framebuffer.chunks_exact_mut(4)` — both borrow `self` mutably. Fix: inline the xorshift PRNG using a local `seed` variable.
- **Vite 7 cannot resolve direct `.wasm` imports.** `import { memory } from '../pkg/nekodroid_bg.wasm'` fails because Vite's import analysis tries to resolve `./nekodroid_bg.js` from inside the wasm file. Fix: export `wasm_memory()` from Rust via `wasm_bindgen::memory()`, call it from TypeScript after `init()`.
- **CSS `@import` must precede all other rules.** Google Fonts `@import` placed after `:root` triggers a PostCSS error.

### Committed
- **Commit:** `ff3a374` — `feat: initial project scaffold with Wasm framebuffer rendering`
- **Pushed to:** [github.com/nishal21/NekoDroid](https://github.com/nishal21/NekoDroid)

---

## Session 4: Input Event Pipeline
**Date:** 2026-03-03  
**Role:** Frontend Interaction Engineer

### What We Built
- `send_touch_event(x, y, is_down)` in Rust — receives touch/mouse events, logs action + coordinates
- `send_key_event(keycode)` in Rust — receives keyboard events, logs keycode
- Canvas event listeners in TypeScript: `mousedown`, `mousemove`, `mouseup`, `mouseleave`, `keydown`
- CSS → framebuffer coordinate translation using `getBoundingClientRect()` scale factors
- Canvas set to `tabindex="0"` for keyboard focus

### Verified
- Touch DOWN at (400, 299) ✅
- Touch UP at (400, 299) ✅  
- `mousemove` only fires while mouse is pressed (drag tracking)
- `mouseleave` sends cancel event (-1, -1)
- Key pressed: a (code=65) ✅

---

## Session 5: ARMv7 CPU Emulator Foundation
**Date:** 2026-03-03  
**Role:** Lead Systems Programmer / ARM Architecture Expert

### What We Built
- **`src/memory.rs`** — `Mmu` struct: flat 16 MB RAM, `read_u8/u16/u32`, `write_u8/u16/u32` (little-endian), `load_bytes` for binary images
- **`src/cpu.rs`** — `RegisterFile`: R0–R15 array + CPSR with N/Z/C/V/T flag accessors and `update_nz()` helper
- **`src/cpu.rs`** — `Cpu` struct: owns `RegisterFile` + `Mmu`, with `fetch()` (ARM/Thumb aware), `advance_pc()`, `load_program()`, `reset()`
- Wired modules into `lib.rs` via `pub mod cpu; pub mod memory;`
- `init_emulator()` now creates a `Cpu` instance and logs: `ARMv7 CPU ready — PC: 0x00008000, SP: 0x007F0000, RAM: 16 MB`

### Tests (all pass)
- `test_read_write_u8`, `test_read_write_u16_little_endian`, `test_read_write_u32_little_endian`
- `test_out_of_bounds_reads_zero`, `test_load_bytes`
- `test_register_read_write`, `test_sp_lr_pc`, `test_cpsr_flags`, `test_thumb_mode`, `test_update_nz`
- `test_cpu_fetch_arm`, `test_cpu_fetch_thumb`, `test_cpu_advance_pc`, `test_cpu_load_program`

---

## Session 6: ARM Instruction Execution Loop
**Date:** 2026-03-03  
**Role:** Systems Programmer / ARM Emulator Architect

### What We Built
- **`step(&mut self)`** — full fetch-decode-execute cycle: reads instruction at PC, advances PC by 4, checks condition code, decodes format, executes
- **Condition code evaluator** — all 15 ARM conditions (EQ, NE, CS, CC, MI, PL, VS, VC, HI, LS, GE, LT, GT, LE, AL) checked against CPSR N/Z/C/V flags
- **Data Processing decode** — bitmask decode of opcode bits [24:21], immediate vs register operand2 with rotation
- **ALU operations:** MOV, ADD, SUB, AND, EOR, ORR, CMP, BIC, MVN — with optional S flag for N/Z/C/V updates
- **Branch (B/BL)** — sign-extended 24-bit offset, left-shifted by 2, added to PC+8 (ARM pipeline adjustment). BL saves return address to LR.

### Tests (21 total, all pass in 0.01s)
- `test_basic_alu` — MOV R0, #5 → ADD R1, R0, #10 → R1 == 15 ✅
- `test_mov_register` — MOV R0, #42 → MOV R1, R0 → R1 == 42 ✅
- `test_sub_instruction` — MOV R0, #20 → SUB R1, R0, #5 → R1 == 15 ✅
- `test_cmp_sets_flags` — CMP R0, #5 → Z flag set ✅
- `test_branch_forward` — B skips one instruction ✅
- `test_branch_backward` — B loops back, R0 increments ✅
- `test_conditional_execution` — MOVEQ executes, MOVNE skipped ✅

### Key Design Decisions
- **ARM pipeline offset:** Branch target = `PC_at_fetch + 8 + (sign_extended_offset << 2)`. The +8 accounts for the 3-stage ARM pipeline where PC reads as current instruction + 8.
- **Unimplemented instructions:** In test builds, `panic!` to catch issues. In release/Wasm, silently skip to avoid crashing the browser.
- **Carry/Overflow flags:** Properly computed for ADD (carry out) and SUB/CMP (borrow).

---

## What's Next (Phase 1: Instruction Decoding)
- [ ] Load/Store (LDR, STR) execution
- [ ] Register shift operands (LSL, LSR, ASR, ROR)
- [ ] BL (Branch with Link) testing
- [ ] Load/Store Multiple (LDM, STM)
- [ ] Test with longer ARM programs

