ENTRY(_start)

SECTIONS {
    . = 0x8000;

    .text : {
        *(.text._start)
        *(.text.startup)
        *(.text*)
    }

    .rodata : { *(.rodata*) }
    .data   : { *(.data*)   }
    .bss    : { *(.bss*)    }
}
