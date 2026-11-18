#![no_std]
#![no_main]

use core::{
    arch::asm,
    panic::PanicInfo,
    ptr::{addr_of_mut, read_volatile, write_volatile},
};

// --- RP1 UART0 Constants (Mapped by boot.S at 0x1C_0000_0000) ---
const RP1_UART0_BASE: usize = 0x0000_001c_0003_0000;
const UART_DR: usize = 0x00;
const UART_FR: usize = 0x18;
const UART_LCRH: usize = 0x2c;
const UART_CR: usize = 0x30;

const UART_FR_RXFE: u32 = 1 << 4;
const UART_FR_TXFF: u32 = 1 << 5;
const UART_LCRH_WLEN_8BIT: u32 = 3 << 5;
const UART_CR_UARTEN: u32 = 1 << 0;
const UART_CR_TXE: u32 = 1 << 8;
const UART_CR_RXE: u32 = 1 << 9;

// --- VideoCore Mailbox Constants (Mapped by boot.S at 0x10_0000_0000) ---
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

// --- Pixel Editor Constants & State ---
const GRID_COLS: usize = 16;
const GRID_ROWS: usize = 16;
const BOX_SIZE: usize = 40;
const GRID_OFFSET_X: usize = 640;
const GRID_OFFSET_Y: usize = 200;

const PALETTE: [u32; 8] = [
    0x0000_0000, // 1: Black (Eraser)
    0x00FF_FFFF, // 2: White
    0x00FF_0000, // 3: Red
    0x0000_FF00, // 4: Green
    0x0000_00FF, // 5: Blue
    0x00FF_FF00, // 6: Yellow
    0x0000_FFFF, // 7: Cyan
    0x00FF_00FF, // 8: Magenta
];

struct PixelEditor {
    cells: [[usize; GRID_COLS]; GRID_ROWS],
    cursor_col: usize,
    cursor_row: usize,
    selected_color: usize,
}

static mut EDITOR: PixelEditor = PixelEditor {
    cells: [[0; GRID_COLS]; GRID_ROWS],
    cursor_col: 0,
    cursor_row: 0,
    selected_color: 2, // Start with Red selected
};

