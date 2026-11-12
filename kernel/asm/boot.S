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
    // Position-independent stack pointer (works at 0x80000 AND 0x40080000)
    adrp x0, __stack_top
    add  x0, x0, :lo12:__stack_top
    mov  sp, x0

    // Position-independent .bss zeroing
    adrp x0, __bss_start
    add  x0, x0, :lo12:__bss_start
    adrp x1, __bss_end
    add  x1, x1, :lo12:__bss_end
clear_bss:
    cmp x0, x1
    b.hs bss_done
    str xzr, [x0], #8
    b clear_bss

bss_done:
    adrp x0, page_table_l1
    add  x0, x0, :lo12:page_table_l1

    // 0x00000000 - 0x3FFFFFFF: Device-nGnRE (0x401) for QEMU UART (0x09000000) & Pi low MMIO
    // NOTE: On Pi 5, kernel RAM at 0x80000 is also here; 0x745 makes it Normal Cacheable
    ldr x1, =0x00000000
    ldr x2, =0x00000745
    orr x1, x1, x2
    str x1, [x0, #0]

    // 0x40000000 - 0x7FFFFFFF: Normal Cacheable RAM (0x745) for QEMU virt RAM & DTB
    ldr x1, =0x40000000
    ldr x2, =0x00000745
    orr x1, x1, x2
    str x1, [x0, #8]

    // 0x80000000 - 0xBFFFFFFF: Normal Cacheable RAM (0x745)
    ldr x1, =0x80000000
    ldr x2, =0x00000745
    orr x1, x1, x2
    str x1, [x0, #16]

    // 0xC0000000 - 0xFFFFFFFF: Device-nGnRE (0x401)
    ldr x1, =0xC0000000
    ldr x2, =0x00000401
    orr x1, x1, x2
    str x1, [x0, #24]

    // 0x1000000000 (64GB, entry 64 * 8 = #512): Pi 5 SoC Peripherals & Debug UART (0x107D001000)
    ldr x1, =0x1000000000
    ldr x2, =0x00000401
    orr x1, x1, x2
    str x1, [x0, #512]

    // 0x1C00000000 (112GB, entry 112 * 8 = #896): Pi 5 RP1 PCIe Peripherals & GPIO UART (0x1F00030000)
    // NOTE: 112 * 8 = 896 (not 224! 224 was 28 * 8 = 0x0700000000!)
    ldr x1, =0x1C00000000
    ldr x2, =0x00000401
    orr x1, x1, x2
    str x1, [x0, #896]

    // Also map 0x1F00000000 (124GB, entry 124 * 8 = #992) directly for RP1 (0x1F00030000)!
    ldr x1, =0x1F00000000
    ldr x2, =0x00000401
    orr x1, x1, x2
    str x1, [x0, #992]

    mrs x1, CurrentEL
    lsr x1, x1, #2
    and x1, x1, #0x3
    cmp x1, #2
    b.ne .L_mmu_el1

    msr ttbr0_el2, x0

    ldr x1, =0x0000ff04
    msr mair_el2, x1

    // T0SZ = 24 (0x18 -> 40-bit VA, covers up to 1TB!), PS = 0b010 (40-bit PA)
    ldr x1, =0x80822518
    msr tcr_el2, x1
    dsb ish
    isb

    mrs x1, sctlr_el2
    orr x1, x1, #(1 << 0)
    orr x1, x1, #(1 << 2)
    orr x1, x1, #(1 << 12)
    msr sctlr_el2, x1
    isb
    b .L_jump_kernel

.L_mmu_el1:
    msr ttbr0_el1, x0

    ldr x1, =0x0000ff04
    msr mair_el1, x1

    // T0SZ = 24 (0x18 -> 40-bit VA, covers up to 1TB!), IPS = 0b010 (40-bit PA)
    ldr x1, =0x200802518
    msr tcr_el1, x1
    dsb ish
    isb

    mrs x1, sctlr_el1
    orr x1, x1, #(1 << 0)
    orr x1, x1, #(1 << 2)
    orr x1, x1, #(1 << 12)
    msr sctlr_el1, x1
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
