.section .text.boot
.global _start
.extern kernel_main

_start:
    mov x19, x0

    mrs x0, MPIDR_EL1
    and x0, x0, #0xff
    cbnz x0, secondary_core

    mrs x0, CurrentEL
    lsr x0, x0, #2
    and x0, x0, #0x3
    cmp x0, #2
    b.ne .L_setup_mmu

    mrs x0, sctlr_el2
    bic x0, x0, #(1 << 0)
    bic x0, x0, #(1 << 2)
    bic x0, x0, #(1 << 12)
    msr sctlr_el2, x0
    isb

.L_setup_mmu:
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
    ldr x0, =page_table_l1

    ldr x1, =0x00000000
    ldr x2, =0x00000741
    orr x1, x1, x2
    str x1, [x0, #0]

    ldr x1, =0x40000000
    ldr x2, =0x00000741
    orr x1, x1, x2
    str x1, [x0, #8]

    ldr x1, =0x80000000
    ldr x2, =0x00000741
    orr x1, x1, x2
    str x1, [x0, #16]

    ldr x1, =0xC0000000
    ldr x2, =0x00000741
    orr x1, x1, x2
    str x1, [x0, #24]

    ldr x1, =0x1C00000000
    ldr x2, =0x00000401
    orr x1, x1, x2
    str x1, [x0, #224]

    mrs x0, CurrentEL
    lsr x0, x0, #2
    and x0, x0, #0x3
    cmp x0, #2
    b.ne .L_mmu_el1

    ldr x0, =page_table_l1
    msr ttbr0_el2, x0

    ldr x0, =0x0000ff04
    msr mair_el2, x0

    ldr x0, =0x80802520
    msr tcr_el2, x0
    isb

    mrs x0, sctlr_el2
    orr x0, x0, #(1 << 0)
    orr x0, x0, #(1 << 2)
    orr x0, x0, #(1 << 12)
    msr sctlr_el2, x0
    isb
    b .L_jump_kernel

.L_mmu_el1:
    ldr x0, =page_table_l1
    msr ttbr0_el1, x0

    ldr x0, =0x0000ff04
    msr mair_el1, x0

    ldr x0, =0x10802520
    msr tcr_el1, x0
    isb

    mrs x0, sctlr_el1
    orr x0, x0, #(1 << 0)
    orr x0, x0, #(1 << 2)
    orr x0, x0, #(1 << 12)
    msr sctlr_el1, x0
    isb

.L_jump_kernel:
    mov x0, x19
    bl kernel_main

secondary_core:
    wfe
    b secondary_core

.section .bss.page_tables, "aw", @nobits
.align 12
page_table_l1:
    .zero 4096
