// snake.c — Neko Snake: the first game for nekodroid
//
// Uses every piece of MMIO hardware: VRAM, keyboard input, VSYNC timer, APU.
// Arrow keys to steer. Eat the red food. Don't hit the walls or yourself.
// Press any arrow key to restart after game over.
//
// Compile:
//   arm-none-eabi-gcc -nostdlib -nostartfiles -Ttext=0x8000 -mcpu=arm7tdmi -marm -O2 \
//     -o snake.elf start.S snake.c
//   arm-none-eabi-objcopy -O binary snake.elf snake.bin

volatile unsigned int * const VRAM       = (unsigned int *)0x04000000;
volatile unsigned int * const INPUT_KEY  = (unsigned int *)0x10000008;
volatile unsigned int * const SYS_TIMER  = (unsigned int *)0x10000014;
volatile unsigned int * const AUDIO_CTRL = (unsigned int *)0x10000018;
volatile unsigned int * const AUDIO_FREQ = (unsigned int *)0x1000001C;

#define WIDTH 800
#define HEIGHT 600
#define GRID_W (WIDTH / GRID_SIZE)   // 40
#define GRID_H (HEIGHT / GRID_SIZE)  // 30
#define GRID_SIZE 20
#define MAX_SNAKE 1000

// Colors (RGBA Little-Endian)
#define COLOR_BG    0xFF111111 // Dark Gray
#define COLOR_SNAKE 0xFF00FF00 // Green
#define COLOR_HEAD  0xFF88FF88 // Light Green
#define COLOR_FOOD  0xFF0000FF // Red
#define COLOR_DEAD  0xFF4444FF // Red-ish for game over

// Keycodes (Standard JS keycodes)
#define KEY_UP    38
#define KEY_DOWN  40
#define KEY_LEFT  37
#define KEY_RIGHT 39

// ── Minimal libc stubs (no stdlib in -nostdlib builds) ────────────────
void *memmove(void *dest, const void *src, unsigned int n) {
    unsigned char *d = (unsigned char *)dest;
    const unsigned char *s = (const unsigned char *)src;
    if (d < s) {
        while (n--) *d++ = *s++;
    } else {
        d += n; s += n;
        while (n--) *--d = *--s;
    }
    return dest;
}

// Binary long division — O(32) iterations instead of O(N) subtraction
unsigned int __aeabi_uidivmod(unsigned int num, unsigned int den) {
    if (den == 0) return 0;
    unsigned int quot = 0;
    unsigned int rem = 0;
    for (int i = 31; i >= 0; i--) {
        rem = (rem << 1) | ((num >> i) & 1);
        if (rem >= den) {
            rem -= den;
            quot |= (1u << i);
        }
    }
    // GCC expects quotient in r0, remainder in r1
    __asm__ volatile("mov r1, %0" :: "r"(rem) : "r1");
    return quot;
}

unsigned int __aeabi_uidiv(unsigned int num, unsigned int den) {
    return __aeabi_uidivmod(num, den);
}

// ── State ─────────────────────────────────────────────────────────────
int snake_x[MAX_SNAKE];
int snake_y[MAX_SNAKE];
int snake_len;
int dir_x, dir_y;
int food_x, food_y;
int game_over;
int sound_timer;

// Simple PRNG for food placement
unsigned int seed = 42;
unsigned int random() {
    seed ^= seed << 13;
    seed ^= seed >> 17;
    seed ^= seed << 5;
    return seed;
}

// ── Drawing ───────────────────────────────────────────────────────────
// Draw a single grid cell (GRID_SIZE-1 × GRID_SIZE-1 with 1px gap)
void draw_cell(int gx, int gy, unsigned int color) {
    int px = gx * GRID_SIZE;
    int py = gy * GRID_SIZE;
    for (int y = 0; y < GRID_SIZE - 1; y++) {
        for (int x = 0; x < GRID_SIZE - 1; x++) {
            VRAM[(py + y) * WIDTH + (px + x)] = color;
        }
    }
}

// Clear entire screen — only called once at init
void clear_screen() {
    for (int i = 0; i < WIDTH * HEIGHT; i++) {
        VRAM[i] = COLOR_BG;
    }
}

void play_sound(int freq, int duration_frames) {
    *AUDIO_FREQ = freq;
    *AUDIO_CTRL = 1 | (0 << 1); // Enable, Square wave
    sound_timer = duration_frames;
}

