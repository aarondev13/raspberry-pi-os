#![no_std]
#![no_main]

use core::{
    arch::asm,
    panic::PanicInfo,
    ptr::{read_volatile, write_volatile},
};

// Pi 5 RP1 UART0
// config.txt must contain enable_rp1_uart=1.
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

// GPIO control register offsets
const GPIO14_CTRL: usize = 14 * 8 + 4;
const GPIO15_CTRL: usize = 15 * 8 + 4;

// Send one byte through UART0
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

// Print the kernel startup message
fn uart_boot_marker() {
    for byte in b"RPi5 Rust kernel: booted\r\n" {
        uart_write_byte(*byte);
    }
}

// Configure GPIO 14 and GPIO 15 for UART0
fn configure_uart0_pins() {
    // Configure GPIO 14 (UART0 TX)
    let gpio14_address = RP1_GPIO_BASE + GPIO14_CTRL;

    // Configure GPIO 15 (UART0 RX)
    let gpio15_address = RP1_GPIO_BASE + GPIO15_CTRL;

    unsafe {
        // Read the existing GPIO 14 control register
        let gpio14_current =
            read_volatile(gpio14_address as *const u32);

        // Change only the function-selection bits
        let gpio14_updated =
            (gpio14_current & !GPIO_FUNC_MASK) | GPIO_FUNC_UART0;

        // Write the updated GPIO 14 configuration
        write_volatile(
            gpio14_address as *mut u32,
            gpio14_updated,
        );

        // Read the existing GPIO 15 control register
        let gpio15_current =
            read_volatile(gpio15_address as *const u32);

        // Change only the function-selection bits
        let gpio15_updated =
            (gpio15_current & !GPIO_FUNC_MASK) | GPIO_FUNC_UART0;

        // Write the updated GPIO 15 configuration
        write_volatile(
            gpio15_address as *mut u32,
            gpio15_updated,
        );
    }
}

#[no_mangle]
pub extern "C" fn kernel_main(_device_tree: u64) -> ! {
    // Configure UART pins first
    configure_uart0_pins();

    // Send the startup message
    uart_boot_marker();

    loop {
        unsafe {
            asm!("isb", options(nomem, nostack, preserves_flags));
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
