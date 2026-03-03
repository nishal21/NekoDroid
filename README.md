<p align="center">
  <img src="https://img.shields.io/badge/status-experimental-orange?style=for-the-badge" alt="Status: Experimental" />
  <img src="https://img.shields.io/badge/rust-wasm--bindgen-blue?style=for-the-badge&logo=rust" alt="Rust + Wasm" />
  <img src="https://img.shields.io/badge/vite-typescript-646CFF?style=for-the-badge&logo=vite" alt="Vite + TypeScript" />
  <img src="https://img.shields.io/badge/license-MIT-green?style=for-the-badge" alt="MIT License" />
</p>

# 🐱 nekodroid

> **Run Android APKs locally in the browser. No cloud. No streaming. Pure WebAssembly.**

nekodroid is an open-source, browser-native Android emulator powered entirely by WebAssembly. The goal is to take a standard `.apk` file, drop it into a browser tab, and run it — no server, no backend, no remote VM. Everything executes client-side through hardware-level emulation compiled to Wasm.

This is a **moonshot project**. We are building the full stack from scratch.

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

| Layer | Technology | Responsibility |
|-------|-----------|----------------|
| **Browser** | HTML / TypeScript / Vite | UI shell, file I/O, canvas rendering, audio output |
| **Wasm Module** | Rust → `wasm-bindgen` → `.wasm` | Compiled emulator core, memory management, JIT bridge |
| **Virtual CPU** | Rust (ARMv7/AArch64 emulation) | Instruction decoding, register file, MMU, interrupt handling |
| **Android OS** | Stripped AOSP / Linux kernel | Syscall layer, Binder IPC, SurfaceFlinger stub, Dalvik/ART runtime |
| **APK** | Standard `.apk` file | The target application running inside the emulated Android environment |

---

## 🗺️ Roadmap

This is a multi-phase, multi-year effort. Here's where we're headed:

### Phase 0 — Foundation (Current)
- [x] Project scaffold (Vite + TypeScript + Rust)
- [x] Wasm build pipeline (`wasm-bindgen`, `cdylib`)
- [ ] Basic Rust → Wasm "hello world" round-trip
- [ ] CI/CD with `wasm-pack` build validation

### Phase 1 — Virtual CPU
- [ ] ARMv7 instruction decoder (Thumb + ARM mode)
- [ ] Register file (R0–R15, CPSR)
- [ ] Basic ALU operations (ADD, SUB, MOV, CMP, branch)
- [ ] Memory bus abstraction (read/write 8/16/32-bit)
- [ ] Interrupt controller stub
- [ ] Unit test suite: run ARM assembly snippets and validate register state

### Phase 2 — Memory & Peripherals
- [ ] MMU with basic page table translation
- [ ] Framebuffer device mapped to HTML `<canvas>`
- [ ] Keyboard/touch input bridged from browser events
- [ ] Timer peripheral (for OS scheduler)
- [ ] UART for debug logging to browser console

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
| Testing | Rust unit tests, Playwright (browser integration) |
| CI | GitHub Actions |

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

### Project Structure

```
nekodroid/
└── browser-droid/
    ├── src/              # TypeScript frontend (Vite)
    │   ├── main.ts       # Entry point
    │   └── ...
    ├── src/lib.rs        # Rust emulator core (Wasm)
    ├── Cargo.toml        # Rust/Wasm configuration
    ├── vite.config.ts    # Vite + Wasm plugin config
    ├── package.json      # Node dependencies
    └── index.html        # Shell HTML
```

---

## 🤝 Call for Contributors

This project is **deeply ambitious** and we need help across the entire stack. If any of these areas excite you, we want you on the team:

### 🔴 Critical — CPU Emulation (Rust/C++)
We need low-level systems programmers who can implement ARM instruction decoding and execution in Rust. Experience with emulators (QEMU, Unicorn, dynarmic) is a huge plus.

### 🟠 High Priority — Wasm Optimization
If you've pushed the limits of WebAssembly performance — SIMD, threading, memory management — we need your expertise to make the emulator fast enough to be usable.

### 🟡 Important — Android Internals
Knowledge of AOSP, the Linux kernel, Binder IPC, or ART/Dalvik is essential for Phases 3–4. If you've built custom ROMs or worked on Android frameworks, this is your playground.

### 🟢 Welcome — Frontend & DevEx
Building the drag-and-drop UI, debug tools, performance dashboards, and documentation. TypeScript + Vite experience is all you need.

### How to Contribute

1. Fork the repository
2. Create a feature branch: `git checkout -b feat/arm-decoder`
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

nekodroid is an experiment in pushing the boundary of what's possible on the web. It may take years. It may never fully work for complex apps. But we're going to try — and we're going to do it in the open.

**Star ⭐ this repo if you believe in the mission.**
