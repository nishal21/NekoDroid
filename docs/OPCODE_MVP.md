# DEX opcode MVP checklist

Implement these first (needed for `testdata/hello.apk`):

- [x] `nop` (0x00)
- [x] `move` / `move-object` family (0x01–0x09 subset used)
- [x] `move-result` / `move-result-object` (0x0a–0x0c)
- [x] `const/4` `const/16` `const` `const-string` (0x12–0x1a)
- [x] `return-void` `return` `return-object` (0x0e–0x11)
- [x] `goto` (0x28)
- [x] `if-eq` / `if-ne` / `if-eqz` / `if-nez` / `if-lt`…`if-le` (0x32–0x39)
- [x] `new-instance` (0x22)
- [x] `invoke-virtual` `invoke-direct` `invoke-static` (0x6e–0x71)
- [x] `iget` / `iput` object variants (0x52–0x5b subset)

Session 55 additions:

- [x] `check-cast` (0x1f)
- [x] `array-length` (0x21) / `new-array` (0x23)
- [x] `aget` / `aput` (0x44 / 0x4b)
- [x] `add-int` / `sub-int` / `mul-int` (+ `/2addr`) (0x90–0x92, 0xb0–0xb2)

Unimplemented opcodes log op+pc and halt the VM cleanly.
