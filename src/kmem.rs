use crate::memory::{
    self,
    frame_alloc::{
        BitmapFrameAllocator,
        PhysFrame,
    },
    page_alloc::{
        BitmapPageAllocator,
        PageRange,
        VirtAddr,
    },
    page_table::read_cr3,
};

use limine::request::{
    HhdmRequest,
    MemmapRequest,
};

//
// ---------------------------------------------------------------------------
// Limine requests
// ---------------------------------------------------------------------------
//

#[used]
#[unsafe(link_section = ".limine_reqs")]
pub static HHDM_REQUEST: HhdmRequest =
    HhdmRequest::new();

#[used]
#[unsafe(link_section = ".limine_reqs")]
pub static MEMORY_MAP_REQUEST: MemmapRequest =
    MemmapRequest::new();

//
// ---------------------------------------------------------------------------
// Kernel page allocator configuration
// ---------------------------------------------------------------------------
//
// We reserve a virtual-address region for kernel dynamic memory.
//
// This is NOT the heap yet.
//
// This allocator only manages virtual pages. The actual heap allocator will
// eventually sit on top of this layer.
//
// 256 MiB:
//
//     256 MiB / 4 KiB = 65536 pages
//
//     65536 bits / 64 = 1024 u64s
//

pub const KERNEL_PAGE_START: u64 =
    0xffff_8000_0000_0000;

pub const KERNEL_PAGE_SIZE: u64 =
    256 * 1024 * 1024;

pub const KERNEL_PAGE_END: u64 =
    KERNEL_PAGE_START + KERNEL_PAGE_SIZE;

pub const KERNEL_PAGE_COUNT: usize =
    (KERNEL_PAGE_SIZE / memory::frame_alloc::FRAME_SIZE)
        as usize;

pub const KERNEL_PAGE_BITMAP_WORDS: usize =
    (KERNEL_PAGE_COUNT + 63) / 64;

//
// ---------------------------------------------------------------------------
// Static page bitmap storage
// ---------------------------------------------------------------------------
//
// This storage belongs exclusively to KernelMemory.
//
// Rust 2024 does not allow:
//
//     &mut PAGE_BITMAP
//
// Instead we obtain a raw pointer with:
//
//     &raw mut PAGE_BITMAP
//
// and construct the mutable slice inside the unsafe block.
//

#[repr(align(4096))]
struct PageBitmapStorage(
    [u64; KERNEL_PAGE_BITMAP_WORDS]
);

static mut PAGE_BITMAP: PageBitmapStorage =
    PageBitmapStorage(
        [0; KERNEL_PAGE_BITMAP_WORDS]
    );

//
// ---------------------------------------------------------------------------
// KernelMemory
// ---------------------------------------------------------------------------
//

/// Kernel physical and virtual memory manager.
///
/// Currently this owns:
///
/// - the physical frame allocator
/// - the kernel virtual-page allocator
/// - the HHDM offset
/// - the active PML4 frame
///
/// The page allocator does not perform mappings. It only manages virtual
/// address-space ownership.
///
/// Mapping/unmapping remains the responsibility of `Mapper`.
pub struct KernelMemory {
    /// Physical frame allocator.
    pub frames: BitmapFrameAllocator,

    /// Kernel virtual-page allocator.
    pub pages: BitmapPageAllocator,

    /// Limine HHDM offset.
    pub hhdm_offset: u64,

    /// Physical frame containing the active PML4.
    pub pml4_frame: PhysFrame,
}

//
// ---------------------------------------------------------------------------
// Initialization
// ---------------------------------------------------------------------------
//

