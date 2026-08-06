#![no_std]
#![no_main]

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
    panic!("Kernel execution is over!")
}

/// This function is called on panic.
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    print!("[FATAL] {}", info);
    loop {}
}
