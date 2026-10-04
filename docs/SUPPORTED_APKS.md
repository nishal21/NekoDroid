# Supported APKs

NekoDroid's browser APK path is a constrained HLE Dalvik runner. It is not full Android, and it does not run Play Store APKs.

## Supported now

| APK | Notes |
|-----|--------|
| `testdata/hello.apk` | Single DEX, text manifest, prints via HLE `println` |
| Binary-manifest fixture | Launcher activity comes from the manifest, not the first `com.*` string or the first class |
| Instance-field fixture | `<init>` stores a field; a later method reads it back |
| arith DEX (in-tree fixture) | `build_arith_dex`: 2+3 → `printInt` → logs `5` |

## What the loader actually does

- One `classes.dex` (no multidex). Inflated DEX and the APK itself are capped at 32 MiB
- Entry is the manifest launcher when the binary (or text) manifest names one, else a DEX class whose name contains `Hello`, `Main`, or `Activity`
- Constructors in the DEX run. `<init>` is not replaced just because of its name
- Instance `iget`/`iput` (int and object, plus the narrow int forms) read and write fields on the HLE object
- Framework calls are limited to stubbed HLE methods (`Log`, `println`, `printInt`, and `Activity`/`TextView` hooks on `android.*` / `java.*` classes only)
- Missing methods that are not in the DEX still return 0. That is a stub, not a real library

## Not supported

- Play Store apps, or any APK that needs the real Android framework
- Multidex, `resources.arsc`, Google Play Services, JNI / `.so`
- `iget-wide` / `iput-wide` and opcodes outside `OPCODE_MVP.md`
- ART JIT/AOT, zygote, SurfaceFlinger
- `boot_android` as an Android boot. It loads a kernel image and, if given an APK, executes that APK on this HLE VM. It does not start Android and then launch the app