fn hdmi_init() {
    let msg_ptr = addr_of_mut!(MBOX_BUF);
    let msg_addr = msg_ptr as usize;

    unsafe {
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

unsafe fn fb_fill_rect(x: usize, y: usize, w: usize, h: usize, color: u32) {
    if CONSOLE.fb.is_null() {
        return;
    }
    for row in 0..h {
        let py = y + row;
        if py >= SCREEN_H {
            break;
        }
        let row_ptr = CONSOLE.fb.add(py * CONSOLE.pitch_pixels + x);
        for col in 0..w {
            if x + col >= SCREEN_W {
                break;
            }
            write_volatile(row_ptr.add(col), color);
        }
    }
}

unsafe fn draw_pixel_box(col: usize, row: usize) {
    let px = GRID_OFFSET_X + col * BOX_SIZE;
    let py = GRID_OFFSET_Y + row * BOX_SIZE;
    let color_idx = EDITOR.cells[row][col];
    let fill_color = PALETTE[color_idx];

    // Grid cell border and inner pixel fill
    fb_fill_rect(px, py, BOX_SIZE, BOX_SIZE, 0x0033_3333);
    fb_fill_rect(px + 2, py + 2, BOX_SIZE - 4, BOX_SIZE - 4, fill_color);

    // Draw cursor highlight frame if active
    if EDITOR.cursor_col == col && EDITOR.cursor_row == row {
        let highlight = PALETTE[EDITOR.selected_color];
        fb_fill_rect(px + 2, py + 2, BOX_SIZE - 4, 3, highlight);
        fb_fill_rect(px + 2, py + BOX_SIZE - 5, BOX_SIZE - 4, 3, highlight);
        fb_fill_rect(px + 2, py + 2, 3, BOX_SIZE - 4, highlight);
        fb_fill_rect(px + BOX_SIZE - 5, py + 2, 3, BOX_SIZE - 4, highlight);
    }
}

unsafe fn draw_palette_bar() {
    let bar_y = GRID_OFFSET_Y + GRID_ROWS * BOX_SIZE + 24;
    for (i, &color) in PALETTE.iter().enumerate() {
        let bx = GRID_OFFSET_X + i * (BOX_SIZE + 8);
        let border = if EDITOR.selected_color == i {
            0x00FF_FFFF
        } else {
            0x0044_4444
        };
        fb_fill_rect(bx, bar_y, BOX_SIZE, BOX_SIZE, border);
        fb_fill_rect(bx + 4, bar_y + 4, BOX_SIZE - 8, BOX_SIZE - 8, color);
    }
}

unsafe fn draw_entire_grid() {
    for r in 0..GRID_ROWS {
        for c in 0..GRID_COLS {
            draw_pixel_box(c, r);
        }
    }
    draw_palette_bar();
}

unsafe fn editor_move_cursor(dcol: isize, drow: isize) {
    let old_col = EDITOR.cursor_col;
    let old_row = EDITOR.cursor_row;

    let new_col = (old_col as isize + dcol).clamp(0, (GRID_COLS - 1) as isize) as usize;
    let new_row = (old_row as isize + drow).clamp(0, (GRID_ROWS - 1) as isize) as usize;

    EDITOR.cursor_col = new_col;
    EDITOR.cursor_row = new_row;

    draw_pixel_box(old_col, old_row);
    draw_pixel_box(new_col, new_row);
}

unsafe fn editor_click() {
    let col = EDITOR.cursor_col;
    let row = EDITOR.cursor_row;
    if EDITOR.cells[row][col] == EDITOR.selected_color {
        EDITOR.cells[row][col] = 0;
    } else {
        EDITOR.cells[row][col] = EDITOR.selected_color;
    }
    draw_pixel_box(col, row);
}

unsafe fn editor_handle_input(byte: u8) {
    match byte {
        b'w' | b'W' | b'A' => editor_move_cursor(0, -1), // Up (W or Up Arrow ESC [ A)
        b's' | b'S' | b'B' => editor_move_cursor(0, 1),  // Down (S or Down Arrow ESC [ B)
        b'a' | b'D' => editor_move_cursor(-1, 0),        // Left (a or Left Arrow ESC [ D)
        b'd' | b'C' => editor_move_cursor(1, 0),         // Right (d or Right Arrow ESC [ C)
        b' ' | b'\r' | b'\n' => editor_click(),          // Paint / Erase pixel
        b'1'..=b'8' => {
            EDITOR.selected_color = (byte - b'1') as usize;
            draw_palette_bar();
            draw_pixel_box(EDITOR.cursor_col, EDITOR.cursor_row);
        }
        b'c' => {
            EDITOR.cells = [[0; GRID_COLS]; GRID_ROWS];
            draw_entire_grid();
        }
        _ => {}
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

fn uart_try_read_byte() -> Option<u8> {
    unsafe {
        if (read_volatile((RP1_UART0_BASE + UART_FR) as *const u32) & UART_FR_RXFE) != 0 {
            None
        } else {
            Some((read_volatile((RP1_UART0_BASE + UART_DR) as *const u32) & 0xFF) as u8)
        }
    }
}

#[no_mangle]
pub extern "C" fn kernel_main(_device_tree: u64) -> ! {
    uart_init();
    hdmi_init();
    uart_write_str("\r\n--- RPi5 Pixel Editor Ready ---\r\n");
    uart_write_str("Move: w/a/s/d | Paint: Space | Color: 1-8 | Clear: c\r\n");

    unsafe {
        draw_entire_grid();
    }

    loop {
        if let Some(byte) = uart_try_read_byte() {
            unsafe {
                editor_handle_input(byte);
            }
        }
        core::hint::spin_loop();
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
        b'/' => [0x60, 0x30, 0x18, 0x0C, 0x06, 0x00, 0x00, 0x00],
        b':' => [0x00, 0x18, 0x18, 0x00, 0x00, 0x18, 0x18, 0x00],
        b'|' => [0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x00],
        b'1' => [0x18, 0x1C, 0x18, 0x18, 0x18, 0x18, 0x7E, 0x00],
        b'5' => [0x7E, 0x06, 0x3E, 0x60, 0x60, 0x66, 0x3C, 0x00],
        b'8' => [0x3C, 0x66, 0x66, 0x3C, 0x66, 0x66, 0x3C, 0x00],
        b'C' => [0x3C, 0x66, 0x06, 0x06, 0x06, 0x66, 0x3C, 0x00],
        b'D' => [0x1E, 0x36, 0x66, 0x66, 0x66, 0x36, 0x1E, 0x00],
        b'E' => [0x7E, 0x06, 0x06, 0x3E, 0x06, 0x06, 0x7E, 0x00],
        b'I' => [0x3C, 0x18, 0x18, 0x18, 0x18, 0x18, 0x3C, 0x00],
        b'K' => [0x66, 0x36, 0x1E, 0x0E, 0x1E, 0x36, 0x66, 0x00],
        b'L' => [0x06, 0x06, 0x06, 0x06, 0x06, 0x06, 0x7E, 0x00],
        b'M' => [0x63, 0x77, 0x7F, 0x6B, 0x63, 0x63, 0x63, 0x00],
        b'N' => [0x66, 0x6E, 0x7E, 0x76, 0x66, 0x66, 0x66, 0x00],
        b'O' => [0x3C, 0x66, 0x66, 0x66, 0x66, 0x66, 0x3C, 0x00],
        b'P' => [0x3E, 0x66, 0x66, 0x3E, 0x06, 0x06, 0x06, 0x00],
        b'R' => [0x3E, 0x66, 0x66, 0x3E, 0x1E, 0x36, 0x66, 0x00],
        b'S' => [0x3C, 0x66, 0x06, 0x3C, 0x60, 0x66, 0x3C, 0x00],
        b'U' => [0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x3C, 0x00],
        b'a' => [0x00, 0x00, 0x3C, 0x60, 0x7C, 0x66, 0x7C, 0x00],
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
        b'v' => [0x00, 0x00, 0x66, 0x66, 0x66, 0x3C, 0x18, 0x00],
        b'w' => [0x00, 0x00, 0x63, 0x6B, 0x6B, 0x7F, 0x36, 0x00],
        b'x' => [0x00, 0x00, 0x66, 0x3C, 0x18, 0x3C, 0x66, 0x00],
        b'y' => [0x00, 0x00, 0x66, 0x66, 0x7C, 0x60, 0x3C, 0x00],
        _ => [0x7E, 0x42, 0x42, 0x42, 0x42, 0x42, 0x7E, 0x00],
    }
}
