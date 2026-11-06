.section .text.boot
.global _start
.extern kernel_main

_start:
    // Save DTB pointer
    mov x19, x0

    // Force 36-bit Physical Address Size (or larger) in TCR_EL2
    // If the firmware booted us in EL2, this prevents data aborts when touching 0x1C00030000
    mrs     x0, tcr_el2
    orr     x0, x0, #(1 << 16)      // PS = 0b001 (36 bits, 64GB)
    msr     tcr_el2, x0
    isb

    // Only run on Primary Core (Core 0)
    mrs x0, MPIDR_EL1
    and x0, x0, #0xff
    cbz x0, primary_core

secondary_core:
    wfe
    b secondary_core

primary_core:
    ldr x0, =__stack_top
    mov sp, x0
    ldr x0, =__bss_start
    ldr x1, =__bss_end

clear_bss:
    cmp x0, x1
    b.hs bss_done
    str xzr, [x0], #8
    b clear_bss

bss_done:
    mov x29, xzr
    mov x0, x19
    bl kernel_main

halt:
    wfe
    b halt
