.section .text.boot
.global _start
.extern kernel_main

_start:
    mov x19, x0
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



#[no_mangle]
pub extern "C" fn kernel_main(_dtb: usize) -> ! {
    uart_init();

    uart_puts("kernel_main reached\r\n");

    loop {
        uart_puts(".");
        delay();
    }
}
