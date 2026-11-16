#![no_std]
#![no_main]

use core::{
    arch::asm,
    panic::PanicInfo,
    ptr::{addr_of_mut, read_volatile, write_volatile},
};

// --- RP1 UART0 Constants (Mapped by your boot.S at 0x1C_0000_0000) ---
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

// --- VideoCore Mailbox Constants (Mapped by your boot.S at 0x10_0000_0000) ---
const MBOX_BASE: usize = 0x0000_0010_7c01_3880;
const MBOX_READ: usize = MBOX_BASE + 0x00;
const MBOX_STATUS: usize = MBOX_BASE + 0x18;
const MBOX_WRITE: usize = MBOX_BASE + 0x20;

const MBOX_FULL: u32 = 0x8000_0000;
const MBOX_EMPTY: u32 = 0x4000_0000;
const MBOX_CH_PROP: u32 = 8;

const SCREEN_W: usize = 1920;
const SCREEN_H: usize = 1080;

// --- Static 16-byte Aligned Mailbox Buffer (< 4GB RAM) ---
#[repr(C, align(16))]
struct FbMailboxMsg {
    size: u32,
    code: u32,
    alloc_tag: u32,
    alloc_buf_size: u32,
    alloc_req_size: u32,
    fb_addr: u32,
    fb_size: u32,
    pitch_tag: u32,
    pitch_buf_size: u32,
    pitch_req_size: u32,
    pitch_bytes: u32,
    end_tag: u32,
}

static mut MBOX_BUF: FbMailboxMsg = FbMailboxMsg {
    size: 48,
    code: 0,
    alloc_tag: 0x0004_0001,
    alloc_buf_size: 8,
    alloc_req_size: 4,
    fb_addr: 16,
    fb_size: 0,
    pitch_tag: 0x0004_0008,
    pitch_buf_size: 4,
    pitch_req_size: 0,
    pitch_bytes: 0,
    end_tag: 0,
};

struct HdmiConsole {
    fb: *mut u32,
    pitch_pixels: usize,
    cursor_x: usize,
    cursor_y: usize,
}

static mut CONSOLE: HdmiConsole = HdmiConsole {
    fb: core::ptr::null_mut(),
    pitch_pixels: SCREEN_W,
    cursor_x: 16,
    cursor_y: 16,
};

fn hdmi_init() {
    let msg_ptr = addr_of_mut!(MBOX_BUF);
    let msg_addr = msg_ptr as usize;

    unsafe {
        // Flush D-Cache (enabled in your boot.S) so GPU reads valid request
        asm!("dc civac, {}", in(reg) msg_addr);
        asm!("dsb sy");

        while (read_volatile(MBOX_STATUS as *const u32) & MBOX_FULL) != 0 {}
        let packed = ((msg_addr as u32) & !0xF) | MBOX_CH_PROP;
        write_volatile(MBOX_WRITE as *mut u32, packed);

        loop {
            while (read_volatile(MBOX_STATUS as *const u32) & MBOX_EMPTY) != 0 {}
            if read_volatile(MBOX_READ as *const u32) == packed {
                break;
            }
        }

        // Invalidate D-Cache so CPU reads GPU's framebuffer address from DRAM
        asm!("dc civac, {}", in(reg) msg_addr);
        asm!("dsb sy");

        let code = read_volatile(&(*msg_ptr).code);
        let fb_addr = read_volatile(&(*msg_ptr).fb_addr);
        let pitch_bytes = read_volatile(&(*msg_ptr).pitch_bytes);

        if code == 0x8000_0000 && fb_addr != 0 {
            let phys_fb = (fb_addr & 0x3FFF_FFFF) as *mut u32;
            CONSOLE.fb = phys_fb;
            if pitch_bytes > 0 {
                CONSOLE.pitch_pixels = (pitch_bytes / 4) as usize;
            }
        }
    }
}

fn hdmi_draw_char(c: u8) {
    unsafe {
        if CONSOLE.fb.is_null() {
            return;
        }
        if c == b'\r' {
            CONSOLE.cursor_x = 16;
            return;
        }
        if c == b'\n' {
            CONSOLE.cursor_x = 16;
            CONSOLE.cursor_y += 16;
            if CONSOLE.cursor_y + 16 >= SCREEN_H {
                CONSOLE.cursor_y = 16;
            }
            return;
        }

        let glyph = get_glyph_8x8(c);
        for (row, &bits) in glyph.iter().enumerate() {
            for col in 0..8 {
                let color = if (bits >> col) & 1 != 0 {
                    0x00FF_FFFF // White
                } else {
                    0x0000_0000 // Black
                };

                for dy in 0..2 {
                    for dx in 0..2 {
                        let px = CONSOLE.cursor_x + (col * 2) + dx;
                        let py = CONSOLE.cursor_y + (row * 2) + dy;
                        let offset = py * CONSOLE.pitch_pixels + px;
                        write_volatile(CONSOLE.fb.add(offset), color);
                    }
                }
            }
        }

        CONSOLE.cursor_x += 16;
        if CONSOLE.cursor_x + 16 >= SCREEN_W {
            CONSOLE.cursor_x = 16;
            CONSOLE.cursor_y += 16;
            if CONSOLE.cursor_y + 16 >= SCREEN_H {
                CONSOLE.cursor_y = 16;
            }
        }
    }
}

