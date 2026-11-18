#![no_std]
#![no_main]

use core::{
    arch::asm,
    panic::PanicInfo,
    ptr::{addr_of_mut, read_volatile, write_volatile},
};

type ByteOpt = Option<u8>;
type FbPtr = *mut u32;

const RP1_BASE: usize = 0x0000_001c_0000_0000;
const RP1_UART0_BASE: usize = RP1_BASE + 0x0003_0000;
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

const MBOX_BASE: usize = 0x0000_0010_7c01_3880;
const MBOX_READ: usize = MBOX_BASE + 0x00;
const MBOX_STATUS: usize = MBOX_BASE + 0x18;
const MBOX_WRITE: usize = MBOX_BASE + 0x20;
const MBOX_FULL: u32 = 0x8000_0000;
const MBOX_EMPTY: u32 = 0x4000_0000;
const MBOX_CH_PROP: u32 = 8;

const REQ_W: u32 = 1920;
const REQ_H: u32 = 1080;

#[repr(C, align(64))]
struct FbMailboxMsg {
    size: u32,
    code: u32,
    phys_tag: u32,
    phys_buf_size: u32,
    phys_req_size: u32,
    phys_w: u32,
    phys_h: u32,
    virt_tag: u32,
    virt_buf_size: u32,
    virt_req_size: u32,
    virt_w: u32,
    virt_h: u32,
    depth_tag: u32,
    depth_buf_size: u32,
    depth_req_size: u32,
    depth: u32,
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
    size: 104,
    code: 0,
    phys_tag: 0x0004_8003,
    phys_buf_size: 8,
    phys_req_size: 8,
    phys_w: REQ_W,
    phys_h: REQ_H,
    virt_tag: 0x0004_8004,
    virt_buf_size: 8,
    virt_req_size: 8,
    virt_w: REQ_W,
    virt_h: REQ_H,
    depth_tag: 0x0004_8005,
    depth_buf_size: 4,
    depth_req_size: 4,
    depth: 32,
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
    fb: FbPtr,
    width: usize,
    height: usize,
    pitch_pixels: usize,
    cursor_x: usize,
    cursor_y: usize,
}

static mut CONSOLE: HdmiConsole = HdmiConsole {
    fb: core::ptr::null_mut(),
    width: REQ_W as usize,
    height: REQ_H as usize,
    pitch_pixels: REQ_W as usize,
    cursor_x: 16,
    cursor_y: 16,
};

const GRID_COLS: usize = 16;
const GRID_ROWS: usize = 16;
const BOX_SIZE: usize = 40;

const PALETTE: [u32; 8] = [
    0x0000_0000,
    0x00FF_FFFF,
    0x00FF_0000,
    0x0000_FF00,
    0x0000_00FF,
    0x00FF_FF00,
    0x0000_FFFF,
    0x00FF_00FF,
];

struct PixelEditor {
    cells: [[usize; GRID_COLS]; GRID_ROWS],
    selected_color: usize,
    grid_x: usize,
    grid_y: usize,
    palette_y: usize,
}

static mut EDITOR: PixelEditor = PixelEditor {
    cells: [[0; GRID_COLS]; GRID_ROWS],
    selected_color: 2,
    grid_x: 640,
    grid_y: 200,
    palette_y: 864,
};

const CURSOR_W: usize = 12;
const CURSOR_H: usize = 16;
const MOUSE_STEP: isize = 12;

const CURSOR_ROWS: [u32; CURSOR_H] = [
    0x00000001,
    0x00000009,
    0x00000029,
    0x000000A9,
    0x000002A9,
    0x00000AA9,
    0x00002AA9,
    0x0000AAA9,
    0x0002AAA9,
    0x000AAAA9,
    0x0002AA99,
    0x0000A6A9,
    0x000291A9,
    0x00029069,
    0x000A4019,
    0x00090001,
];

#[inline(always)]
fn cursor_pixel(row: usize, col: usize) -> u8 {
    ((CURSOR_ROWS[row] >> (col * 2)) & 0x3) as u8
}

struct MouseState {
    x: usize,
    y: usize,
    visible: bool,
    save_under: [[u32; CURSOR_W]; CURSOR_H],
}

static mut MOUSE: MouseState = MouseState {
    x: 660,
    y: 220,
    visible: false,
    save_under: [[0; CURSOR_W]; CURSOR_H],
};

