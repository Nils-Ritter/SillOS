#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]

mod fb;
mod acpi;
mod font;
mod serial;
mod test;
pub mod int;
pub mod gdt;
mod pic;

#[cfg(feature = "test")]
#[path = "../tests/trivial_assert.rs"]
mod trivial_assert_test;

#[cfg(feature = "test")]
#[path = "../tests/interrupts.rs"]
mod interrupts_test;

use crate::acpi::reboot;
pub use crate::test::{TestResult, test};

extern crate sillos_test_macro;

#[cfg(feature = "test")]
mod unit_tests;

use core::panic::PanicInfo;

pub const DEBUG_TOGGLE: bool = true;
pub static mut TESTING: bool = false;

use limine::{RequestsEndMarker, RequestsStartMarker, request::RsdpRequest};

#[used]
#[unsafe(link_section = ".limine_req_start")]
static REQUESTS_START_MARKER: RequestsStartMarker = RequestsStartMarker::new();

#[used]
#[unsafe(link_section = ".limine_req_end")]
static REQUESTS_END_MARKER: RequestsEndMarker = RequestsEndMarker::new();

#[used]
#[unsafe(link_section = ".limine_reqs")]
static RSDP_REQUEST: RsdpRequest = RsdpRequest::new();

#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.entry")]
pub extern "C" fn kmain() -> ! {
    serial_println!("SillOS starting.");
    kinit();

    #[cfg(feature = "test")]
    {
        unsafe { TESTING = true; }
        test::run();
    }

    #[cfg(not(feature = "test"))]
    {
        use crate::acpi::reboot;

        unsafe { TESTING = false; }
        kernel();
    }

    loop {
        x86_64::instructions::hlt();
    }
}

static mut KINIT_CALLED: bool = false;

fn kinit(){
    unsafe {
        if KINIT_CALLED { panic!("KINIT CANNOT BE CALLED MORE THAN ONCE"); }
        KINIT_CALLED = true;
    }
    
    serial_println!("Initializing GDT...");
    gdt::init();
    serial_println!("GDT initialized.");

    serial_println!("Initializing IDT...");
    int::init();
    serial_println!("IDT initialized.");

    serial_println!("Initializing PIC...");
    pic::init();
    serial_println!("PIC initialized.");

    serial_println!("Enabling interrupts...");
    x86_64::instructions::interrupts::enable();
    serial_println!("Interrupts enabled.");

    serial_println!("Initializing ACPI...");
    acpi::init();
    serial_println!("ACPI initialized.");

    serial_println!("Calling int3...");
    unsafe {
        core::arch::asm!("int3");
    }
    serial_println!("Returned from int3.");

    serial_println!();
    serial_println!("Kernel is running.");
    serial_println!("Timer interrupts should now arrive.");
    serial_println!("Press keys to test keyboard IRQs.");
    serial_println!();
}

fn kernel() {
    fb::init();
    fb::clear(fb::Color::BLACK);
    fb::draw_rect(100, 100, 300, 200, fb::Color::RED);

    serial_println!("4: draw_rect returned");

    let font = font::spleen();

    font.draw_text(0, 0, "Hello from SillOS", fb::Color::GREEN);

    fb::present();

    serial_println!("5: present returned");

    serial_println!("REBOOTING VIA ACPI");
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    serial_println!("KERNEL PANIC: {}", info);
    loop {
        core::hint::spin_loop();
    }
}
