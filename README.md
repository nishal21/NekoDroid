<p align="center">
  <img src="https://img.shields.io/badge/status-active_development-brightgreen?style=for-the-badge" alt="Status: Active Development" />
  <img src="https://img.shields.io/badge/rust-wasm--bindgen-blue?style=for-the-badge&logo=rust" alt="Rust + Wasm" />
  <img src="https://img.shields.io/badge/vite-typescript-646CFF?style=for-the-badge&logo=vite" alt="Vite + TypeScript" />
  <img src="https://img.shields.io/badge/tests-61_passing-brightgreen?style=for-the-badge" alt="61 Tests Passing" />
  <img src="https://img.shields.io/badge/license-MIT-green?style=for-the-badge" alt="MIT License" />
</p>

# 🐱 nekodroid

> **Run Android APKs locally in the browser. No cloud. No streaming. Pure WebAssembly.**

nekodroid is an open-source, browser-native Android emulator powered entirely by WebAssembly. The goal is to take a standard `.apk` file, drop it into a browser tab, and run it — no server, no backend, no remote VM. Everything executes client-side through hardware-level emulation compiled to Wasm.

**The ARM CPU core is functional** — it successfully executes GCC-compiled bare-metal C programs, with full ARM and Thumb instruction set support, UART output, and a browser-based debug UI.

---

## ✅ What Works Today

- **ARM7TDMI CPU core** — Full 32-bit ARM and 16-bit Thumb instruction set emulation
- **61 unit tests passing** — Comprehensive test coverage across all instruction formats
- **ROM loading** — Upload `.bin` files directly in the browser and execute them
- **UART output** — Programs can print to the browser console via memory-mapped I/O
- **GCC-compiled C execution** — Bare-metal C programs compiled with `arm-none-eabi-gcc` run correctly
- **Live debug panel** — Step through instructions, inspect registers, view memory and disassembly
- **Pipeline-accurate PC** — Correct ARM pipeline prefetch behavior (PC reads return instruction + 8/4)

```
📟 UART: Hello from Bare-Metal C running on NekoDroid!
📟 UART: If you are reading this, your ARM CPU is fully functional.
```

---

## 🏗️ The "Nesting Doll" Architecture

nekodroid follows a layered emulation model where each layer hosts the one above it — like Russian nesting dolls:

```
┌──────────────────────────────────────────────────────┐
│                    YOUR BROWSER                      │
│  ┌────────────────────────────────────────────────┐  │
│  │          WebAssembly (Wasm Module)              │  │
│  │  ┌──────────────────────────────────────────┐  │  │
│  │  │        Virtual ARM CPU (Emulator)         │  │  │
│  │  │  ┌────────────────────────────────────┐  │  │  │
│  │  │  │    Minimal Android OS (AOSP Kernel) │  │  │  │
│  │  │  │  ┌──────────────────────────────┐  │  │  │  │
│  │  │  │  │      YOUR APK (App)          │  │  │  │  │
│  │  │  │  └──────────────────────────────┘  │  │  │  │
│  │  │  └────────────────────────────────────┘  │  │  │
│  │  └──────────────────────────────────────────┘  │  │
│  └────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────┘
```

### Layer Breakdown

| Layer | Technology | Status |
|-------|-----------|--------|
| **Browser** | HTML / TypeScript / Vite | ✅ Debug UI, ROM upload, canvas rendering |
| **Wasm Module** | Rust → `wasm-bindgen` → `.wasm` | ✅ Full CPU core compiled to Wasm |
| **Virtual CPU** | Rust (ARM7TDMI emulation) | ✅ ARM + Thumb instruction sets, pipeline-accurate |
| **Memory / Peripherals** | Rust (MMU + MMIO) | ✅ RAM, ROM, UART — framebuffer in progress |
| **Android OS** | Stripped AOSP / Linux kernel | 🔲 Future phase |
| **APK** | Standard `.apk` file | 🔲 Future phase |

---

## 🗺️ Roadmap

### Phase 0 — Foundation ✅
- [x] Project scaffold (Vite + TypeScript + Rust)
- [x] Wasm build pipeline (`wasm-bindgen`, `cdylib`)
- [x] Rust → Wasm round-trip working
- [x] Dev server with hot reload

### Phase 1 — Virtual CPU ✅
- [x] ARM 32-bit instruction decoder (Data Processing, Branch, LDR/STR, Block Transfer, MUL, SWI)
- [x] Thumb 16-bit instruction decoder (all 19 formats)
- [x] Register file (R0–R15, CPSR with N/Z/C/V flags)
- [x] Full ALU operations (ADD, SUB, MOV, CMP, AND, ORR, EOR, BIC, MVN, RSB, ADC, SBC, TST, TEQ, CMN)
- [x] Barrel shifter (LSL, LSR, ASR, ROR, RRX)
- [x] ARM/Thumb mode switching (BX instruction)
- [x] Pipeline-accurate PC reads (instruction + 8 for ARM, + 4 for Thumb)
- [x] Conditional execution (all 15 condition codes)
- [x] Long branch with link (BL) in Thumb mode
- [x] PUSH/POP with link register support
- [x] 61 unit tests covering all instruction formats

