#![no_std]
#![no_main]

use core::{
    arch::asm,
    panic::PanicInfo,
    ptr::{read_volatile, write_volatile},
};

// Target base address directly matched to your firmware log line: 
// "RP1_UART 0000001c00030000"
const RP1_UART0_BASE: usize = 0x0000_001c_0003_0000;

const UART_DR: usize = 0x00;
const UART_FR: usize = 0x18;
const UART_FR_TXFF: u32 = 1 << 5;
const UART_SPIN_LIMIT: usize = 5_000_000;

// Send one byte through UART0 using the firmware's ready-to-go state
fn uart_write_byte(byte: u8) {
    for _ in 0..UART_SPIN_LIMIT {
        let flags = unsafe {
            read_volatile((RP1_UART0_BASE + UART_FR) as *const u32)
        };

        // Check if Transmit FIFO is full
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

// Print a clean confirmation message
fn uart_boot_marker() {
    for byte in b"\r\n--- RPi5 Rust Bare-Metal Kernel Booted Successfully! ---\r\n" {
        uart_write_byte(*byte);
    }
}

#[no_mangle]
pub extern "C" fn kernel_main(_device_tree: u64) -> ! {
    // Bypassing pin configuration entirely since the firmware already completed it 
    // during the "dtparam: uart0=on" phase.
    
    // Send the startup message
    uart_boot_marker();

    // Heartbeat ticker loop
    loop {
        uart_write_byte(b'.');
        
        // Software delay line
        for _ in 0..10_000_000 {
            core::hint::spin_loop();
        }
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {
        unsafe {
            asm!("wfe", options(nomem, nostack, preserves_flags));
        }
    }
}
