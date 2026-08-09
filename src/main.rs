#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(SillOS::test_runner)]
#![reexport_test_harness_main = "test_main"]

use SillOS::{print, println};
use x86_64::registers::control::Cr3;
use core::panic::PanicInfo;
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    println!("Hello World{}", "!");

    SillOS::init();
    
    let (level_4_page_table, _) = Cr3::read();
    println!("[INFO] Level 4 page table at: {:?}", level_4_page_table.start_address());

    #[cfg(test)]
    test_main();

    SillOS::hlt_loop();
}

/// This function is called on panic.
#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    use SillOS::println_error;
    println_error!("\n[KERNEL PANIC] {}", info);
    SillOS::hlt_loop()
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    SillOS::test_panic_handler(info)
}
