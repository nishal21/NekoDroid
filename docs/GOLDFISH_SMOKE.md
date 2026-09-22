# Goldfish kernel smoke

Real Android goldfish kernels are large and stay **local-only**.

## Local layout (gitignored)

```
test-images/
  zImage            # goldfish_defconfig kernel
  initrd.cpio.gz    # optional ramdisk
  system.img        # optional MMC-backed system image
```

`test-images/` is in `.gitignore`. Do not commit these files.

## What the emulator does today

- `boot_linux_kernel` / `boot_android` load the image into RAM and set ATAGs / machine id
- MMC CMD17/CMD18 (+ CMD12 stop) can stream a mounted `system.img` sector buffer
- Goldfish Pipe MMIO supports version probe, OPEN/CLOSE/POLL/WRITE/READ for `pipe:qemud:*` style names

## Get a kernel (Windows PowerShell)

ARMv7 goldfish prebuilt from an older AOSP commit (~2.4 MB, stays local):

```powershell
New-Item -ItemType Directory -Force -Path test-images | Out-Null
$url = "https://android.googlesource.com/platform/prebuilts/qemu-kernel/+/e992557132b90a37e6622a78ff5d5a6b89d6ddd5/arm/kernel-qemu-armv7?format=TEXT"
Invoke-WebRequest -Uri $url -OutFile test-images\_k.b64 -UseBasicParsing
$bytes = [Convert]::FromBase64String((Get-Content test-images\_k.b64 -Raw).Trim())
[IO.File]::WriteAllBytes("$PWD\test-images\zImage", $bytes)
Remove-Item test-images\_k.b64
```

Then:

```powershell
cargo test test_optional_goldfish_zimage_smoke -- --nocapture
```

## Run

1. Ensure `test-images/zImage` exists (above)
2. `cargo test test_optional_goldfish_zimage_smoke -- --nocapture`
3. Or use the UI kernel/initrd file pickers in the browser build

Without `test-images/zImage`, the optional test **skips** (still passes).

## Goldfish TTY

Classic serial is at `0xFF002000`:

| Offset | Name | Notes |
|--------|------|--------|
| 0x00 | PUT_CHAR | earlyprintk byte out |
| 0x08 | CMD | 2 = WRITE_BUFFER |
| 0x10 / 0x14 | DATA_PTR / DATA_LEN | buffer write |

`boot_linux_kernel` uses goldfish machine id `0x046F` and cmdline `console=ttyS0 earlyprintk …`.

Kernel decompress can take millions of instructions before the first UART byte; the step smoke checks PC progress and reports UART if/when it appears.
