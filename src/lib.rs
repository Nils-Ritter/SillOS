#![no_std]
#![cfg_attr(test, no_main)]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]
#![feature(abi_x86_interrupt)]

extern crate alloc as ext_alloc;

pub mod serial;
pub mod vga_buffer;
pub mod interrupts;
pub mod gdt;
pub mod term;
pub mod mem;
pub mod alloc;
pub mod termapps;

use core::panic::PanicInfo;

#[cfg(test)]
use bootloader::{BootInfo, entry_point};

#[cfg(test)]
entry_point!(test_kmain);

pub trait Testable {
    fn run(&self) -> ();
}

impl<T> Testable for T
where
    T: Fn(),
{
    fn run(&self) {
        serial_print!("{}...\t", core::any::type_name::<T>());
        print!("{}...\t", core::any::type_name::<T>());
        self();
        serial_println!("[OK]");
        println!("[OK]");
    }
}

pub fn test_runner(tests: &[&dyn Testable]) {
    serial_println!("Running {} tests", tests.len());
    println!("[TESTING] Running {} tests", tests.len());
    for test in tests {
        test.run();
    }
    exit_qemu(QemuExitCode::Success);
}

pub fn test_panic_handler(info: &PanicInfo) -> ! {
    serial_println!("[FAILED]\n");
    serial_println!("Error: {}\n", info);
    println_error!("[FAILED]\n");
    println_error!("Error: {}\n", info);
    hlt_loop();
}

/// Entry point for `cargo test`
#[cfg(test)]
fn test_kmain(boot_info: &'static BootInfo) -> ! {
    init();
    test_main();
    hlt_loop();
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    test_panic_handler(info)
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

///This function will halt the cpu indefinitely.
pub fn hlt_loop() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}

pub fn init(){
    print!("[STARTUP] Initializing IDT... ");
    interrupts::init_idt();
    println!("[OK]");

    print!("[STARTUP] Initializing GDT... ");
    gdt::init();
    println!("[OK]");

    print!("[STARTUP] Setting up PICS... ");
    unsafe { interrupts::PICS.lock().initialize(); }
    println!("[OK]");

    print!("[STARTUP] Enabling interrupts... ");
    x86_64::instructions::interrupts::enable();
    println!("[OK]");
}
