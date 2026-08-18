/*
Framebuffer:
    one frontbuffer
    one backbuffer

The frontbuffer is whatt is currently being shown to the user on screen.
The backbuffer is what the kernel draws to.
Then, when the kernel is done drawing a frame, fb.present should be called.
This replaces everything in the frontbuffer with the content of the backbuffer.
//NOTE: Im currently unsure if present() should also clear the backbuffer.

The font buffer should NEVER be acessed by anything other than this file.
Anyone wanting to present to the screen should only draw to the backbuffer.
Some functions for directly writing to the frontbuffer may be provided,
but marked as unsafe and their use is HEAVILY discouraged.
In many cases it would also be beneficial for programs to have its own backbuffer,
and treat the kernels backbuffer as its frontbuffer, as to not interfere with any other application
that might want to draw to the backbuffer.
*/
use core::ptr;

use limine::request::FramebufferRequest;

#[used]
#[unsafe(link_section = ".limine_reqs")]
static FRAMEBUFFER_REQUEST: FramebufferRequest = FramebufferRequest::new();

const MAX_WIDTH: usize = 1920;
const MAX_HEIGHT: usize = 1080;

const BYTES_PER_PIXEL: usize = 4;

const BACKBUFFER_SIZE: usize = MAX_WIDTH * MAX_HEIGHT * BYTES_PER_PIXEL;

static mut BACKBUFFER: [u8; BACKBUFFER_SIZE] = [0; BACKBUFFER_SIZE];

#[derive(Clone, Copy)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const BLACK: Color = Color { r: 0, g: 0, b: 0 };

    pub const WHITE: Color = Color {
        r: 255,
        g: 255,
        b: 255,
    };

    pub const RED: Color = Color { r: 255, g: 0, b: 0 };

    pub const GREEN: Color = Color { r: 0, g: 255, b: 0 };

    pub const BLUE: Color = Color { r: 0, g: 0, b: 255 };
}

#[derive(Clone, Copy)]
struct Info {
    front: *mut u8,

    width: usize,
    height: usize,

    front_pitch: usize,

    red_shift: u8,
    green_shift: u8,
    blue_shift: u8,
}

unsafe impl Sync for Info {}

static mut INFO: Option<Info> = None;

// ============================================================
// Initialization
// ============================================================

pub fn init() {
    let response = FRAMEBUFFER_REQUEST
        .response()
        .expect("No framebuffer response");

    let framebuffer = response.framebuffers().first().expect("No framebuffer");

    let width = framebuffer.width as usize;
    let height = framebuffer.height as usize;
    let pitch = framebuffer.pitch as usize;
    let bpp = framebuffer.bpp as usize;

    if bpp != 32 {
        panic!("Framebuffer must be 32bpp");
    }

    if width > MAX_WIDTH {
        panic!("Framebuffer width too large");
    }

    if height > MAX_HEIGHT {
        panic!("Framebuffer height too large");
    }

    let info = Info {
        front: framebuffer.address() as *mut u8,

        width,
        height,

        front_pitch: pitch,

        red_shift: framebuffer.red_mask_shift,
        green_shift: framebuffer.green_mask_shift,
        blue_shift: framebuffer.blue_mask_shift,
    };

    unsafe {
        ptr::addr_of_mut!(INFO).write(Some(info));
    }

    clear(Color::BLACK);
}

// ============================================================
// Internal
// ============================================================

#[inline(always)]
fn info() -> Info {
    unsafe {
        ptr::addr_of!(INFO)
            .read()
            .expect("Framebuffer not initialized")
    }
}

// ============================================================
// Color
// ============================================================

#[inline(always)]
fn color_to_u32(color: Color) -> u32 {
    let info = info();

    ((color.r as u32) << info.red_shift)
        | ((color.g as u32) << info.green_shift)
        | ((color.b as u32) << info.blue_shift)
}

// ============================================================
// Draw pixel
// ============================================================

pub fn put_pixel(x: usize, y: usize, color: Color) {
    let info = info();

    if x >= info.width || y >= info.height {
        return;
    }

    let offset = (y * info.width + x) * 4;

    let pixel = color_to_u32(color);

    unsafe {
        let back = ptr::addr_of_mut!(BACKBUFFER) as *mut u8;

        ptr::write_unaligned(back.add(offset) as *mut u32, pixel);
    }
}

// ============================================================
// Clear
// ============================================================

pub fn clear(color: Color) {
    let info = info();

    let pixel = color_to_u32(color);

    unsafe {
        let back = ptr::addr_of_mut!(BACKBUFFER) as *mut u32;

        for i in 0..(info.width * info.height) {
            ptr::write_unaligned(back.add(i), pixel);
        }
    }
}

// ============================================================
// Rectangle
// ============================================================

pub fn draw_rect(x: usize, y: usize, width: usize, height: usize, color: Color) {
    let info = info();

    let end_x = x.saturating_add(width).min(info.width);

    let end_y = y.saturating_add(height).min(info.height);

    for py in y..end_y {
        for px in x..end_x {
            put_pixel(px, py, color);
        }
    }
}

// ============================================================
// Present
// ============================================================

pub fn present() {
    let info = info();

    unsafe {
        let back = ptr::addr_of!(BACKBUFFER) as *const u8;

        let front = info.front;

        let row_size = info.width * BYTES_PER_PIXEL;

        for y in 0..info.height {
            let src = back.add(y * row_size);

            let dst = front.add(y * info.front_pitch);

            ptr::copy_nonoverlapping(src, dst, row_size);
        }
    }
}