unsafe fn dcache_clean_invalidate_range(start: usize, len: usize) {
    let line_size = 64usize;
    let mut addr = start & !(line_size - 1);
    let end = start + len;
    while addr < end {
        asm!("dc civac, {}", in(reg) addr, options(nostack));
        addr += line_size;
    }
    asm!("dsb sy", options(nostack));
}

unsafe fn dcache_clean_range(start: usize, len: usize) {
    let line_size = 64usize;
    let mut addr = start & !(line_size - 1);
    let end = start + len;
    while addr < end {
        asm!("dc cvac, {}", in(reg) addr, options(nostack));
        addr += line_size;
    }
    asm!("dsb sy", options(nostack));
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

fn hdmi_init() {
    unsafe {
        CONSOLE.fb = 0x3f40_0000 as FbPtr;
        CONSOLE.width = 1920;
        CONSOLE.height = 1080;
        CONSOLE.pitch_pixels = 1920;

        let grid_w = GRID_COLS * BOX_SIZE;
        let grid_h = GRID_ROWS * BOX_SIZE;
        EDITOR.grid_x = (CONSOLE.width - grid_w) / 2;
        EDITOR.grid_y = (CONSOLE.height - grid_h - 80) / 2;
        EDITOR.palette_y = EDITOR.grid_y + grid_h + 24;
        MOUSE.x = EDITOR.grid_x + 20;
        MOUSE.y = EDITOR.grid_y + 20;

        // Clear boot screen to black
        fb_fill_rect(0, 0, CONSOLE.width, CONSOLE.height, 0x0000_0000);
    }
}


        dcache_clean_invalidate_range(msg_addr, msg_len);

        let code = read_volatile(&(*msg_ptr).code);
        let fb_addr = read_volatile(&(*msg_ptr).fb_addr);
        let pitch_bytes = read_volatile(&(*msg_ptr).pitch_bytes);
        let actual_w = read_volatile(&(*msg_ptr).phys_w);
        let actual_h = read_volatile(&(*msg_ptr).phys_h);

        uart_write_uart_only("[HDMI] MBOX code=");
        uart_write_hex32(code);
        uart_write_uart_only(" fb=");
        uart_write_hex32(fb_addr);
        uart_write_uart_only(" pitch=");
        uart_write_hex32(pitch_bytes);
        uart_write_uart_only(" w=");
        uart_write_hex32(actual_w);
        uart_write_uart_only(" h=");
        uart_write_hex32(actual_h);
        uart_write_uart_only("\r\n");

        if code == 0x8000_0000 && fb_addr != 0 {
            let phys_fb = (fb_addr & 0x3FFF_FFFF) as FbPtr;
            CONSOLE.fb = phys_fb;
            if actual_w > 0 {
                CONSOLE.width = actual_w as usize;
            }
            if actual_h > 0 {
                CONSOLE.height = actual_h as usize;
            }
            if pitch_bytes > 0 {
                CONSOLE.pitch_pixels = (pitch_bytes / 4) as usize;
            }

            // Center the 16x16 grid on whatever resolution the monitor negotiated
            let grid_w = GRID_COLS * BOX_SIZE;
            let grid_h = GRID_ROWS * BOX_SIZE;
            EDITOR.grid_x = CONSOLE.width.saturating_sub(grid_w) / 2;
            EDITOR.grid_y = CONSOLE.height.saturating_sub(grid_h + 80) / 2;
            EDITOR.palette_y = EDITOR.grid_y + grid_h + 24;
            MOUSE.x = EDITOR.grid_x + 20;
            MOUSE.y = EDITOR.grid_y + 20;

            // Clear screen to black so any dark-blue boot color is wiped
            fb_fill_rect(0, 0, CONSOLE.width, CONSOLE.height, 0x0000_0000);
        }
    }
}

unsafe fn mouse_hide() {
    if !MOUSE.visible || CONSOLE.fb.is_null() {
        return;
    }
    for row in 0..CURSOR_H {
        let py = MOUSE.y + row;
        if py >= CONSOLE.height {
            break;
        }
        let row_ptr = CONSOLE.fb.add(py * CONSOLE.pitch_pixels + MOUSE.x);
        for col in 0..CURSOR_W {
            if MOUSE.x + col >= CONSOLE.width {
                break;
            }
            if cursor_pixel(row, col) != 0 {
                write_volatile(row_ptr.add(col), MOUSE.save_under[row][col]);
            }
        }
        dcache_clean_range(row_ptr as usize, CURSOR_W * 4);
    }
    MOUSE.visible = false;
}

