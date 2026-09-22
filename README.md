<p align="center">
  <img src="https://img.shields.io/badge/status-active_development-brightgreen?style=for-the-badge" alt="Status: Active Development" />
  <img src="https://img.shields.io/badge/rust-wasm--bindgen-blue?style=for-the-badge&logo=rust" alt="Rust + Wasm" />
  <img src="https://img.shields.io/badge/vite-typescript-646CFF?style=for-the-badge&logo=vite" alt="Vite + TypeScript" />
  <img src="https://img.shields.io/badge/tests-116_passing-brightgreen?style=for-the-badge" alt="116 Tests Passing" />
  <img src="https://img.shields.io/badge/license-MIT-green?style=for-the-badge" alt="MIT License" />
</p>

# nekodroid

Browser-native ARM emulator in Rust/WebAssembly, with a **constrained APK/DEX HLE runner**. The long-term goal is richer Android support. Today you can run bare-metal ARM binaries, explore Linux boot plumbing, and run a small hello APK through Dalvik HLE (not full ART/AOSP).

## What works today

- ARMv7-era CPU core (ARM + Thumb), pipeline-accurate PC reads
- 116 unit tests (`cargo test --lib`)
- ROM / custom hex upload, UART, VRAM canvas, input, audio MMIO
- VersatilePB subset: PL011 UART, SP804 timer, PL190 VIC + IRQ path
- CP15 + short-descriptor MMU (section + coarse L2)
- Linux ATAG boot helper + UI upload for zImage/initrd
- **APK HLE:** parse APK/DEX, MVP Dalvik interpreter, stubbed Log/println, canvas text blit (`testdata/hello.apk`)

## What does not work yet

- Arbitrary Play Store APKs, multidex, JNI/native libs
- Full Android userspace (zygote, Binder fidelity, SurfaceFlinger)
- Guaranteed goldfish kernel bring-up on every image

See [docs/SUPPORTED_APKS.md](docs/SUPPORTED_APKS.md).

## Nesting doll (target architecture)

```
Browser → Wasm → ARM CPU (+ optional DEX HLE) → (future) Android OS → APK
```

Right now the DEX HLE path runs selected APKs **without** booting a full Android OS. Goldfish MMIO stubs exist for later kernel work.

## Getting started

```bash
git clone https://github.com/nishal21/NekoDroid.git
cd NekoDroid
npm install
wasm-pack build --target web
npm run dev
```

Open http://localhost:5173

### Commands

```bash
cargo test --lib          # or: npm test
npm run build:wasm
npm run build             # wasm-pack + TypeScript + Vite
```

### Try the hello APK

1. Build Wasm, start `npm run dev`
2. In the UI, **Load APK** → choose `testdata/hello.apk`
3. **Run APK (HLE)** — check logcat panel and VRAM mode

Regenerate fixtures: `cargo test --lib write_fixtures_to_testdata`

## Project layout

```
src/                 Rust emulator + android/ HLE + TypeScript UI
public/              Static assets / host headers
testdata/            hello.apk and related fixtures
docs/                REFERENCES, SUPPORTED_APKS, OPCODE_MVP
examples/hello-apk/  Notes for rebuilding fixtures
```

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Keep changes at the repo root (do not publish a nested `browser-droid/` folder).

## License

MIT — see [LICENSE](LICENSE).
