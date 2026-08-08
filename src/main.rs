#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(SillOS::test_runner)]
#![reexport_test_harness_main = "test_main"]

use SillOS::println;
use core::panic::PanicInfo;

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    println!("Hello World{}", "!");

    SillOS::init();
    

    #[cfg(test)]
    test_main();

    x86_64::instructions::interrupts::int3(); //test breakpoint

    loop {}
}

/// This function is called on panic.
#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    loop {}
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    SillOS::test_panic_handler(info)
}