fn uart_init() {
    unsafe {
        write_volatile((RP1_UART0_BASE + UART_CR) as *mut u32, 0);
        write_volatile((RP1_UART0_BASE + UART_LCRH) as *mut u32, UART_LCRH_WLEN_8BIT);
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
    hdmi_draw_char(byte);
}

fn uart_write_str(s: &str) {
    for byte in s.bytes() {
        uart_write_byte(byte);
    }
}

#[no_mangle]
pub extern "C" fn kernel_main(_device_tree: u64) -> ! {
    uart_init();
    hdmi_init();
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

fn get_glyph_8x8(c: u8) -> [u8; 8] {
    match c {
        b' ' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
        b'!' => [0x18, 0x3C, 0x3C, 0x18, 0x18, 0x00, 0x18, 0x00],
        b'-' => [0x00, 0x00, 0x00, 0x7E, 0x7E, 0x00, 0x00, 0x00],
        b'.' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x18, 0x18],
        b':' => [0x00, 0x18, 0x18, 0x00, 0x00, 0x18, 0x18, 0x00],
        b'5' => [0x7E, 0x06, 0x3E, 0x60, 0x60, 0x66, 0x3C, 0x00],
        b'C' => [0x3C, 0x66, 0x06, 0x06, 0x06, 0x66, 0x3C, 0x00],
        b'D' => [0x1E, 0x36, 0x66, 0x66, 0x66, 0x36, 0x1E, 0x00],
        b'E' => [0x7E, 0x06, 0x06, 0x3E, 0x06, 0x06, 0x7E, 0x00],
        b'I' => [0x3C, 0x18, 0x18, 0x18, 0x18, 0x18, 0x3C, 0x00],
        b'K' => [0x66, 0x36, 0x1E, 0x0E, 0x1E, 0x36, 0x66, 0x00],
        b'L' => [0x06, 0x06, 0x06, 0x06, 0x06, 0x06, 0x7E, 0x00],
        b'N' => [0x66, 0x6E, 0x7E, 0x76, 0x66, 0x66, 0x66, 0x00],
        b'O' => [0x3C, 0x66, 0x66, 0x66, 0x66, 0x66, 0x3C, 0x00],
        b'P' => [0x3E, 0x66, 0x66, 0x3E, 0x06, 0x06, 0x06, 0x00],
        b'R' => [0x3E, 0x66, 0x66, 0x3E, 0x1E, 0x36, 0x66, 0x00],
        b'U' => [0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x3C, 0x00],
        b'c' => [0x00, 0x00, 0x3C, 0x06, 0x06, 0x06, 0x3C, 0x00],
        b'd' => [0x60, 0x60, 0x7C, 0x66, 0x66, 0x66, 0x7C, 0x00],
        b'e' => [0x00, 0x00, 0x3C, 0x66, 0x7E, 0x06, 0x3C, 0x00],
        b'f' => [0x38, 0x6C, 0x0C, 0x3E, 0x0C, 0x0C, 0x0C, 0x00],
        b'i' => [0x18, 0x00, 0x1C, 0x18, 0x18, 0x18, 0x3C, 0x00],
        b'l' => [0x1C, 0x18, 0x18, 0x18, 0x18, 0x18, 0x3C, 0x00],
        b'm' => [0x00, 0x00, 0x36, 0x7F, 0x6B, 0x6B, 0x63, 0x00],
        b'n' => [0x00, 0x00, 0x3E, 0x66, 0x66, 0x66, 0x66, 0x00],
        b'o' => [0x00, 0x00, 0x3C, 0x66, 0x66, 0x66, 0x3C, 0x00],
        b'r' => [0x00, 0x00, 0x3E, 0x66, 0x06, 0x06, 0x06, 0x00],
        b's' => [0x00, 0x00, 0x3C, 0x06, 0x3C, 0x60, 0x3E, 0x00],
        b't' => [0x0C, 0x0C, 0x3E, 0x0C, 0x0C, 0x6C, 0x38, 0x00],
        b'u' => [0x00, 0x00, 0x66, 0x66, 0x66, 0x66, 0x7C, 0x00],
        b'x' => [0x00, 0x00, 0x66, 0x3C, 0x18, 0x3C, 0x66, 0x00],
        _ => [0x7E, 0x42, 0x42, 0x42, 0x42, 0x42, 0x7E, 0x00],
    }
}
