# hello-apk example

Minimal APK sources for the HLE path.

The committed binaries live in `../../testdata/` and are produced by:

```bash
cargo test --lib write_fixtures_to_testdata
```

Optional rebuild with [smali](https://github.com/JesusFreke/smali) (if installed):

```text
; Hello.smali outline — const-string + invoke-static println + return-void
```

Requirements: no native libs, single DEX, MVP opcodes only. See `docs/SUPPORTED_APKS.md`.