// ── Game Init ─────────────────────────────────────────────────────────
void init_game() {
    snake_len = 5;
    dir_x = 1;
    dir_y = 0;
    game_over = 0;
    sound_timer = 0;
    food_x = 15;
    food_y = 10;

    for (int i = 0; i < snake_len; i++) {
        snake_x[i] = 10 - i;
        snake_y[i] = 10;
    }

    clear_screen();

    // Draw initial snake
    draw_cell(snake_x[0], snake_y[0], COLOR_HEAD);
    for (int i = 1; i < snake_len; i++) {
        draw_cell(snake_x[i], snake_y[i], COLOR_SNAKE);
    }
    // Draw initial food
    draw_cell(food_x, food_y, COLOR_FOOD);
}

// ── Entry Point ───────────────────────────────────────────────────────
void _start(void) {
    init_game();

    unsigned int last_time = 0;
    int frame_skip = 0;

    while (1) {
        // VSYNC Wait — spin until timer changes
        unsigned int current_time = *SYS_TIMER;
        if (current_time == last_time) continue;
        last_time = current_time;

        // Handle Audio Timer
        if (sound_timer > 0) {
            sound_timer--;
            if (sound_timer == 0) *AUDIO_CTRL = 0;
        }

        // Input Handling
        unsigned int key = *INPUT_KEY;

        // Game Over — press any arrow to restart
        if (game_over) {
            if (key == KEY_UP || key == KEY_DOWN || key == KEY_LEFT || key == KEY_RIGHT) {
                init_game();
                last_time = *SYS_TIMER;
                frame_skip = 0;
            }
            continue;
        }

        // Direction changes (prevent 180° reversal)
        if (key == KEY_UP    && dir_y == 0) { dir_x =  0; dir_y = -1; }
        if (key == KEY_DOWN  && dir_y == 0) { dir_x =  0; dir_y =  1; }
        if (key == KEY_LEFT  && dir_x == 0) { dir_x = -1; dir_y =  0; }
        if (key == KEY_RIGHT && dir_x == 0) { dir_x =  1; dir_y =  0; }

        // Game speed: update every 4th VSYNC tick
        frame_skip++;
        if (frame_skip < 4) continue;
        frame_skip = 0;

        // Calculate new head position
        int new_x = snake_x[0] + dir_x;
        int new_y = snake_y[0] + dir_y;

        // Wall Collision
        if (new_x < 0 || new_x >= GRID_W || new_y < 0 || new_y >= GRID_H) {
            game_over = 1;
            // Flash snake red
            for (int i = 0; i < snake_len; i++) {
                draw_cell(snake_x[i], snake_y[i], COLOR_DEAD);
            }
            play_sound(150, 30);
            continue;
        }

        // Self Collision
        for (int i = 0; i < snake_len; i++) {
            if (new_x == snake_x[i] && new_y == snake_y[i]) {
                game_over = 1;
                for (int j = 0; j < snake_len; j++) {
                    draw_cell(snake_x[j], snake_y[j], COLOR_DEAD);
                }
                play_sound(150, 30);
                break;
            }
        }
        if (game_over) continue;

        // Check food BEFORE moving (so we know whether to grow)
        int ate_food = (new_x == food_x && new_y == food_y);

        // ── Incremental render (NOT clear_screen!) ────────────────
        // 1. Erase old tail (only if not growing)
        if (!ate_food) {
            draw_cell(snake_x[snake_len - 1], snake_y[snake_len - 1], COLOR_BG);
        }

        // 2. Old head becomes body color
        draw_cell(snake_x[0], snake_y[0], COLOR_SNAKE);

        // Move body segments
        if (ate_food && snake_len < MAX_SNAKE) {
            snake_len++;
        }
        for (int i = snake_len - 1; i > 0; i--) {
            snake_x[i] = snake_x[i - 1];
            snake_y[i] = snake_y[i - 1];
        }
        snake_x[0] = new_x;
        snake_y[0] = new_y;

        // 3. Draw new head
        draw_cell(new_x, new_y, COLOR_HEAD);

        // Handle food
        if (ate_food) {
            // Spawn new food (avoid snake body)
            int valid = 0;
            while (!valid) {
                food_x = random() % GRID_W;
                food_y = random() % GRID_H;
                valid = 1;
                for (int i = 0; i < snake_len; i++) {
                    if (food_x == snake_x[i] && food_y == snake_y[i]) {
                        valid = 0;
                        break;
                    }
                }
            }
            draw_cell(food_x, food_y, COLOR_FOOD);
            play_sound(600, 5);
        }
    }
}
