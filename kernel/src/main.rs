#![no_std]
#![no_main]

use core::{
    arch::asm,
    panic::PanicInfo,
    ptr::{read_volatile, write_volatile},
};

const RP1_UART0_BASE: usize = 0x0000_001f_0003_0000;
const UART_DR: usize = 0x00;
const UART_FR: usize = 0x18;
const UART_FR_TXFF: u32 = 1 << 5;
const UART_SPIN_LIMIT: usize = 5_000_000;

fn uart_write_byte(byte: u8) {
    for _ in 0..UART_SPIN_LIMIT {
        let flags = unsafe {
            read_volatile((RP1_UART0_BASE + UART_FR) as *const u32)
        };

        if flags & UART_FR_TXFF == 0 {
            unsafe {
                write_volatile(
                    (RP1_UART0_BASE + UART_DR) as *mut u32,
                    byte as u32,
                );
            }
            return;
        }
    }
}

fn uart_write_str(s: &str) {
    for byte in s.bytes() {
        uart_write_byte(byte);
    }
}

#[no_mangle]
pub extern "C" fn kernel_main(_device_tree: u64) -> ! {
    uart_write_str("\r\n--- RPi5 Rust Kernel: Execution Confirmed! ---\r\n");

    loop {
        uart_write_byte(b'.');
        for _ in 0..15_000_000 {
            core::hint::spin_loop();
        }
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    uart_write_str("\r\n!!! KERNEL PANIC OCCURRED !!!\r\n");
    loop {
        unsafe {
            asm!("wfe", options(nomem, nostack, preserves_flags));
        }
    }
}
