use crate::memory::{self, frame_alloc::BitmapFrameAllocator, page_table::read_cr3};
use limine::request::{HhdmRequest, MemmapRequest};
use memory::frame_alloc::{PhysFrame};

#[used]
#[unsafe(link_section = ".limine_reqs")]
pub static HHDM_REQUEST: HhdmRequest =
    HhdmRequest::new();

#[used]
#[unsafe(link_section = ".limine_reqs")]
pub static MEMORY_MAP_REQUEST: MemmapRequest =
    MemmapRequest::new();

/// Kernel physical-memory manager.
///
/// This currently owns the physical frame allocator. As the memory
/// subsystem grows, this can become the owner of the page-table mapper,
/// kernel address-space manager, heap, etc.

pub struct KernelMemory {
    pub frames: BitmapFrameAllocator,
    pub hhdm_offset: u64,
    pub pml4_frame: PhysFrame,
}

/// Initialize the kernel memory subsystem.
///
/// This must run after Limine has initialized the boot information and
/// before anything attempts to allocate physical frames.
///
/// The HHDM is required because the frame allocator stores its bitmap in
/// physical memory and accesses it through the direct physical-memory map.
pub fn init() -> KernelMemory {
    let memory_map = MEMORY_MAP_REQUEST
        .response()
        .expect("Limine did not provide a memory map");

    let hhdm = HHDM_REQUEST
        .response()
        .expect("Limine did not provide an HHDM");

    let frames = unsafe {
        BitmapFrameAllocator::new(
            memory_map.entries(),
            hhdm.offset,
        )
        .expect("failed to initialize physical frame allocator")
    };

    let pml4_frame = read_cr3();

    let stats = frames.stats();

    crate::serial_println!(
        "physical memory: {} total frames, {} free",
        stats.total_frames,
        stats.free_frames,
    );

    crate::serial_println!(
        "frame bitmap: {:?}, {} bytes",
        stats.bitmap_start,
        stats.bitmap_size,
    );

    crate::serial_println!(
        "PML4: {:?}",
        pml4_frame,
    );

    KernelMemory {
        frames,
        hhdm_offset: hhdm.offset,
        pml4_frame,
    }
}