### Phase 2 — Memory & Peripherals (In Progress)
- [x] Memory bus with 8/16/32-bit read/write
- [x] ROM region (loaded at 0x8000)
- [x] RAM region (configurable size)
- [x] UART TX via memory-mapped I/O (0x10000000) → browser console
- [x] ROM upload from browser (FileReader API)
- [ ] Framebuffer device mapped to HTML `<canvas>`
- [ ] Keyboard/touch input bridged from browser events
- [ ] Timer peripheral (for OS scheduler)
- [ ] Interrupt controller

### Phase 3 — Linux Kernel Bootstrap
- [ ] Load a minimal ARM Linux kernel image into emulated memory
- [ ] Boot to init process
- [ ] Implement essential syscalls (`read`, `write`, `mmap`, `ioctl`, `clone`)
- [ ] `/dev` and `/proc` virtual filesystem stubs

### Phase 4 — Android Userspace
- [ ] Dalvik/ART bytecode interpreter or ahead-of-time compiler
- [ ] Binder IPC mechanism
- [ ] SurfaceFlinger rendering pipeline → canvas
- [ ] Zygote process & app lifecycle
- [ ] APK parsing (AndroidManifest, DEX, resources)

### Phase 5 — Usability & Performance
- [ ] Drag-and-drop APK loader UI
- [ ] JIT compilation for hot CPU paths
- [ ] WebGPU acceleration for rendering
- [ ] SharedArrayBuffer multi-threading
- [ ] Persistent storage via IndexedDB / OPFS
- [ ] PWA support for offline use

---

## 🛠️ Tech Stack

| Domain | Tool |
|--------|------|
| Frontend | TypeScript, Vite, HTML5 Canvas |
| Emulator Core | Rust, `wasm-bindgen`, `wasm-pack` |
| Build Target | WebAssembly (`wasm32-unknown-unknown`) |
| Testing | Rust unit tests (61 passing) |
| C Toolchain | `arm-none-eabi-gcc` (for test binaries) |

---

## 🚀 Getting Started

### Prerequisites

- **Node.js** ≥ 18
- **Rust** (stable) — install via [rustup.rs](https://rustup.rs)
- **wasm-pack** — `cargo install wasm-pack`

### Setup

```bash
# Clone the repository
git clone https://github.com/nishal21/NekoDroid.git
cd nekodroid/browser-droid

# Install JS dependencies
npm install

# Build the Wasm module
wasm-pack build --target web

# Start the dev server
npm run dev
```

Open [http://localhost:5173](http://localhost:5173) in your browser.

### Loading a ROM

1. Compile a bare-metal ARM binary (e.g., with `arm-none-eabi-gcc`)
2. Click **"Select & Load .bin"** in the browser UI
3. Select your `.bin` file — it loads at address `0x8000`
4. Use the debug panel to step through execution or run continuously
5. UART output appears in the browser console (`📟 UART: ...`)

### Running Tests

```bash
cargo test
```

All 61 tests validate ARM and Thumb instruction behavior against expected register/memory state.

### Project Structure

```
browser-droid/
├── src/
│   ├── main.ts          # Frontend UI, debug panel, ROM upload
│   ├── style.css         # UI styling
│   ├── lib.rs            # Wasm exports (step, reset, load_rom, etc.)
│   ├── cpu.rs            # ARM7TDMI CPU core (ARM + Thumb decoder)
│   ├── cpu/tests.rs      # 61 unit tests
│   └── memory.rs         # MMU, RAM, ROM, UART MMIO
├── pkg/                  # wasm-pack output
├── Cargo.toml            # Rust/Wasm configuration
├── vite.config.ts        # Vite + Wasm plugin config
├── package.json          # Node dependencies
├── index.html            # Shell HTML
├── DEVLOG.md             # Detailed development log (33 sessions)
└── README.md
```

---

## 🤝 Call for Contributors

This project is **deeply ambitious** and we need help across the entire stack:

### 🔴 Critical — Peripherals & OS Bootstrap
The CPU core is functional. The next frontier is implementing enough peripherals (interrupt controller, timer, framebuffer) and syscalls to boot a minimal Linux kernel.

### 🟠 High Priority — Wasm Optimization
If you've pushed the limits of WebAssembly performance — SIMD, threading, memory management — we need your expertise to make the emulator fast enough for real workloads.

### 🟡 Important — Android Internals
Knowledge of AOSP, the Linux kernel, Binder IPC, or ART/Dalvik is essential for Phases 3–4. If you've built custom ROMs or worked on Android frameworks, this is your playground.

### 🟢 Welcome — Frontend & DevEx
Improving the debug UI, adding visualization tools, performance dashboards, and documentation. TypeScript + Vite experience is all you need.

### How to Contribute

1. Fork the repository
2. Create a feature branch: `git checkout -b feat/my-feature`
3. Make your changes with tests
4. Submit a pull request

Please read our [CONTRIBUTING.md](CONTRIBUTING.md) (coming soon) before submitting.

---

## 📜 License

MIT — see [LICENSE](LICENSE) for details.

---

## 💬 Philosophy

> *"Any sufficiently advanced browser tab is indistinguishable from a computer."*

We believe the browser is the most universal runtime on the planet. If we can emulate hardware in Wasm at reasonable speeds, we unlock something extraordinary: **run any Android app, anywhere, on any device with a browser — with zero installation.**

nekodroid is an experiment in pushing the boundary of what's possible on the web. The CPU already works. The journey continues.

**Star ⭐ this repo if you believe in the mission.**