unsafe fn mouse_show() {
    if MOUSE.visible || CONSOLE.fb.is_null() {
        return;
    }
    for row in 0..CURSOR_H {
        let py = MOUSE.y + row;
        if py >= CONSOLE.height {
            break;
        }
        let row_ptr = CONSOLE.fb.add(py * CONSOLE.pitch_pixels + MOUSE.x);
        for col in 0..CURSOR_W {
            if MOUSE.x + col >= CONSOLE.width {
                break;
            }
            MOUSE.save_under[row][col] = read_volatile(row_ptr.add(col));
            match cursor_pixel(row, col) {
                1 => write_volatile(row_ptr.add(col), 0x0000_0000),
                2 => write_volatile(row_ptr.add(col), 0x00FF_FFFF),
                _ => {}
            }
        }
        dcache_clean_range(row_ptr as usize, CURSOR_W * 4);
    }
    MOUSE.visible = true;
}

unsafe fn mouse_move(dx: isize, dy: isize) {
    mouse_hide();
    let max_x = CONSOLE.width.saturating_sub(CURSOR_W) as isize;
    let max_y = CONSOLE.height.saturating_sub(CURSOR_H) as isize;
    MOUSE.x = (MOUSE.x as isize + dx).clamp(0, max_x) as usize;
    MOUSE.y = (MOUSE.y as isize + dy).clamp(0, max_y) as usize;
    mouse_show();
}

unsafe fn fb_fill_rect(x: usize, y: usize, w: usize, h: usize, color: u32) {
    if CONSOLE.fb.is_null() {
        return;
    }
    for row in 0..h {
        let py = y + row;
        if py >= CONSOLE.height {
            break;
        }
        let row_ptr = CONSOLE.fb.add(py * CONSOLE.pitch_pixels + x);
        for col in 0..w {
            if x + col >= CONSOLE.width {
                break;
            }
            write_volatile(row_ptr.add(col), color);
        }
        dcache_clean_range(row_ptr as usize, w * 4);
    }
}

