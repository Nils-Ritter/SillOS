/*
Framebuffer:
    one frontbuffer
    one backbuffer

The frontbuffer is what is currently shown to the user.

The backbuffer is what the kernel draws to.

When the kernel is done drawing, fb::present() should be called.

present() copies only the region of the backbuffer that has
changed.

No heap allocation is used by this framebuffer.
*/

use core::ptr;

use limine::request::FramebufferRequest;

#[used]
#[unsafe(link_section = ".limine_reqs")]
static FRAMEBUFFER_REQUEST: FramebufferRequest =
    FramebufferRequest::new();

const MAX_WIDTH: usize = 1920;
const MAX_HEIGHT: usize = 1080;

const BYTES_PER_PIXEL: usize = 4;
const MAX_PIXELS: usize = MAX_WIDTH * MAX_HEIGHT;

static mut BACKBUFFER: [u32; MAX_PIXELS] =
    [0; MAX_PIXELS];

#[derive(PartialEq, Eq, Clone, Copy)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

#[expect(unused)]
impl Color {
    pub const BLACK: Color = Color {
        r: 0,
        g: 0,
        b: 0,
    };

    pub const WHITE: Color = Color {
        r: 255,
        g: 255,
        b: 255,
    };

    pub const RED: Color = Color {
        r: 255,
        g: 0,
        b: 0,
    };

    pub const GREEN: Color = Color {
        r: 0,
        g: 255,
        b: 0,
    };

    pub const BLUE: Color = Color {
        r: 0,
        g: 0,
        b: 255,
    };

