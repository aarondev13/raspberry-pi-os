#![no_std]
#![no_main]

use core::{
    arch::asm,
    panic::PanicInfo,
    ptr::{read_volatile, write_volatile},
};

const RP1_UART0_BASE: usize = 0x0000_001c_0003_0000;
const UART_DR: usize = 0x00;
const UART_FR: usize = 0x18;
const UART_LCRH: usize = 0x2c;
const UART_CR: usize = 0x30;

const UART_FR_TXFF: u32 = 1 << 5;
const UART_LCRH_WLEN_8BIT: u32 = 3 << 5;
const UART_CR_UARTEN: u32 = 1 << 0;
const UART_CR_TXE: u32 = 1 << 8;
const UART_CR_RXE: u32 = 1 << 9;

fn uart_init() {
    unsafe {
        // Disable UART before configuring
        write_volatile((RP1_UART0_BASE + UART_CR) as *mut u32, 0);
        // Set 8-bit word length (keep firmware's 115200 baud IBRD/FBRD untouched)
        write_volatile((RP1_UART0_BASE + UART_LCRH) as *mut u32, UART_LCRH_WLEN_8BIT);
        // Re-enable UART, TX, and RX
        write_volatile(
            (RP1_UART0_BASE + UART_CR) as *mut u32,
            UART_CR_UARTEN | UART_CR_TXE | UART_CR_RXE,
        );
    }
}

fn uart_write_byte(byte: u8) {
    while unsafe { read_volatile((RP1_UART0_BASE + UART_FR) as *const u32) } & UART_FR_TXFF != 0 {}
    unsafe {
        write_volatile((RP1_UART0_BASE + UART_DR) as *mut u32, byte as u32);
    }
}

fn uart_write_str(s: &str) {
    for byte in s.bytes() {
        uart_write_byte(byte);
    }
}

#[no_mangle]
pub extern "C" fn kernel_main(_device_tree: u64) -> ! {
    uart_init();
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
