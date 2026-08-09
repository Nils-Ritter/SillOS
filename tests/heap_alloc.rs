#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(SillOS::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc as ext_alloc;

use ext_alloc::{boxed::Box, vec::Vec};
use SillOS::alloc::HEAP_SIZE;
use bootloader::{BootInfo, entry_point};
use core::panic::PanicInfo;

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

#[test_case]
fn simple_allocation() {
    let heap_value_1 = Box::new(41);
    let heap_value_2 = Box::new(13);
    assert_eq!(*heap_value_1, 41);
    assert_eq!(*heap_value_2, 13);
}

#[test_case]
fn large_vec() {
    let n = 1000;
    let mut vec = Vec::new();
    for i in 0..n {
        vec.push(i);
    }
    assert_eq!(vec.iter().sum::<u64>(), (n - 1) * n / 2);
}

#[test_case]
fn filling_heap() {
    for i in 0..HEAP_SIZE {
        let x = Box::new(i);
        assert_eq!(*x, i);
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    SillOS::test_panic_handler(info)
}
