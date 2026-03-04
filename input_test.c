// input_test.c — Interactive MMIO Input & Timer Test for nekodroid
//
// Reads keyboard, touch, and timer registers via MMIO.
// Draws a movable cursor and displays input state on screen.
//
// Compile:
//   arm-none-eabi-gcc -nostdlib -nostartfiles -T link.ld \
//     -mcpu=arm7tdmi -marm -O2 -o input_test.elf input_test.c
//   arm-none-eabi-objcopy -O binary input_test.elf input_test.bin

// ── Hardware addresses ────────────────────────────────────────────────
volatile unsigned int * const VRAM        = (unsigned int *)0x04000000;
volatile unsigned char * const UART_TX    = (unsigned char *)0x10000000;
volatile unsigned int * const INPUT_KEY   = (unsigned int *)0x10000008;
volatile unsigned int * const INPUT_TOUCH = (unsigned int *)0x1000000C;
volatile unsigned int * const INPUT_COORD = (unsigned int *)0x10000010;
volatile unsigned int * const SYS_TIMER   = (unsigned int *)0x10000014;

#define WIDTH  800
#define HEIGHT 600

// ── Colors (RGBA little-endian) ───────────────────────────────────────
#define BLACK   0xFF000000
#define WHITE   0xFFFFFFFF
#define RED     0xFF0000FF
#define GREEN   0xFF00FF00
#define BLUE    0xFFFF0000
#define YELLOW  0xFF00FFFF
#define CYAN    0xFFFFFF00
#define MAGENTA 0xFFFF00FF
#define GRAY    0xFF404040
#define ORANGE  0xFF0088FF

// ── Drawing primitives ────────────────────────────────────────────────
void draw_pixel(int x, int y, unsigned int color) {
    if (x >= 0 && x < WIDTH && y >= 0 && y < HEIGHT) {
        VRAM[y * WIDTH + x] = color;
    }
}

void fill_rect(int x0, int y0, int w, int h, unsigned int color) {
    for (int y = y0; y < y0 + h; y++) {
        for (int x = x0; x < x0 + w; x++) {
            draw_pixel(x, y, color);
        }
    }
}

// Draw a small 10x10 crosshair cursor
void draw_cursor(int cx, int cy, unsigned int color) {
    // Horizontal line
    for (int x = cx - 5; x <= cx + 5; x++) {
        draw_pixel(x, cy, color);
    }
    // Vertical line
    for (int y = cy - 5; y <= cy + 5; y++) {
        draw_pixel(cx, y, color);
    }
    // Corner dots for visibility
    draw_pixel(cx - 5, cy - 5, color);
    draw_pixel(cx + 5, cy - 5, color);
    draw_pixel(cx - 5, cy + 5, color);
    draw_pixel(cx + 5, cy + 5, color);
}

// Draw a horizontal bar (for timer visualization)
void draw_bar(int x0, int y0, int width, int max_width, int h, unsigned int color) {
    if (width > max_width) width = max_width;
    fill_rect(x0, y0, width, h, color);
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

void print_hex(unsigned int val) {
    const char hex[] = "0123456789ABCDEF";
    print_string("0x");
    for (int i = 28; i >= 0; i -= 4) {
        print_char(hex[(val >> i) & 0xF]);
    }
}

// ── Entry point ───────────────────────────────────────────────────────
void __attribute__((section(".text._start"))) _start(void) {
    print_string("Input MMIO test v2 starting...\n");

    // Draw static UI background
    fill_rect(0, 0, WIDTH, HEIGHT, BLUE);

    // Title bar
    fill_rect(0, 0, WIDTH, 30, GRAY);

    // Status boxes
    fill_rect(20, 50, 200, 80, GRAY);   // Key status box
    fill_rect(240, 50, 200, 80, GRAY);   // Touch status box
    fill_rect(460, 50, 200, 80, GRAY);   // Timer status box

    // Labels (small colored indicators)
    fill_rect(30, 55, 10, 10, YELLOW);   // Key indicator
    fill_rect(250, 55, 10, 10, GREEN);   // Touch indicator
    fill_rect(470, 55, 10, 10, CYAN);    // Timer indicator

    // Instructions area
    fill_rect(20, 150, 660, 3, WHITE);

    print_string("UI drawn. Entering main loop...\n");

    unsigned int last_key = 0xFFFFFFFF;
    unsigned int last_touch = 0xFFFFFFFF;
    int prev_cx = -1, prev_cy = -1;
    unsigned int prev_timer_bar = 0;

    // Main loop — polls input registers and updates display
    while (1) {
        unsigned int key   = *INPUT_KEY;
        unsigned int touch = *INPUT_TOUCH;
        unsigned int coord = *INPUT_COORD;
        unsigned int timer = *SYS_TIMER;

        unsigned int tx = coord & 0xFFFF;
        unsigned int ty = (coord >> 16) & 0xFFFF;

        // ── Update key display ─────────────────────────────────────
        if (key != last_key) {
            // Clear key value area
            fill_rect(50, 70, 160, 50, GRAY);

            if (key != 0) {
                // Key is pressed — draw colored block proportional to keycode
                unsigned int bar_w = key % 160;
                fill_rect(50, 80, bar_w, 30, YELLOW);
                print_string("Key: ");
                print_hex(key);
                print_char('\n');
            }
            last_key = key;
        }

        // ── Update touch display ───────────────────────────────────
        if (touch != last_touch) {
            // Clear touch value area
            fill_rect(260, 70, 170, 50, GRAY);

            if (touch) {
                fill_rect(260, 80, 170, 30, GREEN);
                print_string("Touch DOWN at (");
                print_hex(tx);
                print_string(", ");
                print_hex(ty);
                print_string(")\n");
            } else {
                fill_rect(260, 80, 170, 30, RED);
                print_string("Touch UP\n");
            }
            last_touch = touch;
        }

        // ── Update timer bar ───────────────────────────────────────
        unsigned int timer_bar = timer % 200;
        if (timer_bar != prev_timer_bar) {
            // Clear and redraw timer bar
            fill_rect(480, 80, 170, 30, GRAY);
            fill_rect(480, 80, timer_bar, 30, GREEN);
            prev_timer_bar = timer_bar;
        }

        // ── Draw touch cursor ──────────────────────────────────────
        if (touch && tx < WIDTH && ty < HEIGHT) {
            // Erase previous cursor
            if (prev_cx >= 0 && prev_cy >= 0) {
                draw_cursor(prev_cx, prev_cy, BLACK);
            }
            // Draw new cursor
            draw_cursor((int)tx, (int)ty, WHITE);
            prev_cx = (int)tx;
            prev_cy = (int)ty;

            // Draw a trail dot (persists)
            fill_rect((int)tx - 1, (int)ty - 1, 3, 3, MAGENTA);
        }
    }
}
