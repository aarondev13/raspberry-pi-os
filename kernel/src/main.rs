#![no_std]
#![no_main]

use core::{
    arch::asm,
    panic::PanicInfo,
    ptr::{read_volatile, write_volatile},
};

// Pi 5 RP1 UART0 as mapped by firmware during the early-boot handoff.
// config.txt must contain enable_rp1_uart=1 before this address is used.
const RP1_UART0_BASE: usize = 0x0000_001f_0003_0000;
const UART_DR: usize = 0x00;
const UART_FR: usize = 0x18;
const UART_FR_TXFF: u32 = 1 << 5;
const UART_SPIN_LIMIT: usize = 1_000_000;
// RP1 GPIO controller base address
const RP1_GPIO_BASE: usize = 0x0000_001f_000d_0000;

// GPIO function selection
const GPIO_FUNC_MASK: u32 = 0x1f;
const GPIO_FUNC_UART0: u32 = 4;

fn uart_write_byte(byte: u8) {
    for _ in 0..UART_SPIN_LIMIT {
        let flags = unsafe { read_volatile((RP1_UART0_BASE + UART_FR) as *const u32) };
        if flags & UART_FR_TXFF == 0 {
            unsafe { write_volatile((RP1_UART0_BASE + UART_DR) as *mut u32, byte as u32) };
            return;
        }
    }
}

fn uart_boot_marker() {
    for byte in b"RPi5 Rust kernel: booted\r\n" {
        uart_write_byte(*byte);
    }
}

#[no_mangle]
pub extern "C" fn kernel_main(_device_tree: u64) -> ! {
    uart_boot_marker();

    loop {
        unsafe { asm!("isb", options(nomem, nostack, preserves_flags)) };
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {
        unsafe { asm!("wfe", options(nomem, nostack, preserves_flags)) };
    }
}