unsafe fn draw_pixel_box(col: usize, row: usize) {
    let px = EDITOR.grid_x + col * BOX_SIZE;
    let py = EDITOR.grid_y + row * BOX_SIZE;
    let color_idx = EDITOR.cells[row][col];
    let fill_color = PALETTE[color_idx];

    fb_fill_rect(px, py, BOX_SIZE, BOX_SIZE, 0x0033_3333);
    fb_fill_rect(px + 2, py + 2, BOX_SIZE - 4, BOX_SIZE - 4, fill_color);
}
unsafe fn draw_palette_bar() {
for (i, &color) in PALETTE.iter().enumerate() {
let bx = EDITOR.grid_x + i * (BOX_SIZE + 8);
let border = if EDITOR.selected_color == i {
0x00FF_FFFF
} else {
0x0044_4444
};
fb_fill_rect(bx, EDITOR.palette_y, BOX_SIZE, BOX_SIZE, border);
fb_fill_rect(bx + 4, EDITOR.palette_y + 4, BOX_SIZE - 8, BOX_SIZE - 8, color);
}
}
unsafe fn draw_entire_grid() {
mouse_hide();
for r in 0..GRID_ROWS {
for c in 0..GRID_COLS {
draw_pixel_box(c, r);
}
}
draw_palette_bar();
mouse_show();
}
unsafe fn handle_mouse_click() {
let mx = MOUSE.x;
let my = MOUSE.y;
let grid_w = GRID_COLS * BOX_SIZE;
let grid_h = GRID_ROWS * BOX_SIZE;
if mx >= EDITOR.grid_x
&& mx < EDITOR.grid_x + grid_w
&& my >= EDITOR.grid_y
&& my < EDITOR.grid_y + grid_h
{
let col = (mx - EDITOR.grid_x) / BOX_SIZE;
let row = (my - EDITOR.grid_y) / BOX_SIZE;
mouse_hide();
if EDITOR.cells[row][col] == EDITOR.selected_color {
EDITOR.cells[row][col] = 0;
} else {
EDITOR.cells[row][col] = EDITOR.selected_color;
}
draw_pixel_box(col, row);
mouse_show();
return;
}
if my >= EDITOR.palette_y && my < EDITOR.palette_y + BOX_SIZE {
for i in 0..PALETTE.len() {
let bx = EDITOR.grid_x + i * (BOX_SIZE + 8);
if mx >= bx && mx < bx + BOX_SIZE {
mouse_hide();
EDITOR.selected_color = i;
draw_palette_bar();
mouse_show();
return;
}
}
}
}
fn uart_write_hex32(val: u32) {
const HEX: &[u8; 16] = b"0123456789ABCDEF";
uart_write_uart_only("0x");
for shift in (0..8).rev() {
let nibble = ((val >> (shift * 4)) & 0xF) as usize;
uart_write_byte_uart_only(HEX[nibble]);
}
}
unsafe fn export_sprite_to_uart() {
uart_write_uart_only("\r\n// --- Exported 16x16 Sprite ---\r\n");
uart_write_uart_only("const SPRITE: [[u32; 16]; 16] = [\r\n");
for r in 0..GRID_ROWS {
uart_write_uart_only("    [");
for c in 0..GRID_COLS {
let color = PALETTE[EDITOR.cells[r][c]];
uart_write_hex32(color);
if c + 1 < GRID_COLS {
uart_write_uart_only(", ");
}
}
uart_write_uart_only("],\r\n");
}
uart_write_uart_only("];\r\n");
}
unsafe fn editor_handle_input(byte: u8) {
match byte {
b'w' | b'W' | b'A' => mouse_move(0, -MOUSE_STEP),
b's' | b'S' | b'B' => mouse_move(0, MOUSE_STEP),
b'a' | b'D' => mouse_move(-MOUSE_STEP, 0),
b'd' | b'C' => mouse_move(MOUSE_STEP, 0),
b' ' | b'\r' | b'\n' => handle_mouse_click(),
b'1'..=b'8' => {
mouse_hide();
EDITOR.selected_color = (byte - b'1') as usize;
draw_palette_bar();
mouse_show();
}
b'c' => {
EDITOR.cells = [[0; GRID_COLS]; GRID_ROWS];
draw_entire_grid();
}
b'e' | b'E' => export_sprite_to_uart(),
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
if CONSOLE.cursor_y + 16 >= CONSOLE.height {
CONSOLE.cursor_y = 16;
}
return;
}
let glyph = get_glyph_8x8(c);
for (row, &bits) in glyph.iter().enumerate() {
for col in 0..8 {
let color = if (bits >> col) & 1 != 0 {
0x00FF_FFFF
} else {
0x0000_0000
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
let row_ptr = CONSOLE.fb.add((CONSOLE.cursor_y + row * 2) * CONSOLE.pitch_pixels + CONSOLE.cursor_x);
dcache_clean_range(row_ptr as usize, 16 * 4);
let row_ptr2 = CONSOLE.fb.add((CONSOLE.cursor_y + row * 2 + 1) * CONSOLE.pitch_pixels + CONSOLE.cursor_x);
dcache_clean_range(row_ptr2 as usize, 16 * 4);
}
CONSOLE.cursor_x += 16;
if CONSOLE.cursor_x + 16 >= CONSOLE.width {
CONSOLE.cursor_x = 16;
CONSOLE.cursor_y += 16;
if CONSOLE.cursor_y + 16 >= CONSOLE.height {
CONSOLE.cursor_y = 16;
}
}
}
}
fn uart_write_byte_uart_only(byte: u8) {
while unsafe { read_volatile((RP1_UART0_BASE + UART_FR) as *const u32) } & UART_FR_TXFF != 0 {}
unsafe {
write_volatile((RP1_UART0_BASE + UART_DR) as *mut u32, byte as u32);
}
}
fn uart_write_uart_only(s: &str) {
for byte in s.bytes() {
uart_write_byte_uart_only(byte);
}
}
fn uart_write_byte(byte: u8) {
uart_write_byte_uart_only(byte);
hdmi_draw_char(byte);
}
fn uart_write_str(s: &str) {
for byte in s.bytes() {
uart_write_byte(byte);
}
}
fn uart_try_read_byte() -> ByteOpt {
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
uart_write_str("\r\n[BOOT] RP1 UART0 Online at 0x1C_0003_0000!\r\n");
hdmi_init();
uart_write_str("--- RPi5 Pixel Editor + Mouse Ready ---\r\n");
uart_write_str("Move: w/a/s/d | Click: Space | Color: 1-8 | Clear: c | Export: e\r\n");
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
b'+' => [0x00, 0x18, 0x18, 0x7E, 0x18, 0x18, 0x00, 0x00],
b'-' => [0x00, 0x00, 0x00, 0x7E, 0x7E, 0x00, 0x00, 0x00],
b'.' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x18, 0x18],
b'/' => [0x60, 0x30, 0x18, 0x0C, 0x06, 0x00, 0x00, 0x00],
b':' => [0x00, 0x18, 0x18, 0x00, 0x00, 0x18, 0x18, 0x00],
b'[' => [0x3C, 0x0C, 0x0C, 0x0C, 0x0C, 0x0C, 0x3C, 0x00],
b']' => [0x3C, 0x30, 0x30, 0x30, 0x30, 0x30, 0x3C, 0x00],
b'_' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x7E, 0x00],
b'|' => [0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x00],
b'0' => [0x3C, 0x66, 0x76, 0x7E, 0x6E, 0x66, 0x3C, 0x00],
b'1' => [0x18, 0x1C, 0x18, 0x18, 0x18, 0x18, 0x7E, 0x00],
b'3' => [0x3C, 0x66, 0x60, 0x38, 0x60, 0x66, 0x3C, 0x00],
b'5' => [0x7E, 0x06, 0x3E, 0x60, 0x60, 0x66, 0x3C, 0x00],
b'8' => [0x3C, 0x66, 0x66, 0x3C, 0x66, 0x66, 0x3C, 0x00],
b'B' => [0x3E, 0x66, 0x66, 0x3E, 0x66, 0x66, 0x3E, 0x00],
b'C' => [0x3C, 0x66, 0x06, 0x06, 0x06, 0x66, 0x3C, 0x00],
b'D' => [0x1E, 0x36, 0x66, 0x66, 0x66, 0x36, 0x1E, 0x00],
b'E' => [0x7E, 0x06, 0x06, 0x3E, 0x06, 0x06, 0x7E, 0x00],
b'F' => [0x7E, 0x06, 0x06, 0x3E, 0x06, 0x06, 0x06, 0x00],
b'I' => [0x3C, 0x18, 0x18, 0x18, 0x18, 0x18, 0x3C, 0x00],
b'K' => [0x66, 0x36, 0x1E, 0x0E, 0x1E, 0x36, 0x66, 0x00],
b'L' => [0x06, 0x06, 0x06, 0x06, 0x06, 0x06, 0x7E, 0x00],
b'M' => [0x63, 0x77, 0x7F, 0x6B, 0x63, 0x63, 0x63, 0x00],
b'N' => [0x66, 0x6E, 0x7E, 0x76, 0x66, 0x66, 0x66, 0x00],
b'O' => [0x3C, 0x66, 0x66, 0x66, 0x66, 0x66, 0x3C, 0x00],
b'P' => [0x3E, 0x66, 0x66, 0x3E, 0x06, 0x06, 0x06, 0x00],
b'R' => [0x3E, 0x66, 0x66, 0x3E, 0x1E, 0x36, 0x66, 0x00],
b'S' => [0x3C, 0x66, 0x06, 0x3C, 0x60, 0x66, 0x3C, 0x00],
b'T' => [0x7E, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x00],
b'U' => [0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x3C, 0x00],
b'a' => [0x00, 0x00, 0x3C, 0x60, 0x7C, 0x66, 0x7C, 0x00],
b'c' => [0x00, 0x00, 0x3C, 0x06, 0x06, 0x06, 0x3C, 0x00],
b'd' => [0x60, 0x60, 0x7C, 0x66, 0x66, 0x66, 0x7C, 0x00],
b'e' => [0x00, 0x00, 0x3C, 0x66, 0x7E, 0x06, 0x3C, 0x00],
b'f' => [0x38, 0x6C, 0x0C, 0x3E, 0x0C, 0x0C, 0x0C, 0x00],
b'i' => [0x18, 0x00, 0x1C, 0x18, 0x18, 0x18, 0x3C, 0x00],
b'k' => [0x06, 0x06, 0x36, 0x1E, 0x0E, 0x1E, 0x36, 0x00],
b'l' => [0x1C, 0x18, 0x18, 0x18, 0x18, 0x18, 0x3C, 0x00],
b'm' => [0x00, 0x00, 0x36, 0x7F, 0x6B, 0x6B, 0x63, 0x00],
b'n' => [0x00, 0x00, 0x3E, 0x66, 0x66, 0x66, 0x66, 0x00],
b'o' => [0x00, 0x00, 0x3C, 0x66, 0x66, 0x66, 0x3C, 0x00],
b'p' => [0x00, 0x00, 0x3C, 0x66, 0x66, 0x3E, 0x06, 0x06],
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
