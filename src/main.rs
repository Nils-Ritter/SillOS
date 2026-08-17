#![no_std]
#![no_main]

use core::panic::PanicInfo;

use limine::{
    RequestsEndMarker,
    RequestsStartMarker,
};

use limine::request::FramebufferRequest;

#[used]
#[unsafe(link_section = ".limine_req_start")]
static REQUESTS_START_MARKER: RequestsStartMarker = RequestsStartMarker::new();

#[used]
#[unsafe(link_section = ".limine_reqs")]
static FRAMEBUFFER_REQUEST: FramebufferRequest = FramebufferRequest::new();

#[used]
#[unsafe(link_section = ".limine_req_end")]
static REQUESTS_END_MARKER: RequestsEndMarker = RequestsEndMarker::new();

#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.entry")]
pub extern "C" fn kmain() -> ! {
    let framebuffer_response = FRAMEBUFFER_REQUEST
        .response()
        .expect("Limine did not provide a framebuffer");

    let framebuffer = framebuffer_response
        .framebuffers()
        .first()
        .expect("No framebuffer available");

    let width = framebuffer.width as usize;
    let height = framebuffer.height as usize;
    let pitch = framebuffer.pitch as usize;

    let address = framebuffer.address();
    let buffer = address as *mut u32;

    for y in 0..height {
        for x in 0..width {
            let r = (x * 255 / width) as u32;
            let g = (y * 255 / height) as u32;
            let b = 80u32;

            let color = (r << 16) | (g << 8) | b;

            let offset = y * (pitch / 4) + x;

            unsafe {
                buffer.add(offset).write_volatile(color);
            }
        }
    }

    loop {
        core::hint::spin_loop();
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
