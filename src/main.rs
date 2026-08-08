#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(test_runner)]
#![reexport_test_harness_main = "test_main"]

mod vga_buffer;
mod serial;

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
#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    print!("[FATAL] {}\n", info);
    loop {}
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    serial_print!("[FATAL] {}\n", info);
    exit_qemu(QemuExitCode::Failed);
    loop {}
}

#[cfg(test)]
pub fn test_runner(tests: &[&dyn Fn()]) {
    println!("[TESTS] Running {} tests", tests.len());
    serial_println!("[TESTS] Running {} tests", tests.len());
    for test in tests {
        test();
    }
    exit_qemu(QemuExitCode::Success);
}

#[test_case]
fn trivial_assertion(){
    serial_print!("[TESTS] Trivial assertion... ");
    assert_eq!(1, 1);
    serial_println!("[ok]");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum QemuExitCode {
    Success = 0x10,
    Failed = 0x11,
}

pub fn exit_qemu(exit_code: QemuExitCode) {
    use x86_64::instructions::port::Port;

    unsafe {
        let mut port = Port::new(0xf4);
        port.write(exit_code as u32);
    }
}
