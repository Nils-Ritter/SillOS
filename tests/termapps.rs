#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(SillOS::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc as ext_alloc;

use ext_alloc::{boxed::Box, vec::Vec};
use SillOS::{alloc::HEAP_SIZE, term::exec_str_as_cmd};
use bootloader::{BootInfo, entry_point};
use core::{panic::PanicInfo, slice::GetDisjointMutError::OverlappingIndices};

entry_point!(main);

fn main(boot_info: &'static BootInfo) -> ! {
    use SillOS::alloc;
    use SillOS::mem::{self, BootInfoFrameAllocator};
    use x86_64::VirtAddr;

    SillOS::init();
    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { mem::init(phys_mem_offset) };
    let mut frame_allocator = unsafe { BootInfoFrameAllocator::init(&boot_info.memory_map) };
    alloc::init_heap(&mut mapper, &mut frame_allocator).expect("heap initialization failed");

    test_main();
    loop {}
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    SillOS::test_panic_handler(info)
}

#[test_case]
fn test_termapp_help(){
    unsafe { exec_str_as_cmd(ext_alloc::string::ToString::to_string(&"help")); }
}

#[test_case]
fn test_termapp_bp(){
    unsafe { exec_str_as_cmd(ext_alloc::string::ToString::to_string(&"bp")); }
}

#[test_case]
fn test_termapp_clear(){
    unsafe { exec_str_as_cmd(ext_alloc::string::ToString::to_string(&"clear")); }
    panic!();
}
