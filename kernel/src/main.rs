#![no_std]
#![no_main]

use core::{
    arch::asm,
    panic::PanicInfo,
    ptr::{read_volatile, write_volatile},
};

// Target base address configuration matching the hardware log layout
const RP1_UART0_BASE: usize = 0x0000_001c_0003_0000;

const UART_DR: usize = 0x00;        // Data Register Offset
const UART_FR: usize = 0x18;        // Flag Register Offset
const UART_FR_TXFF: u32 = 1 << 5;   // Transmit FIFO Full flag bit
const UART_SPIN_LIMIT: usize = 5_000_000;

/// Low-level function to pass a single byte straight into the active PL011 line
fn uart_write_byte(byte: u8) {
    for _ in 0..UART_SPIN_LIMIT {
        let flags = unsafe {
            read_volatile((RP1_UART0_BASE + UART_FR) as *const u32)
        };

        // If the Transmit FIFO Full bit is 0, there is space to write
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

/// Helper function to write fixed string references out across the system bus
fn uart_write_str(s: &str) {
    for byte in s.bytes() {
        uart_write_byte(byte);
    }
}

#[no_mangle]
pub extern "C" fn kernel_main(_device_tree: u64) -> ! {
    // Send immediate confirmation down the serial interface
    uart_write_str("\r\n--- RPi5 Rust Kernel: Execution Confirmed! ---\r\n");

    loop {
        // Continuous heartbeat logging indicator
        uart_write_byte(b'.');
        
        // Software baseline delay window
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