    pub fn from_name(name: &str) -> Option<Color> {
        match name {
            "black" => Some(Color::BLACK),
            "red" => Some(Color::RED),
            "green" => Some(Color::GREEN),
            "white" => Some(Color::WHITE),
            "blue" => Some(Color::BLUE),
            _ => None,
        }
    }
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

/*
Dirty rectangle.

The rectangle is:

    [min_x, max_x)
    [min_y, max_y)

If DIRTY is false, there is nothing to present.
*/

static mut DIRTY: bool = false;

static mut DIRTY_MIN_X: usize = 0;
static mut DIRTY_MIN_Y: usize = 0;
static mut DIRTY_MAX_X: usize = 0;
static mut DIRTY_MAX_Y: usize = 0;

// ============================================================
// Initialization
// ============================================================

pub fn init() {
    let response = FRAMEBUFFER_REQUEST
        .response()
        .expect("No framebuffer response");

    let framebuffer = response
        .framebuffers()
        .first()
        .expect("No framebuffer");

    let width = framebuffer.width as usize;
    let height = framebuffer.height as usize;
    let pitch = framebuffer.pitch as usize;
    let bpp = framebuffer.bpp as usize;

    if bpp != 32 {
        panic!("Framebuffer must be 32bpp");
    }

    if width == 0 || height == 0 {
        panic!("Framebuffer has invalid dimensions");
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

        DIRTY = false;
        DIRTY_MIN_X = 0;
        DIRTY_MIN_Y = 0;
        DIRTY_MAX_X = 0;
        DIRTY_MAX_Y = 0;
    }

    clear(Color::BLACK);
    present();
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

/*
Marks a rectangle as dirty.

Coordinates are clipped to the framebuffer.
*/

#[inline(always)]
fn mark_dirty(
    x: usize,
    y: usize,
    width: usize,
    height: usize,
) {
    let info = info();

    if width == 0 || height == 0 {
        return;
    }

    if x >= info.width || y >= info.height {
        return;
    }

    let max_x = x
        .saturating_add(width)
        .min(info.width);

    let max_y = y
        .saturating_add(height)
        .min(info.height);

    if max_x <= x || max_y <= y {
        return;
    }

    unsafe {
        if !DIRTY {
            DIRTY = true;

            DIRTY_MIN_X = x;
            DIRTY_MIN_Y = y;
            DIRTY_MAX_X = max_x;
            DIRTY_MAX_Y = max_y;
        } else {
            DIRTY_MIN_X = DIRTY_MIN_X.min(x);
            DIRTY_MIN_Y = DIRTY_MIN_Y.min(y);
            DIRTY_MAX_X = DIRTY_MAX_X.max(max_x);
            DIRTY_MAX_Y = DIRTY_MAX_Y.max(max_y);
        }
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
// Pixel
// ============================================================

#[inline(always)]
pub fn put_pixel(
    x: usize,
    y: usize,
    color: Color,
) {
    let info = info();

    if x >= info.width || y >= info.height {
        return;
    }

    let pixel = color_to_u32(color);

    unsafe {
        let back =
            ptr::addr_of_mut!(BACKBUFFER) as *mut u32;

        *back.add(
            y * info.width + x
        ) = pixel;
    }

    mark_dirty(x, y, 1, 1);
}

// ============================================================
// Clear
// ============================================================

pub fn clear(color: Color) {
    let info = info();

    let pixel = color_to_u32(color);

    unsafe {
        let back =
            ptr::addr_of_mut!(BACKBUFFER) as *mut u32;

        let buffer =
            core::slice::from_raw_parts_mut(
                back,
                info.width * info.height,
            );

        buffer.fill(pixel);
    }

    mark_dirty(
        0,
        0,
        info.width,
        info.height,
    );
}

// ============================================================
// Rectangle
// ============================================================

pub fn draw_rect(
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    color: Color,
) {
    let info = info();

    if width == 0 || height == 0 {
        return;
    }

    if x >= info.width || y >= info.height {
        return;
    }

    let end_x = x
        .saturating_add(width)
        .min(info.width);

    let end_y = y
        .saturating_add(height)
        .min(info.height);

    if end_x <= x || end_y <= y {
        return;
    }

    let pixel = color_to_u32(color);

    unsafe {
        let back =
            ptr::addr_of_mut!(BACKBUFFER) as *mut u32;

        for py in y..end_y {
            let row =
                back.add(
                    py * info.width + x
                );

            let row =
                core::slice::from_raw_parts_mut(
                    row,
                    end_x - x,
                );

            row.fill(pixel);
        }
    }

    mark_dirty(
        x,
        y,
        end_x - x,
        end_y - y,
    );
}

// ============================================================
// Copy rectangle
// ============================================================

/*
Copies a rectangle inside the backbuffer.

ptr::copy() is overlap-safe, making this suitable for
scrolling.
*/

pub fn copy_rect(
    src_x: usize,
    src_y: usize,
    dst_x: usize,
    dst_y: usize,
    width: usize,
    height: usize,
) {
    let info = info();

    if width == 0 || height == 0 {
        return;
    }

    if src_x >= info.width
        || src_y >= info.height
        || dst_x >= info.width
        || dst_y >= info.height
    {
        return;
    }

    let width = width
        .min(info.width - src_x)
        .min(info.width - dst_x);

    let height = height
        .min(info.height - src_y)
        .min(info.height - dst_y);

    if width == 0 || height == 0 {
        return;
    }

    unsafe {
        let back =
            ptr::addr_of_mut!(BACKBUFFER) as *mut u32;

        /*
         * ptr::copy() handles overlapping source/destination
         * regions correctly.
         */
        for row in 0..height {
            let src =
                back.add(
                    (src_y + row) * info.width + src_x
                );

            let dst =
                back.add(
                    (dst_y + row) * info.width + dst_x
                );

            ptr::copy(
                src,
                dst,
                width,
            );
        }
    }

    mark_dirty(
        dst_x,
        dst_y,
        width,
        height,
    );
}

// ============================================================
// Scroll up
// ============================================================

/*
Scrolls the framebuffer backbuffer upward by `pixels`.

The newly exposed area at the bottom is filled with `color`.
*/

pub fn scroll_up(
    pixels: usize,
    color: Color,
) {
    let info = info();

    if pixels == 0 {
        return;
    }

    if pixels >= info.height {
        clear(color);
        return;
    }

    copy_rect(
        0,
        pixels,
        0,
        0,
        info.width,
        info.height - pixels,
    );

    draw_rect(
        0,
        info.height - pixels,
        info.width,
        pixels,
        color,
    );
}

// ============================================================
// Present
// ============================================================

/*
Copies only the dirty region from the backbuffer into the
actual framebuffer.

For a single terminal character this is approximately:

    8 × 16 × 4 = 512 bytes

instead of copying the entire 1920×1080 framebuffer.
*/

pub fn present() {
    let info = info();

    let (
        min_x,
        min_y,
        max_x,
        max_y,
    ) = unsafe {
        if !DIRTY {
            return;
        }

        let result = (
            DIRTY_MIN_X,
            DIRTY_MIN_Y,
            DIRTY_MAX_X,
            DIRTY_MAX_Y,
        );

        DIRTY = false;

        result
    };

    if min_x >= max_x || min_y >= max_y {
        return;
    }

    let width =
        max_x - min_x;

    let row_size =
        width * BYTES_PER_PIXEL;

    unsafe {
        let back =
            ptr::addr_of!(BACKBUFFER) as *const u8;

        let front =
            info.front;

        for y in min_y..max_y {
            let src =
                back.add(
                    y * info.width
                        * BYTES_PER_PIXEL
                        + min_x
                            * BYTES_PER_PIXEL,
                );

            let dst =
                front.add(
                    y * info.front_pitch
                        + min_x
                            * BYTES_PER_PIXEL,
                );

            ptr::copy_nonoverlapping(
                src,
                dst,
                row_size,
            );
        }
    }
}

// ============================================================
// Full Present
// ============================================================

pub fn present_full() {
    let info = info();

    mark_dirty(
        0,
        0,
        info.width,
        info.height,
    );

    present();
}

// ============================================================
// Info
// ============================================================

#[inline]
pub fn width() -> usize {
    info().width
}

#[inline]
pub fn height() -> usize {
    info().height
}

#[inline]
pub fn pitch() -> usize {
    info().front_pitch
}
