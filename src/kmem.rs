use crate::memory::frame_alloc::BitmapFrameAllocator;
use limine::request::{HhdmRequest, MemmapRequest};

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

    let stats = frames.stats();

    crate::serial_println!(
        "physical memory: {} frames total, {} free",
        stats.total_frames,
        stats.free_frames,
    );

    crate::serial_println!(
        "frame bitmap: {:#x} ({:#x} bytes)",
        stats.bitmap_start.as_u64(),
        stats.bitmap_size,
    );

    KernelMemory {
        frames,
    }
}
