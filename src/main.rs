#![no_std]
#![no_main]

mod fb;
mod font;
mod serial;
mod test;

#[cfg(feature = "test")]
#[path = "../tests/trivial_assert.rs"]
mod trivial_assert;

pub use crate::test::{TestResult, test};

extern crate sillos_test_macro;

#[cfg(feature = "test")]
mod unit_tests;

use core::panic::PanicInfo;

pub const DEBUG_TOGGLE: bool = true;

use limine::{RequestsEndMarker, RequestsStartMarker};

#[used]
#[unsafe(link_section = ".limine_req_start")]
static REQUESTS_START_MARKER: RequestsStartMarker = RequestsStartMarker::new();

#[used]
#[unsafe(link_section = ".limine_req_end")]
static REQUESTS_END_MARKER: RequestsEndMarker = RequestsEndMarker::new();

#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.entry")]
pub extern "C" fn kmain() -> ! {
    serial_println!("SillOS starting.");

    #[cfg(feature = "test")]
    {
        test::run();
    }

    #[cfg(not(feature = "test"))]
    {
        kernel();
    }

    loop {
        core::hint::spin_loop();
    }
}

fn kernel() {
    serial_println!("1: entered kernel");

    fb::init();

    serial_println!("2: fb::init() returned");

    fb::clear(fb::Color::BLACK);

    serial_println!("3: clear returned");

    fb::draw_rect(100, 100, 300, 200, fb::Color::RED);

    serial_println!("4: draw_rect returned");

    let font = font::spleen();

    font.draw_text(0, 0, "Hello from SillOS", fb::Color::GREEN);

    fb::present();

    serial_println!("5: present returned");
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    serial_println!("KERNEL PANIC: {}", info);
    loop {
        core::hint::spin_loop();
    }
}
