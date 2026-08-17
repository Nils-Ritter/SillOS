#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(SillOS::test_runner)]
#![reexport_test_harness_main = "test_main"]

use SillOS::{alloc, mem::{self, BootInfoFrameAllocator}, println, term::begin_new_cmd_line};
use bootloader::{BootInfo, entry_point};
use x86_64::{VirtAddr, structures::paging::Page};
use core::panic::PanicInfo;
use ext_alloc::{boxed::Box, rc::Rc, vec::Vec, vec};

extern crate alloc as ext_alloc;

entry_point!(kmain);

fn kmain(boot_info: &'static BootInfo) -> ! {
    SillOS::init();
    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { mem::init(phys_mem_offset) };

    let mut frame_allocator = unsafe { BootInfoFrameAllocator::init(&boot_info.memory_map) };

    alloc::init_heap(&mut mapper, &mut frame_allocator).expect("heap init failed");

    // map an unused page
    let page = Page::containing_address(VirtAddr::new(0xdeadbeef000));
    mem::create_example_mapping(page, &mut mapper, &mut frame_allocator);

    // write the string `New!` to the screen through the new mapping
    let page_ptr: *mut u64 = page.start_address().as_mut_ptr();
    unsafe { page_ptr.offset(20).write_volatile(0x_f021_f077_f065_f04e)};

    println!("[STARTUP] Startup done, entering kernel!");

    let x = Box::new(41);

    println!("[INFO] Boxed var is: {}", x);

    // allocate a number on the heap
    let heap_value = Box::new(41);
    println!("heap_value at {:p}", heap_value);

    // create a dynamically sized vector
    let mut vec = Vec::new();
    for i in 0..50000 {
        vec.push(i);
    }
    println!("vec at {:p}", vec.as_slice());

    // create a reference counted vector -> will be freed when count reaches 0
    let reference_counted = Rc::new(vec![1, 2, 3]);
    let cloned_reference = reference_counted.clone();
    println!("current reference count is {}", Rc::strong_count(&cloned_reference));
    core::mem::drop(reference_counted);
    println!("reference count is {} now", Rc::strong_count(&cloned_reference));

    begin_new_cmd_line();

    #[cfg(test)]
    test_main();

    SillOS::hlt_loop();
}

/// This function is called on panic.
/// Very likely to be misdiagnosed by LSPs.
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
