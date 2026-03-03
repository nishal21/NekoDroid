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

## Session 7: CPU Debug Panel
**Date:** 2026-03-03  
**Role:** WebAssembly & Frontend UI Engineer

### What We Built
- **Persistent ARM CPU** — `thread_local! RefCell<Option<Cpu>>` keeps the CPU across Wasm calls
- **`get_cpu_state()`** — returns JSON with R0–R15, CPSR, N/Z/C/V/T flags, cycle count, halted state
- **`step_cpu()`** — single-step execution, returns true if instruction ran
- **`load_demo_program()`** — loads 10-instruction test program at 0x8000 (MOV/ADD/SUB/CMP/BEQ)
- **Debug panel UI** — register grid (4×4), CPSR flag pills, Step/Load Demo/Run 10 buttons
- **Live updates** at 5 Hz via `setInterval(updateDebugPanel, 200)`
- **Register flash** — changed values glow cyan for 300ms

### Verified
- Load Demo → PC = 0x00008000 ✅
- Step 1: R0 = 00000005 (MOV R0, #5) ✅
- Step 2: R1 = 0000000A (MOV R1, #10) ✅
- Step 3: R2 = 0000000F (ADD R2, R0, R1 = 15) ✅
- PC increments by 4 each step ✅
- No console errors ✅

---

## Session 8: Barrel Shifter & Load/Store Instructions
**Date:** 2026-03-03  
**Role:** Lead Systems Programmer / ARM Architecture Expert

### What We Built
- **Barrel Shifter** — `shift_operand(value, shift_type, shift_amount)`: LSL, LSR, ASR, ROR
- **`decode_register_operand()`** — extracts Rm, shift_type (bits [6:5]), shift_amount (bits [11:7]) and applies barrel shift
- **Integrated into Data Processing** — register operand2 path now uses barrel shift instead of raw Rm
- **`execute_single_data_transfer()`** — full LDR/STR decode with all control bits:
  - I (bit 25): immediate vs register offset
  - P (bit 24): pre-indexed vs post-indexed
  - U (bit 23): add vs subtract offset
  - B (bit 22): byte vs word transfer
  - W (bit 21): write-back to base register
  - L (bit 20): load vs store

### Tests (27 total, all pass)
- `test_shift_lsl` — MOV R0, R1, LSL #2: 3 << 2 = 12 ✅
- `test_shift_lsr` — MOV R0, R1, LSR #3: 32 >> 3 = 4 ✅
- `test_add_with_shift` — ADD R0, R1, R2, LSL #1: 10 + (3 << 1) = 16 ✅
- `test_basic_str_ldr` — STR/LDR round-trip at address 0x100 ✅
- `test_str_pre_indexed_writeback` — STR R0, [R1, #4]! writes and updates R1 ✅
- `test_ldrb_strb` — STRB/LDRB byte-level transfer ✅

---

## Session 9: Block Data Transfer (LDM/STM)
**Date:** 2026-03-03  
**Role:** Systems Programmer / ARM Emulator Architect

### What We Built
- **`execute_block_data_transfer()`** — LDM/STM with all 4 addressing modes:
  - IA (Increment After), IB (Increment Before)
  - DA (Decrement After), DB (Decrement Before / PUSH)
- Supports writeback (W bit) to update base register
- Lowest-numbered register always at lowest address (ARM convention)
- PUSH = STMDB SP!, POP = LDMIA SP!

### Tests (29 total, all pass)
- `test_push_pop_stack` — STMDB/LDMIA round-trip: PUSH {R0,R1}, POP {R2,R3} ✅
- `test_stm_ldm_multiple` — STMIA/LDMIA 4-register transfer ✅

---

## Session 10: ARM Disassembler & Custom Program Loader
**Date:** 2026-03-03  
**Role:** WebAssembly & Frontend UI Engineer

### What We Built
- **`disassemble_instruction(instr: u32) -> String`** — ARM disassembler covering:
  - Data Processing (MOV/ADD/SUB/CMP/AND/ORR/EOR/BIC/MVN) with barrel shift notation
  - Condition suffixes (EQ/NE/CS/CC/MI/PL etc.)
  - LDR/STR with offset/pre-index/post-index/writeback notation
  - LDM/STM with register list formatting
  - B/BL with signed offset
- **`get_cpu_state()`** now includes `disasm[]` — next 5 instructions from PC
- **`load_custom_hex(hex_string)`** — parses hex, writes to 0x8000, resets PC
- **Disassembly panel** — shows next 5 instructions, current PC highlighted cyan
- **Custom Program panel** — textarea for pasting hex + "Upload to RAM" button

### Verified
- Load Demo → Step: `0x00008004: MOV R1, #10` highlighted ✅
- Disassembly shows `ADD R2, R0, R1` / `SUB R3, R2, #1` / `CMP R3, #14` / `BEQ #+8` ✅
- Hex upload textarea + Upload to RAM button visible ✅

---

## Session 11: Multiply (MUL/MLA) & Branch Exchange (BX)
**Date:** 2026-03-03  
**Role:** Lead Systems Programmer / ARM Architecture Expert

### What We Built
- **`execute_multiply()`** — MUL (Rd = Rm * Rs) and MLA (Rd = Rm * Rs + Rn)
  - Correct register encoding: Rd [19:16], Rn [15:12], Rs [11:8], Rm [3:0]
  - Optional S flag for CPSR N/Z updates
- **`execute_branch_exchange()`** — BX Rm with Thumb interworking
  - LSB = 1 → set T flag in CPSR, clear LSB, switch to Thumb
  - LSB = 0 → clear T flag, stay in ARM mode
- Dispatch detection: MUL/MLA identified by bits [7:4]=1001, BX by 0x012FFF1x
- Disassembler updated for MUL, MLA, BX

### Tests (33 total, all pass)
- `test_mul` — 5 * 6 = 30 ✅
- `test_mla` — 5 * 6 + 10 = 40 ✅
- `test_bx_to_thumb` — R0 = 0x101 → PC = 0x100, T flag set ✅
- `test_bx_stay_arm` — R0 = 0x100 → PC = 0x100, T flag clear ✅

---

## Session 12: Software Interrupt (SWI / SVC)
**Date:** 2026-03-03  
**Role:** Systems Programmer / OS Architect

### What We Built
- **CPSR mode infrastructure** — mode bits [4:0], IRQ disable (bit 7), mode constants (User=0x10, SVC=0x13)
- **SPSR_svc** — Saved Program Status Register for Supervisor mode exceptions
- **`execute_swi()`** — full ARM exception handling:
  1. Save CPSR → SPSR_svc (preserves original flags + mode)
  2. Save next instruction address → LR (return address)
  3. Switch to Supervisor mode (0x13)
  4. Disable IRQ interrupts
  5. Force ARM mode (clear T flag)
  6. Jump to SWI vector (0x00000008)
- **Debug log** — `🚨 SWI executed: Syscall number 0xNNNNNN` in browser console
- **Disassembler** — `SWI #0x000042` formatting

### Tests (35 total, all pass)
- `test_swi_exception` — mode=SVC, LR=return addr, IRQ disabled, PC=0x08 ✅
- `test_swi_preserves_spsr` — SPSR_svc saves pre-SWI CPSR with Z flag ✅

---

## Session 13: Memory-Mapped I/O & Virtual UART
**Date:** 2026-03-03  
**Role:** Systems Engineer / Hardware Emulation Expert

### What We Built
- **MMIO interception** in `memory.rs` — all read/write methods check address against MMIO ranges before RAM access
- **Virtual UART at 0x10000000:**
  - TX (0x10000000): write a byte → accumulates in buffer; newline flushes to `console.log` with `📟 UART:` prefix
  - RX (0x10000004): read stub, returns 0 (no incoming data)
- **`uart_buffer()`** accessor for testing/debugging
- `write_u16`/`write_u32` to UART TX: only sends low byte (like real UART)

### Tests (39 total, all pass)
- `test_uart_tx_buffer` — 'H' + 'i' → buffer = "Hi", newline clears ✅
- `test_uart_tx_does_not_write_ram` — UART writes don't touch RAM ✅
- `test_uart_rx_returns_zero` — UART RX read returns 0 ✅
- `test_uart_write_u32_only_sends_low_byte` — 0x41 → 'A' ✅

---

## Session 14: BLX and Halfword Load/Stores
**Date:** 2026-03-03  
**Role:** Lead Systems Programmer / ARM Architecture Expert

### What We Built
- **BLX (Register)**: Branch with Link and Exchange.
  - Implemented `execute_blx_register()`
  - Saves return address (current PC + 4) into Link Register (R14).
  - Uses LSB of target address to correctly switch between ARM and Thumb modes.
- **Halfword/Signed Data Transfers**:
  - Implemented `execute_halfword_transfer()`
  - Added support for **STRH**, **LDRH** (zero-extended), **LDRSB** (sign-extended to 32 bits), and **LDRSH** (sign-extended to 32 bits).
  - Handles immediate and register offsets, pre/post-indexing, up/down, and write-back.
- **Disassembler**: Added string formatting for `BLX Rm` and all four extra load/stores with their respective addressing modes.

### Tests (44 total, all pass)
- `test_blx_register` — Validates branch to PC, T flag update, and LR save. ✅
- `test_strh_stores_halfword` — Validates only lower 16-bits are written. ✅
- `test_ldrh_zero_extends` — Validates unsigned 16-bit load. ✅
- `test_ldrsh_sign_extends` — Validates sign extension of 16-bit loaded value. ✅
- `test_ldrsb_sign_extends` — Validates sign extension of 8-bit loaded value. ✅

---

## What's Next (Phase 4)
- [ ] UART-based "Hello World" demo program
- [ ] Basic syscall handler at vector 0x08








