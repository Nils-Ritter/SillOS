use spin::MutexGuard;
use x86_64::{
    structures::paging::{
        mapper::MapToError, FrameAllocator, Mapper, Page, PageTableFlags, Size4KiB,
    },
    VirtAddr
};

use crate::{alloc::{fixed_size_block::FixedSizeBlockAllocator}, print, println};

pub mod bump;
pub mod linked_list;
pub mod fixed_size_block;

pub const HEAP_START: usize = 0x_4444_4444_0000;
pub const HEAP_SIZE: usize = 1000 * 1024; //~1MiB

pub fn init_heap(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), MapToError<Size4KiB>>{
    print!("[STARTUP] Initializing kernel heap... ");
    let page_range = {
        let heap_start = VirtAddr::new(HEAP_START as u64);
        let heap_end = heap_start + HEAP_SIZE as u64 - 1u64;
        let heap_start_page = Page::containing_address(heap_start);
        let heap_end_page = Page::containing_address(heap_end);
        Page::range_inclusive(heap_start_page, heap_end_page)
    };
    
    for page in page_range {
        let frame = frame_allocator.allocate_frame().ok_or(MapToError::FrameAllocationFailed)?;
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
        unsafe { mapper.map_to(page, frame, flags, frame_allocator)?.flush() }
    }

    unsafe { ALLOCATOR.lock().init(HEAP_START, HEAP_SIZE); }

    println!("[ok]");
    return Ok(());
}

#[global_allocator]
static ALLOCATOR: Locked<FixedSizeBlockAllocator> = Locked::new(FixedSizeBlockAllocator::new());

pub struct Locked<A> { inner: spin::Mutex<A> }

impl<A> Locked<A>{
    pub const fn new(inner: A) -> Self {
        return Locked { inner: spin::Mutex::new(inner) };
    }

    pub fn lock(&self) -> MutexGuard<'_, A> {
        return self.inner.lock();
    }
}

///Align the given address "addr" upwards to alignment "align".
///
///Requires that "align" is a power of two.
fn align_up(addr: usize, align: usize) -> usize {
    return (addr + align - 1) & !(align - 1);
}
