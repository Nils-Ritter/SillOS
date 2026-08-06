#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(test_runner)]
#![reexport_test_harness_main = "test_main"]

mod vga_buffer;

use core::panic::PanicInfo;

/// The main entry point, pointed to by the linker.
/// Then calls k_main.
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    k_main();
    loop {}
}

/// The main kernel function
/// this ever returning will halt the cpu forever.
fn k_main(){
    println!("[INFO] Welcome to SillOS!");
    #[cfg(test)]
    test_main();
}

/// This function is called on panic.
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    print!("[FATAL] {}", info);
    loop {}
}

#[cfg(test)]
pub fn test_runner(tests: &[&dyn Fn()]) {
    println!("[TESTS] Running {} tests", tests.len());
    for test in tests {
        test();
    }
}

#[test_case]
fn trivial_assertion(){
    print!("[TESTS] Trivial assertion... ");
    assert_eq!(1, 1);
    println!("[ok]");
}