/// Initialize the kernel memory subsystem.
///
/// This must run after Limine has initialized the boot information and before
/// anything attempts to allocate physical frames or kernel virtual pages.
///
/// The initialization performs:
///
/// 1. Read Limine's memory map.
/// 2. Read Limine's HHDM.
/// 3. Initialize the physical frame allocator.
/// 4. Read CR3.
/// 5. Initialize the kernel virtual-page allocator.
///
/// The kernel page allocator manages:
///
///     [KERNEL_PAGE_START, KERNEL_PAGE_END)
///
/// At this stage this is only virtual address-space management. No pages in
/// this range are mapped automatically.
pub fn init() -> KernelMemory {
    //
    // -----------------------------------------------------------------------
    // 1. Limine memory map
    // -----------------------------------------------------------------------
    //

    let memory_map =
        MEMORY_MAP_REQUEST
            .response()
            .expect(
                "Limine did not provide a memory map"
            );

    //
    // -----------------------------------------------------------------------
    // 2. Limine HHDM
    // -----------------------------------------------------------------------
    //

    let hhdm =
        HHDM_REQUEST
            .response()
            .expect(
                "Limine did not provide an HHDM"
            );

    //
    // -----------------------------------------------------------------------
    // 3. Physical frame allocator
    // -----------------------------------------------------------------------
    //

    let frames = unsafe {
        BitmapFrameAllocator::new(
            memory_map.entries(),
            hhdm.offset,
        )
        .expect(
            "failed to initialize physical frame allocator"
        )
    };

    //
    // -----------------------------------------------------------------------
    // 4. Active PML4
    // -----------------------------------------------------------------------
    //

    let pml4_frame =
        read_cr3();

    //
    // -----------------------------------------------------------------------
    // 5. Kernel virtual-page allocator
    // -----------------------------------------------------------------------
    //
    // The bitmap is static storage.
    //
    // IMPORTANT:
    //
    // Do not use:
    //
    //     &mut PAGE_BITMAP
    //
    // because Rust 2024 rejects mutable references to mutable statics.
    //
    // Instead use `&raw mut PAGE_BITMAP`, which produces a raw pointer.
    //

    let page_range =
        PageRange::new(
            VirtAddr::new(
                KERNEL_PAGE_START
            ),
            VirtAddr::new(
                KERNEL_PAGE_END
            ),
        )
        .expect(
            "kernel page allocator range is invalid"
        );

    let pages = unsafe {
        let bitmap_ptr =
            (&raw mut PAGE_BITMAP)
                .cast::<u64>();

        let bitmap =
            core::slice::from_raw_parts_mut(
                bitmap_ptr,
                KERNEL_PAGE_BITMAP_WORDS,
            );

        BitmapPageAllocator::new(
            page_range,
            bitmap,
        )
        .expect(
            "failed to initialize kernel page allocator"
        )
    };

    //
    // -----------------------------------------------------------------------
    // 6. Print diagnostics
    // -----------------------------------------------------------------------
    //

    let frame_stats =
        frames.stats();

    let page_stats =
        pages.stats();

    crate::serial_println!(
        "physical memory: {} total frames, {} free",
        frame_stats.total_frames,
        frame_stats.free_frames,
    );

    crate::serial_println!(
        "frame bitmap: {:?}, {} bytes",
        frame_stats.bitmap_start,
        frame_stats.bitmap_size,
    );

    crate::serial_println!(
        "PML4: {:?}",
        pml4_frame,
    );

    crate::serial_println!(
        "kernel virtual memory: {:#x} - {:#x}",
        KERNEL_PAGE_START,
        KERNEL_PAGE_END,
    );

    crate::serial_println!(
        "kernel pages: {} total, {} free",
        page_stats.total_pages,
        page_stats.free_pages,
    );

    crate::serial_println!(
        "kernel page bitmap: {} words, {} bytes",
        KERNEL_PAGE_BITMAP_WORDS,
        KERNEL_PAGE_BITMAP_WORDS * core::mem::size_of::<u64>(),
    );

    //
    // -----------------------------------------------------------------------
    // 7. Construct KernelMemory
    // -----------------------------------------------------------------------
    //

    KernelMemory {
        frames,
        pages,
        hhdm_offset: hhdm.offset,
        pml4_frame,
    }
}
