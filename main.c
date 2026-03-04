// Define the UART TX address
volatile unsigned char * const UART_TX = (unsigned char *)0x10000000;

void print_string(const char *str);

// _start is our entry point at 0x8000
void __attribute__((section(".text._start"))) _start() {
    print_string("Hello from Bare-Metal C running on NekoDroid!\n");
    print_string("If you are reading this, your ARM CPU is fully functional.\n");
    
    // Halt
    while (1) {}
}

void print_string(const char *str) {
    while (*str) {
        *UART_TX = *str++;
    }
}