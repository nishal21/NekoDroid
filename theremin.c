// theremin.c — Touch-controlled synthesizer for nekodroid
//
// Drag on the canvas: X = pitch (100–900 Hz), Y = waveform (square/sine/saw/tri)
// Release to silence.
//
// Compile:
//   arm-none-eabi-gcc -nostdlib -nostartfiles -Ttext=0x8000 -mcpu=arm7tdmi -marm -O2 \
//     -o theremin.elf start.S theremin.c
//   arm-none-eabi-objcopy -O binary theremin.elf theremin.bin

volatile unsigned int * const INPUT_TOUCH = (unsigned int *)0x1000000C;
volatile unsigned int * const INPUT_COORD = (unsigned int *)0x10000010;
volatile unsigned int * const AUDIO_CTRL  = (unsigned int *)0x10000018;
volatile unsigned int * const AUDIO_FREQ  = (unsigned int *)0x1000001C;

void _start(void) {
    while (1) {
        if (*INPUT_TOUCH == 1) {
            unsigned int coords = *INPUT_COORD;
            unsigned int x = coords & 0xFFFF;
            unsigned int y = (coords >> 16) & 0xFFFF;

            // Map X coordinate (0-800) to frequency (100Hz - 900Hz)
            *AUDIO_FREQ = 100 + x;

            // Map Y coordinate (0-600) to waveform type (0-3)
            unsigned int wave_type = (y / 150) & 3;

            // Enable audio (Bit 0) and set waveform (Bits 1-2)
            *AUDIO_CTRL = 1 | (wave_type << 1);
        } else {
            // Disable audio
            *AUDIO_CTRL = 0;
        }
    }
}
