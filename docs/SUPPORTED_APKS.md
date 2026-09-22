# Supported APKs

NekoDroid's browser APK path is a constrained HLE Dalvik runner. It is not full Android.

## Supported now (Milestone 1)

| APK | Notes |
|-----|--------|
| `testdata/hello.apk` | Single DEX, no native `.so`, prints via HLE Log / canvas text |

## Requirements for a fixture APK

- One `classes.dex` (no multidex)
- No JNI / `.so` libraries
- Entry: static `main` or `onCreate` on a class we can find
- Uses only MVP Dalvik opcodes listed in `OPCODE_MVP.md`
- Framework calls limited to stubbed HLE methods (`Log`, `println`, simple `TextView`/`Activity` hooks)

## Not supported

- Play Store apps in general
- Multidex, Google Play Services, Binder-heavy apps
- Full ART JIT/AOT, SurfaceFlinger, zygote
