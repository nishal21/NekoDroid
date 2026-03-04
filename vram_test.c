// vram_test.c — Bare-metal ARM VRAM test for nekodroid
//
// Draws a red square (100x100) on a black background,
// then prints to UART confirming the draw is complete.
//
// Compile:
//   arm-none-eabi-gcc -nostdlib -nostartfiles -T link.ld \
//     -mcpu=arm7tdmi -marm -O2 -o vram_test.elf vram_test.c
//   arm-none-eabi-objcopy -O binary vram_test.elf vram_test.bin

// ── Hardware addresses ────────────────────────────────────────────────
volatile unsigned int * const VRAM = (unsigned int *)0x04000000;
volatile unsigned char * const UART_TX = (unsigned char *)0x10000000;

// ── Screen dimensions ─────────────────────────────────────────────────
#define WIDTH  800
#define HEIGHT 600

// ── Pixel drawing ─────────────────────────────────────────────────────
void draw_pixel(int x, int y, unsigned int color) {
    if (x >= 0 && x < WIDTH && y >= 0 && y < HEIGHT) {
        VRAM[y * WIDTH + x] = color;
    }
}

void fill_rect(int x0, int y0, int x1, int y1, unsigned int color) {
    for (int y = y0; y < y1; y++) {
        for (int x = x0; x < x1; x++) {
            draw_pixel(x, y, color);
        }
    }
}

// ── UART output ───────────────────────────────────────────────────────
void print_char(char c) {
    *UART_TX = (unsigned char)c;
}

void print_string(const char *s) {
    while (*s) {
        print_char(*s++);
    }
}

// ── Entry point ───────────────────────────────────────────────────────
void __attribute__((section(".text._start"))) _start(void) {
    // Draw a red 100x100 square at position (100, 100)
    fill_rect(100, 100, 200, 200, 0xFF0000FF);  // RGBA: Red=0xFF, A=0xFF

    // Draw a green 100x100 square at position (250, 100)
    fill_rect(250, 100, 350, 200, 0xFF00FF00);  // RGBA: Green=0xFF, A=0xFF

    // Draw a blue 100x100 square at position (400, 100)
    fill_rect(400, 100, 500, 200, 0xFFFF0000);  // RGBA: Blue=0xFF, A=0xFF

    // Print confirmation via UART
    print_string("VRAM test complete: RGB squares drawn!\n");

    // Halt
    while (1) {}
}
