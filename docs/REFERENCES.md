# References

Primary specs for the APK/DEX HLE path:

- [DEX file format](https://source.android.com/docs/core/runtime/dex-format)
- [Dalvik bytecode](https://source.android.com/docs/core/runtime/dalvik-bytecode)
- [Instruction formats](https://source.android.com/docs/core/runtime/instruction-formats)
- [Opcodes](https://developer.android.com/reference/dalvik/bytecode/Opcodes)

ART interpreter (behavior reference only; not vendored):

- [interpreter_switch_impl.h](https://android.googlesource.com/platform/art/+/master/runtime/interpreter/interpreter_switch_impl.h)
- [interpreter_common.cc](https://android.googlesource.com/platform/art/+/master/runtime/interpreter/interpreter_common.cc)

Fixture tooling:

- [smali / baksmali](https://github.com/JesusFreke/smali)
- [smali HelloWorld example](https://github.com/JesusFreke/smali/blob/master/examples/HelloWorld/HelloWorld.smali)

Optional host validators (not Wasm runtime deps):

- [rusty-axml](https://crates.io/crates/rusty-axml)
- [dexrs](https://github.com/MatrixEditor/dexrs)
