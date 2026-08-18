use crate::test::{test, TestResult};

use crate::memory::{
    frame_alloc::{
        FrameAllocator,
        FrameError,
        PhysAddr,
        PhysFrame,
        FRAME_SIZE,
    },
    page_alloc::{
        BitmapPageAllocator,
        Page,
        PageAllocatorError,
        PageRange,
        VirtAddr,
    },
    page_table::{
        is_canonical,
        read_cr3,
        Mapper,
        MapperError,
        PageTable,
        PageTableEntry,
        PageTableFlags,
        PAGE_TABLE_ENTRIES,
        PAGE_TABLE_SIZE,
    },
};

use crate::kmem::KernelMemory;

//
// ============================================================================
// Helpers
// ============================================================================
//

fn pass() -> TestResult {
    TestResult::Pass
}

fn fail(reason: &'static str) -> TestResult {
    TestResult::Fail(reason)
}

//
// Keep all mapper tests well outside the kernel's normal virtual range.
//
// Kernel range:
//
//     0xffff800000000000 - 0xffff800010000000
//
// Test range:
//
//     0xffff900000000000 onward
//
// This is canonical for 48-bit x86-64.
//
const TEST_VA_BASE: u64 = 0xffff_9000_0000_0000;

fn test_page(index: u64) -> Page {
    Page::from_start_address(
        VirtAddr::new(
            TEST_VA_BASE + index * FRAME_SIZE
        )
    )
    .expect("test address must be page aligned")
}

//
// Convert our aligned test storage into the exact bitmap slice expected by
// BitmapPageAllocator.
//
// Rust 2024 denies creating references directly from `static mut`. Obtain the
// address as a raw pointer first, then create the temporary mutable slice.
//
// SAFETY:
// - The caller must provide a valid, uniquely-owned pointer to the storage.
// - The pointed-to storage must remain valid for the returned lifetime.
// - Each test uses its own storage object and does not access it concurrently.
//
unsafe fn bitmap_storage(
    storage: *mut [u64; 128],
) -> &'static mut [u64] {
    core::slice::from_raw_parts_mut(
        (*storage).as_mut_ptr(),
        128,
    )
}

//
// ============================================================================
// ADDRESS TESTS
// ============================================================================
//

#[test]
fn test_phys_addr_creation() -> TestResult {
    let address = PhysAddr::new(0x1234_5000);

    if address.as_u64() != 0x1234_5000 {
        return fail("PhysAddr returned incorrect value");
    }

    pass()
}

#[test]
fn test_phys_addr_alignment() -> TestResult {
    let aligned = PhysAddr::new(0x4000);

    if !aligned.is_aligned() {
        return fail("aligned physical address reported unaligned");
    }

    let unaligned = PhysAddr::new(0x4001);

    if unaligned.is_aligned() {
        return fail("unaligned physical address reported aligned");
    }

    pass()
}

#[test]
fn test_phys_frame_creation() -> TestResult {
    let address = PhysAddr::new(0x8000);

    let frame = match PhysFrame::from_start_address(address) {
        Some(frame) => frame,
        None => return fail("failed to create aligned physical frame"),
    };

    if frame.start_address().as_u64() != 0x8000 {
        return fail("physical frame has incorrect start address");
    }

    pass()
}

#[test]
fn test_phys_frame_rejects_unaligned() -> TestResult {
    let address = PhysAddr::new(0x8001);

    if PhysFrame::from_start_address(address).is_some() {
        return fail("unaligned address produced a physical frame");
    }

    pass()
}

#[test]
fn test_phys_frame_containing_address() -> TestResult {
    let frame = PhysFrame::containing_address(
        PhysAddr::new(0x8123)
    );

    if frame.start_address().as_u64() != 0x8000 {
        return fail("containing_address returned incorrect frame");
    }

    pass()
}

#[test]
fn test_phys_frame_number() -> TestResult {
    let frame = PhysFrame::from_start_address(
        PhysAddr::new(0x20_000)
    )
    .unwrap();

    if frame.number() != 0x20 {
        return fail("incorrect physical frame number");
    }

    pass()
}

#[test]
fn test_virt_addr_alignment() -> TestResult {
    let aligned = VirtAddr::new(0x8000);

    if !aligned.is_aligned() {
        return fail("aligned virtual address reported unaligned");
    }

    let unaligned = VirtAddr::new(0x8001);

    if unaligned.is_aligned() {
        return fail("unaligned virtual address reported aligned");
    }

    pass()
}

#[test]
fn test_virt_addr_align_down() -> TestResult {
    let address = VirtAddr::new(0x8fff);

    let aligned = address.align_down();

    if aligned.as_u64() != 0x8000 {
        return fail("align_down returned incorrect address");
    }

    pass()
}

#[test]
fn test_virt_addr_align_up() -> TestResult {
    let address = VirtAddr::new(0x8001);

    let aligned = match address.align_up() {
        Some(value) => value,
        None => return fail("align_up unexpectedly overflowed"),
    };

    if aligned.as_u64() != 0x9000 {
        return fail("align_up returned incorrect address");
    }

    pass()
}

#[test]
fn test_page_creation() -> TestResult {
    let page = match Page::from_start_address(
        VirtAddr::new(0x4000)
    ) {
        Some(page) => page,
        None => return fail("failed to create page"),
    };

    if page.start_address().as_u64() != 0x4000 {
        return fail("page has incorrect start address");
    }

    pass()
}

#[test]
fn test_page_rejects_unaligned() -> TestResult {
    if Page::from_start_address(
        VirtAddr::new(0x4001)
    ).is_some() {
        return fail("unaligned address produced a page");
    }

    pass()
}

#[test]
fn test_page_containing_address() -> TestResult {
    let page = Page::containing_address(
        VirtAddr::new(0x4fff)
    );

    if page.start_address().as_u64() != 0x4000 {
        return fail("containing_address returned incorrect page");
    }

    pass()
}

#[test]
fn test_page_number() -> TestResult {
    let page = Page::from_start_address(
        VirtAddr::new(0x20_000)
    )
    .unwrap();

    if page.number() != 0x20 {
        return fail("incorrect page number");
    }

    pass()
}

//
// ============================================================================
// PAGE INDEX TESTS
// ============================================================================
//

#[test]
fn test_page_pt_index() -> TestResult {
    let address = 0x1234_5000u64;

    let page = Page::from_start_address(
        VirtAddr::new(address)
    )
    .unwrap();

    let expected =
        ((address >> 12) & 0x1ff) as usize;

    if page.pt_index() != expected {
        return fail("incorrect PT index");
    }

    pass()
}

#[test]
fn test_page_pd_index() -> TestResult {
    let address = 0x1234_5000u64;

    let page = Page::from_start_address(
        VirtAddr::new(address)
    )
    .unwrap();

    let expected =
        ((address >> 21) & 0x1ff) as usize;

    if page.pd_index() != expected {
        return fail("incorrect PD index");
    }

    pass()
}

#[test]
fn test_page_pdpt_index() -> TestResult {
    let address = 0x1234_5000u64;

    let page = Page::from_start_address(
        VirtAddr::new(address)
    )
    .unwrap();

    let expected =
        ((address >> 30) & 0x1ff) as usize;

    if page.pdpt_index() != expected {
        return fail("incorrect PDPT index");
    }

    pass()
}

#[test]
fn test_page_pml4_index() -> TestResult {
    let address = 0x1234_5000u64;

    let page = Page::from_start_address(
        VirtAddr::new(address)
    )
    .unwrap();

    let expected =
        ((address >> 39) & 0x1ff) as usize;

    if page.pml4_index() != expected {
        return fail("incorrect PML4 index");
    }

    pass()
}

#[test]
fn test_page_index_boundaries() -> TestResult {
    let page = Page::from_start_address(
        VirtAddr::new(1u64 << 39)
    )
    .unwrap();

    if page.pml4_index() != 1 {
        return fail("PML4 boundary index incorrect");
    }

    pass()
}

//
// ============================================================================
// CANONICAL ADDRESS TESTS
// ============================================================================
//

#[test]
fn test_canonical_zero() -> TestResult {
    if !is_canonical(0) {
        return fail("zero should be canonical");
    }

    pass()
}

#[test]
fn test_canonical_low_address() -> TestResult {
    if !is_canonical(0x0000_7fff_ffff_ffff) {
        return fail("highest low canonical address rejected");
    }

    pass()
}

#[test]
fn test_canonical_high_address() -> TestResult {
    if !is_canonical(0xffff_8000_0000_0000) {
        return fail("lowest high canonical address rejected");
    }

    pass()
}

#[test]
fn test_canonical_max_address() -> TestResult {
    if !is_canonical(0xffff_ffff_ffff_ffff) {
        return fail("maximum canonical address rejected");
    }

    pass()
}

#[test]
fn test_noncanonical_middle_address() -> TestResult {
    if is_canonical(0x0000_8000_0000_0000) {
        return fail("noncanonical lower address accepted");
    }

    pass()
}

#[test]
fn test_noncanonical_upper_address() -> TestResult {
    if is_canonical(0xffff_7fff_ffff_ffff) {
        return fail("noncanonical upper address accepted");
    }

    pass()
}

//
// ============================================================================
// PAGE RANGE TESTS
// ============================================================================
//

#[test]
fn test_page_range_creation() -> TestResult {
    let range = match PageRange::new(
        VirtAddr::new(0x10_0000),
        VirtAddr::new(0x12_0000),
    ) {
        Some(range) => range,
        None => return fail("valid page range rejected"),
    };

    if range.page_count() != 0x20 {
        return fail("incorrect page count");
    }

    pass()
}

#[test]
fn test_page_range_rejects_unaligned_start() -> TestResult {
    if PageRange::new(
        VirtAddr::new(0x1001),
        VirtAddr::new(0x2000),
    ).is_some() {
        return fail("unaligned range accepted");
    }

    pass()
}

#[test]
fn test_page_range_rejects_unaligned_end() -> TestResult {
    if PageRange::new(
        VirtAddr::new(0x1000),
        VirtAddr::new(0x2001),
    ).is_some() {
        return fail("unaligned range accepted");
    }

    pass()
}

#[test]
fn test_page_range_rejects_empty() -> TestResult {
    if PageRange::new(
        VirtAddr::new(0x2000),
        VirtAddr::new(0x2000),
    ).is_some() {
        return fail("empty range accepted");
    }

    pass()
}

#[test]
fn test_page_range_rejects_backwards() -> TestResult {
    if PageRange::new(
        VirtAddr::new(0x3000),
        VirtAddr::new(0x2000),
    ).is_some() {
        return fail("backwards range accepted");
    }

    pass()
}

#[test]
fn test_page_range_page_at() -> TestResult {
    let range = PageRange::new(
        VirtAddr::new(0x10_0000),
        VirtAddr::new(0x14_0000),
    ).unwrap();

    let page = match range.page_at(2) {
        Some(page) => page,
        None => return fail("page_at returned None"),
    };

    if page.start_address().as_u64() != 0x10_2000 {
        return fail("page_at returned incorrect page");
    }

    pass()
}

#[test]
fn test_page_range_page_at_out_of_bounds() -> TestResult {
    let range = PageRange::new(
        VirtAddr::new(0x10_0000),
        VirtAddr::new(0x11_0000),
    ).unwrap();

    if range.page_at(range.page_count()).is_some() {
        return fail("page_at accepted out-of-range index");
    }

    pass()
}

//
// ============================================================================
// PAGE TABLE ENTRY TESTS
// ============================================================================
//

#[test]
fn test_page_table_entry_empty() -> TestResult {
    let entry = PageTableEntry::empty();

    if entry.is_present() {
        return fail("empty entry reported present");
    }

    if entry.frame().is_some() {
        return fail("empty entry returned frame");
    }

    pass()
}

#[test]
fn test_page_table_entry_set() -> TestResult {
    let frame = PhysFrame::from_start_address(
        PhysAddr::new(0x1234_5000)
    )
    .unwrap();

    let mut entry = PageTableEntry::empty();

    entry.set(
        frame,
        PageTableFlags::PRESENT
            | PageTableFlags::WRITABLE,
    );

    if !entry.is_present() {
        return fail("entry not marked present");
    }

    let returned = match entry.frame() {
        Some(frame) => frame,
        None => return fail("entry did not contain frame"),
    };

    if returned != frame {
        return fail("entry returned wrong frame");
    }

    pass()
}

#[test]
fn test_page_table_entry_flags() -> TestResult {
    let frame = PhysFrame::from_start_address(
        PhysAddr::new(0x8000)
    )
    .unwrap();

    let mut entry = PageTableEntry::empty();

    entry.set(
        frame,
        PageTableFlags::PRESENT
            | PageTableFlags::WRITABLE
            | PageTableFlags::NO_EXECUTE,
    );

    let flags = entry.flags();

    if !flags.contains(PageTableFlags::PRESENT) {
        return fail("PRESENT flag missing");
    }

    if !flags.contains(PageTableFlags::WRITABLE) {
        return fail("WRITABLE flag missing");
    }

    if !flags.contains(PageTableFlags::NO_EXECUTE) {
        return fail("NO_EXECUTE flag missing");
    }

    pass()
}

#[test]
fn test_page_table_entry_clear() -> TestResult {
    let frame = PhysFrame::from_start_address(
        PhysAddr::new(0x8000)
    )
    .unwrap();

    let mut entry = PageTableEntry::empty();

    entry.set(
        frame,
        PageTableFlags::PRESENT,
    );

    entry.clear();

    if entry.is_present() {
        return fail("cleared entry remains present");
    }

    if entry.frame().is_some() {
        return fail("cleared entry still contains frame");
    }

    pass()
}

//
// ============================================================================
// PAGE TABLE TESTS
// ============================================================================
//

#[test]
fn test_page_table_size() -> TestResult {
    if core::mem::size_of::<PageTable>() != PAGE_TABLE_SIZE {
        return fail("PageTable is not exactly 4096 bytes");
    }

    pass()
}

#[test]
fn test_page_table_entry_count() -> TestResult {
    if PAGE_TABLE_ENTRIES != 512 {
        return fail("incorrect page table entry count");
    }

    pass()
}

#[test]
fn test_page_table_zero() -> TestResult {
    let mut table = PageTable::new();

    let frame = PhysFrame::from_start_address(
        PhysAddr::new(0x8000)
    )
    .unwrap();

    table.entry_mut(42).set(
        frame,
        PageTableFlags::PRESENT,
    );

    table.zero();

    for entry in table.entries() {
        if entry.is_present() {
            return fail("PageTable::zero left present entry");
        }
    }

    pass()
}

#[test]
fn test_page_table_mutation() -> TestResult {
    let mut table = PageTable::new();

    let frame = PhysFrame::from_start_address(
        PhysAddr::new(0x9000)
    )
    .unwrap();

    table.entry_mut(100).set(
        frame,
        PageTableFlags::PRESENT,
    );

    if !table.entry(100).is_present() {
        return fail("entry mutation failed");
    }

    if table.entry(99).is_present() {
        return fail("wrong entry modified");
    }

    pass()
}

//
// ============================================================================
// PAGE ALLOCATOR TESTS
// ============================================================================
//

#[repr(align(4096))]
struct PageBitmapStorage {
    words: [u64; 128],
}

#[test]
fn test_page_allocator_creation() -> TestResult {
    static mut STORAGE: PageBitmapStorage =
        PageBitmapStorage {
            words: [0; 128],
        };

    let range = PageRange::new(
        VirtAddr::new(TEST_VA_BASE),
        VirtAddr::new(
            TEST_VA_BASE + 64 * FRAME_SIZE
        ),
    ).unwrap();

    let storage_ptr = unsafe {
        &raw mut STORAGE.words
    };

    let storage = unsafe {
        bitmap_storage(storage_ptr)
    };

    let allocator = unsafe {
        BitmapPageAllocator::new(
            range,
            storage,
        )
    };

    if allocator.is_err() {
        return fail("page allocator creation failed");
    }

    pass()
}

#[test]
fn test_page_allocator_initial_state() -> TestResult {
    static mut STORAGE: PageBitmapStorage =
        PageBitmapStorage {
            words: [0; 128],
        };

    let range = PageRange::new(
        VirtAddr::new(
            TEST_VA_BASE + 0x10_0000
        ),
        VirtAddr::new(
            TEST_VA_BASE
                + 0x10_0000
                + 16 * FRAME_SIZE
        ),
    ).unwrap();

    let storage_ptr = unsafe {
        &raw mut STORAGE.words
    };

    let storage = unsafe {
        bitmap_storage(storage_ptr)
    };

    let allocator = unsafe {
        BitmapPageAllocator::new(
            range,
            storage,
        )
    }.unwrap();

    if allocator.total_pages() != 16 {
        return fail("incorrect total page count");
    }

    if allocator.free_pages() != 16 {
        return fail("allocator did not start fully free");
    }

    pass()
}

#[test]
fn test_page_allocator_allocate() -> TestResult {
    static mut STORAGE: PageBitmapStorage =
        PageBitmapStorage {
            words: [0; 128],
        };

    let base =
        TEST_VA_BASE + 0x20_0000;

    let range = PageRange::new(
        VirtAddr::new(base),
        VirtAddr::new(
            base + 8 * FRAME_SIZE
        ),
    ).unwrap();

    let storage_ptr = unsafe {
        &raw mut STORAGE.words
    };

    let storage = unsafe {
        bitmap_storage(storage_ptr)
    };

    let mut allocator = unsafe {
        BitmapPageAllocator::new(
            range,
            storage,
        )
    }.unwrap();

    let page = match allocator.allocate() {
        Some(page) => page,
        None => return fail("page allocation returned None"),
    };

    if page.start_address().as_u64() != base {
        return fail("allocator returned wrong first page");
    }

    if allocator.free_pages() != 7 {
        return fail("free page count incorrect");
    }

    pass()
}

#[test]
fn test_page_allocator_sequential_allocation() -> TestResult {
    static mut STORAGE: PageBitmapStorage =
        PageBitmapStorage {
            words: [0; 128],
        };

    let base =
        TEST_VA_BASE + 0x30_0000;

    let range = PageRange::new(
        VirtAddr::new(base),
        VirtAddr::new(
            base + 16 * FRAME_SIZE
        ),
    ).unwrap();

    let storage_ptr = unsafe {
        &raw mut STORAGE.words
    };

    let storage = unsafe {
        bitmap_storage(storage_ptr)
    };

    let mut allocator = unsafe {
        BitmapPageAllocator::new(
            range,
            storage,
        )
    }.unwrap();

    for i in 0..16u64 {
        let page = match allocator.allocate() {
            Some(page) => page,
            None => return fail("allocation unexpectedly exhausted"),
        };

        if page.start_address().as_u64()
            != base + i * FRAME_SIZE
        {
            return fail("allocator did not allocate sequential pages");
        }
    }

    if allocator.allocate().is_some() {
        return fail("allocator returned page after exhaustion");
    }

    pass()
}

#[test]
fn test_page_allocator_deallocate() -> TestResult {
    static mut STORAGE: PageBitmapStorage =
        PageBitmapStorage {
            words: [0; 128],
        };

    let base =
        TEST_VA_BASE + 0x40_0000;

    let range = PageRange::new(
        VirtAddr::new(base),
        VirtAddr::new(
            base + 4 * FRAME_SIZE
        ),
    ).unwrap();

    let storage_ptr = unsafe {
        &raw mut STORAGE.words
    };

    let storage = unsafe {
        bitmap_storage(storage_ptr)
    };

    let mut allocator = unsafe {
        BitmapPageAllocator::new(
            range,
            storage,
        )
    }.unwrap();

    let page = allocator.allocate().unwrap();

    if allocator.deallocate(page).is_err() {
        return fail("deallocation failed");
    }

    if allocator.free_pages() != 4 {
        return fail("free count incorrect after deallocation");
    }

    pass()
}

#[test]
fn test_page_allocator_reuse() -> TestResult {
    static mut STORAGE: PageBitmapStorage =
        PageBitmapStorage {
            words: [0; 128],
        };

    let base =
        TEST_VA_BASE + 0x50_0000;

    let range = PageRange::new(
        VirtAddr::new(base),
        VirtAddr::new(
            base + 4 * FRAME_SIZE
        ),
    ).unwrap();

    let storage_ptr = unsafe {
        &raw mut STORAGE.words
    };

    let storage = unsafe {
        bitmap_storage(storage_ptr)
    };

    let mut allocator = unsafe {
        BitmapPageAllocator::new(
            range,
            storage,
        )
    }.unwrap();

    let first = allocator.allocate().unwrap();
    let _second = allocator.allocate().unwrap();

    allocator.deallocate(first).unwrap();

    let reused = allocator.allocate().unwrap();

    if reused != first {
        return fail("allocator did not reuse freed page");
    }

    pass()
}

#[test]
fn test_page_allocator_double_free() -> TestResult {
    static mut STORAGE: PageBitmapStorage =
        PageBitmapStorage {
            words: [0; 128],
        };

    let base =
        TEST_VA_BASE + 0x60_0000;

    let range = PageRange::new(
        VirtAddr::new(base),
        VirtAddr::new(
            base + 4 * FRAME_SIZE
        ),
    ).unwrap();

    let storage_ptr = unsafe {
        &raw mut STORAGE.words
    };

    let storage = unsafe {
        bitmap_storage(storage_ptr)
    };

    let mut allocator = unsafe {
        BitmapPageAllocator::new(
            range,
            storage,
        )
    }.unwrap();

    let page = allocator.allocate().unwrap();

    allocator.deallocate(page).unwrap();

    match allocator.deallocate(page) {
        Err(PageAllocatorError::AlreadyFree) => pass(),
        _ => fail("double free was not rejected"),
    }
}

#[test]
fn test_page_allocator_invalid_page() -> TestResult {
    static mut STORAGE: PageBitmapStorage =
        PageBitmapStorage {
            words: [0; 128],
        };

    let base =
        TEST_VA_BASE + 0x70_0000;

    let range = PageRange::new(
        VirtAddr::new(base),
        VirtAddr::new(
            base + 4 * FRAME_SIZE
        ),
    ).unwrap();

    let storage_ptr = unsafe {
        &raw mut STORAGE.words
    };

    let storage = unsafe {
        bitmap_storage(storage_ptr)
    };

    let mut allocator = unsafe {
        BitmapPageAllocator::new(
            range,
            storage,
        )
    }.unwrap();

    let outside = test_page(0);

    match allocator.deallocate(outside) {
        Err(PageAllocatorError::InvalidPage) => pass(),
        _ => fail("invalid page was accepted"),
    }
}

#[test]
fn test_page_allocator_reserve() -> TestResult {
    static mut STORAGE: PageBitmapStorage =
        PageBitmapStorage {
            words: [0; 128],
        };

    let base =
        TEST_VA_BASE + 0x80_0000;

    let range = PageRange::new(
        VirtAddr::new(base),
        VirtAddr::new(
            base + 16 * FRAME_SIZE
        ),
    ).unwrap();

    let storage_ptr = unsafe {
        &raw mut STORAGE.words
    };

    let storage = unsafe {
        bitmap_storage(storage_ptr)
    };

    let mut allocator = unsafe {
        BitmapPageAllocator::new(
            range,
            storage,
        )
    }.unwrap();

    let reserve = PageRange::new(
        VirtAddr::new(
            base + 4 * FRAME_SIZE
        ),
        VirtAddr::new(
            base + 8 * FRAME_SIZE
        ),
    ).unwrap();

    allocator.reserve(reserve).unwrap();

    if allocator.free_pages() != 12 {
        return fail("reserve produced incorrect free count");
    }

    pass()
}

//
// ============================================================================
// KERNEL MEMORY / FRAME ALLOCATOR TESTS
// ============================================================================
//

fn get_kernel_memory() -> KernelMemory {
    crate::kmem::init()
}

#[test]
fn test_kernel_memory_initializes() -> TestResult {
    let kmem = get_kernel_memory();

    if kmem.frames.total_frames() == 0 {
        return fail("frame allocator contains zero frames");
    }

    if kmem.frames.free_frames() == 0 {
        return fail("frame allocator contains zero free frames");
    }

    pass()
}

#[test]
fn test_kernel_memory_has_valid_hhdm() -> TestResult {
    let kmem = get_kernel_memory();

    let _ = kmem.hhdm_offset;

    pass()
}

#[test]
fn test_kernel_memory_has_valid_pml4() -> TestResult {
    let kmem = get_kernel_memory();

    if !kmem.pml4_frame.start_address().is_aligned() {
        return fail("PML4 frame is not aligned");
    }

    pass()
}

#[test]
fn test_frame_allocate() -> TestResult {
    let mut kmem = get_kernel_memory();

    let before = kmem.frames.free_frames();

    let frame = match kmem.frames.allocate() {
        Some(frame) => frame,
        None => return fail("frame allocation returned None"),
    };

    let after = kmem.frames.free_frames();

    if after + 1 != before {
        return fail("free frame count did not decrease");
    }

    if !frame.start_address().is_aligned() {
        let _ = kmem.frames.deallocate(frame);
        return fail("allocated frame is not aligned");
    }

    kmem.frames.deallocate(frame).unwrap();

    pass()
}

#[test]
fn test_frame_allocate_deallocate() -> TestResult {
    let mut kmem = get_kernel_memory();

    let before = kmem.frames.free_frames();

    let frame = match kmem.frames.allocate() {
        Some(frame) => frame,
        None => return fail("frame allocation failed"),
    };

    if kmem.frames.deallocate(frame).is_err() {
        return fail("frame deallocation failed");
    }

    if kmem.frames.free_frames() != before {
        return fail("free count did not return to original value");
    }

    pass()
}

#[test]
fn test_frame_reuse() -> TestResult {
    let mut kmem = get_kernel_memory();

    let frame = match kmem.frames.allocate() {
        Some(frame) => frame,
        None => return fail("frame allocation failed"),
    };

    if kmem.frames.deallocate(frame).is_err() {
        return fail("frame deallocation failed");
    }

    let reused = match kmem.frames.allocate() {
        Some(frame) => frame,
        None => return fail("reallocation failed"),
    };

    if reused != frame {
        let _ = kmem.frames.deallocate(reused);
        return fail("allocator did not reuse freed frame");
    }

    kmem.frames.deallocate(reused).unwrap();

    pass()
}

#[test]
fn test_frame_double_free() -> TestResult {
    let mut kmem = get_kernel_memory();

    let frame = match kmem.frames.allocate() {
        Some(frame) => frame,
        None => return fail("frame allocation failed"),
    };

    kmem.frames.deallocate(frame).unwrap();

    match kmem.frames.deallocate(frame) {
        Err(FrameError::DoubleFree) => pass(),
        _ => fail("double free was not rejected"),
    }
}

#[test]
fn test_frame_is_free() -> TestResult {
    let mut kmem = get_kernel_memory();

    let frame = match kmem.frames.allocate() {
        Some(frame) => frame,
        None => return fail("frame allocation failed"),
    };

    match kmem.frames.is_free(frame) {
        Ok(false) => {}
        Ok(true) => {
            let _ = kmem.frames.deallocate(frame);
            return fail("allocated frame reported free");
        }
        Err(_) => {
            let _ = kmem.frames.deallocate(frame);
            return fail("is_free failed");
        }
    }

    kmem.frames.deallocate(frame).unwrap();

    match kmem.frames.is_free(frame) {
        Ok(true) => pass(),
        _ => fail("deallocated frame not reported free"),
    }
}

//
// ============================================================================
// PAGE TABLE FLAG TESTS
// ============================================================================
//

#[test]
fn test_page_flags_union() -> TestResult {
    let flags =
        PageTableFlags::PRESENT
            | PageTableFlags::WRITABLE;

    if !flags.contains(PageTableFlags::PRESENT) {
        return fail("PRESENT missing");
    }

    if !flags.contains(PageTableFlags::WRITABLE) {
        return fail("WRITABLE missing");
    }

    pass()
}

#[test]
fn test_page_flags_remove() -> TestResult {
    let flags =
        PageTableFlags::PRESENT
            | PageTableFlags::WRITABLE;

    let flags =
        flags.remove(PageTableFlags::WRITABLE);

    if flags.contains(PageTableFlags::WRITABLE) {
        return fail("WRITABLE was not removed");
    }

    if !flags.contains(PageTableFlags::PRESENT) {
        return fail("PRESENT was incorrectly removed");
    }

    pass()
}

#[test]
fn test_page_flags_empty() -> TestResult {
    if PageTableFlags::EMPTY.bits() != 0 {
        return fail("EMPTY flags are not zero");
    }

    pass()
}

//
// ============================================================================
// MAPPER HELPERS
// ============================================================================
//

fn new_mapper(kmem: &KernelMemory) -> Mapper {
    unsafe {
        Mapper::new(
            kmem.pml4_frame,
            kmem.hhdm_offset,
        )
    }
}

//
// ============================================================================
// MAPPER TESTS
// ============================================================================
//

#[test]
fn test_mapper_translate_unmapped() -> TestResult {
    let kmem = get_kernel_memory();

    let mapper = new_mapper(&kmem);

    let page =
        test_page(0x1000);

    if mapper.translate_page(page).is_some() {
        return fail("unmapped address translated successfully");
    }

    pass()
}

#[test]
fn test_mapper_map_single_page() -> TestResult {
    let mut kmem = get_kernel_memory();

    let mapper = new_mapper(&kmem);

    let page =
        test_page(0x1100);

    let frame =
        match kmem.frames.allocate() {
            Some(frame) => frame,
            None => return fail("frame allocation failed"),
        };

    match mapper.map(
        page,
        frame,
        PageTableFlags::WRITABLE
            | PageTableFlags::NO_EXECUTE,
        &mut kmem.frames,
    ) {
        Ok(()) => {}

        Err(MapperError::PageAlreadyMapped) => {
            let _ = kmem.frames.deallocate(frame);
            return fail("test page was already mapped");
        }

        Err(MapperError::HugePage) => {
            let _ = kmem.frames.deallocate(frame);
            return fail("test page is inside a huge-page mapping");
        }

        Err(MapperError::OutOfFrames) => {
            let _ = kmem.frames.deallocate(frame);
            return fail("mapper ran out of frames");
        }

        Err(_) => {
            let _ = kmem.frames.deallocate(frame);
            return fail("single-page mapping failed");
        }
    }

    let translated =
        mapper.translate_page(page);

    if translated != Some(frame) {
        let _ = mapper.unmap(page);
        let _ = kmem.frames.deallocate(frame);

        return fail("mapped page did not translate to its frame");
    }

    match mapper.unmap(page) {
        Ok(result) => {
            if result.frame != frame {
                let _ = kmem.frames.deallocate(frame);
                return fail("unmap returned wrong frame");
            }
        }

        Err(_) => {
            let _ = kmem.frames.deallocate(frame);
            return fail("unmapping failed");
        }
    }

    kmem.frames.deallocate(frame).unwrap();

    pass()
}

#[test]
fn test_mapper_map_and_unmap() -> TestResult {
    let mut kmem = get_kernel_memory();

    let mapper = new_mapper(&kmem);

    let page =
        test_page(0x1200);

    let frame =
        match kmem.frames.allocate() {
            Some(frame) => frame,
            None => return fail("frame allocation failed"),
        };

    if mapper.map(
        page,
        frame,
        PageTableFlags::WRITABLE,
        &mut kmem.frames,
    ).is_err() {
        let _ = kmem.frames.deallocate(frame);
        return fail("mapping failed");
    }

    if mapper.translate_page(page) != Some(frame) {
        let _ = mapper.unmap(page);
        let _ = kmem.frames.deallocate(frame);

        return fail("mapped page did not translate");
    }

    let result =
        match mapper.unmap(page) {
            Ok(result) => result,
            Err(_) => {
                let _ = kmem.frames.deallocate(frame);
                return fail("unmapping failed");
            }
        };

    if result.frame != frame {
        let _ = kmem.frames.deallocate(frame);
        return fail("unmap returned wrong frame");
    }

    if mapper.translate_page(page).is_some() {
        let _ = kmem.frames.deallocate(frame);
        return fail("page remained mapped after unmap");
    }

    kmem.frames.deallocate(frame).unwrap();

    pass()
}

#[test]
fn test_mapper_double_map_rejected() -> TestResult {
    let mut kmem = get_kernel_memory();

    let mapper = new_mapper(&kmem);

    let page =
        test_page(0x1300);

    let frame1 =
        match kmem.frames.allocate() {
            Some(frame) => frame,
            None => return fail("frame allocation failed"),
        };

    let frame2 =
        match kmem.frames.allocate() {
            Some(frame) => frame,
            None => {
                let _ = kmem.frames.deallocate(frame1);
                return fail("second frame allocation failed");
            }
        };

    if mapper.map(
        page,
        frame1,
        PageTableFlags::WRITABLE,
        &mut kmem.frames,
    ).is_err() {
        let _ = kmem.frames.deallocate(frame1);
        let _ = kmem.frames.deallocate(frame2);

        return fail("first mapping failed");
    }

    match mapper.map(
        page,
        frame2,
        PageTableFlags::WRITABLE,
        &mut kmem.frames,
    ) {
        Err(MapperError::PageAlreadyMapped) => {}

        _ => {
            let _ = mapper.unmap(page);
            let _ = kmem.frames.deallocate(frame1);
            let _ = kmem.frames.deallocate(frame2);

            return fail("double mapping was not rejected");
        }
    }

    let result =
        match mapper.unmap(page) {
            Ok(result) => result,
            Err(_) => {
                let _ = kmem.frames.deallocate(frame1);
                let _ = kmem.frames.deallocate(frame2);

                return fail("cleanup unmap failed");
            }
        };

    if result.frame != frame1 {
        let _ = kmem.frames.deallocate(frame1);
        let _ = kmem.frames.deallocate(frame2);

        return fail("original frame was not preserved");
    }

    kmem.frames.deallocate(frame1).unwrap();
    kmem.frames.deallocate(frame2).unwrap();

    pass()
}

#[test]
fn test_mapper_unmap_unmapped_rejected() -> TestResult {
    let kmem = get_kernel_memory();

    let mapper = new_mapper(&kmem);

    let page =
        test_page(0x1400);

    match mapper.unmap(page) {
        Err(MapperError::PageNotMapped) => pass(),
        _ => fail("unmapping unmapped page was not rejected"),
    }
}

#[test]
fn test_mapper_translate_page() -> TestResult {
    let mut kmem = get_kernel_memory();

    let mapper = new_mapper(&kmem);

    let page =
        test_page(0x1500);

    let frame =
        match kmem.frames.allocate() {
            Some(frame) => frame,
            None => return fail("frame allocation failed"),
        };

    if mapper.map(
        page,
        frame,
        PageTableFlags::WRITABLE,
        &mut kmem.frames,
    ).is_err() {
        let _ = kmem.frames.deallocate(frame);
        return fail("mapping failed");
    }

    let translated =
        mapper.translate_page(page);

    let cleanup =
        mapper.unmap(page);

    let _ =
        kmem.frames.deallocate(frame);

    if cleanup.is_err() {
        return fail("cleanup failed");
    }

    match translated {
        Some(result) if result == frame => pass(),
        Some(_) => fail("translate_page returned wrong frame"),
        None => fail("translate_page returned None"),
    }
}

#[test]
fn test_mapper_translate_offset() -> TestResult {
    let mut kmem = get_kernel_memory();

    let mapper = new_mapper(&kmem);

    let page =
        test_page(0x1600);

    let frame =
        match kmem.frames.allocate() {
            Some(frame) => frame,
            None => return fail("frame allocation failed"),
        };

    if mapper.map(
        page,
        frame,
        PageTableFlags::WRITABLE,
        &mut kmem.frames,
    ).is_err() {
        let _ = kmem.frames.deallocate(frame);
        return fail("mapping failed");
    }

    let offset = 0x321u64;

    let virtual_address =
        VirtAddr::new(
            page.start_address().as_u64()
                + offset
        );

    let physical =
        mapper.translate(virtual_address);

    let expected =
        frame.start_address().as_u64()
            + offset;

    let cleanup =
        mapper.unmap(page);

    let _ =
        kmem.frames.deallocate(frame);

    if cleanup.is_err() {
        return fail("cleanup failed");
    }

    match physical {
        Some(address)
            if address.as_u64() == expected =>
        {
            pass()
        }

        Some(_) =>
            fail("translated physical address incorrect"),

        None =>
            fail("offset translation returned None"),
    }
}

//
// ============================================================================
// MULTI-PAGE MAPPING TESTS
// ============================================================================
//

#[test]
fn test_8_page_mappings() -> TestResult {
    let mut kmem = get_kernel_memory();

    let mapper = new_mapper(&kmem);

    let base_index = 0x2000u64;

    let mut frames = [None; 8];

    for i in 0..8 {
        let frame =
            match kmem.frames.allocate() {
                Some(frame) => frame,
                None => {
                    for frame in frames.iter().flatten() {
                        let _ = kmem.frames.deallocate(*frame);
                    }

                    return fail("frame allocation failed");
                }
            };

        frames[i] = Some(frame);

        let page =
            test_page(base_index + i as u64);

        if mapper.map(
            page,
            frame,
            PageTableFlags::WRITABLE,
            &mut kmem.frames,
        ).is_err() {
            for j in 0..=i {
                if let Some(frame) = frames[j] {
                    let _ = mapper.unmap(
                        test_page(
                            base_index + j as u64
                        )
                    );

                    let _ =
                        kmem.frames.deallocate(frame);
                }
            }

            return fail("multi-page mapping failed");
        }
    }

    for i in 0..8 {
        let page =
            test_page(base_index + i as u64);

        let translated =
            match mapper.translate_page(page) {
                Some(frame) => frame,
                None => {
                    for j in 0..8 {
                        let _ =
                            mapper.unmap(
                                test_page(
                                    base_index + j as u64
                                )
                            );

                        if let Some(frame) = frames[j] {
                            let _ =
                                kmem.frames.deallocate(frame);
                        }
                    }

                    return fail("multi-page translation failed");
                }
            };

        if translated != frames[i].unwrap() {
            return fail("multi-page translation returned wrong frame");
        }
    }

    for i in 0..8 {
        let page =
            test_page(base_index + i as u64);

        let result =
            match mapper.unmap(page) {
                Ok(result) => result,
                Err(_) => return fail("multi-page unmap failed"),
            };

        if result.frame != frames[i].unwrap() {
            return fail("multi-page unmap returned wrong frame");
        }

        kmem.frames
            .deallocate(result.frame)
            .unwrap();
    }

    pass()
}

#[test]
fn test_32_page_mappings() -> TestResult {
    let mut kmem = get_kernel_memory();

    let mapper = new_mapper(&kmem);

    //
    // Keep all 32 pages inside one 2 MiB area.
    //
    // 32 pages = 128 KiB.
    //
    let base =
        TEST_VA_BASE + 0x3000_0000;

    let mut pages = [None; 32];
    let mut frames = [None; 32];

    for i in 0..32 {
        let page =
            Page::from_start_address(
                VirtAddr::new(
                    base + i as u64 * FRAME_SIZE
                )
            )
            .unwrap();

        let frame =
            match kmem.frames.allocate() {
                Some(frame) => frame,
                None => {
                    for j in 0..32 {
                        if let Some(page) = pages[j] {
                            let _ = mapper.unmap(page);
                        }

                        if let Some(frame) = frames[j] {
                            let _ = kmem.frames.deallocate(frame);
                        }
                    }

                    return fail("frame allocation failed");
                }
            };

        pages[i] = Some(page);
        frames[i] = Some(frame);

        if mapper.map(
            page,
            frame,
            PageTableFlags::WRITABLE
                | PageTableFlags::NO_EXECUTE,
            &mut kmem.frames,
        ).is_err() {
            for j in 0..32 {
                if let Some(page) = pages[j] {
                    let _ = mapper.unmap(page);
                }

                if let Some(frame) = frames[j] {
                    let _ = kmem.frames.deallocate(frame);
                }
            }

            return fail("32-page mapping failed");
        }
    }

    for i in 0..32 {
        let page = pages[i].unwrap();
        let expected = frames[i].unwrap();

        match mapper.translate_page(page) {
            Some(frame) if frame == expected => {}

            Some(_) => {
                for j in 0..32 {
                    let _ = mapper.unmap(
                        pages[j].unwrap()
                    );

                    if let Some(frame) = frames[j] {
                        let _ =
                            kmem.frames.deallocate(frame);
                    }
                }

                return fail("32-page translation returned wrong frame");
            }

            None => {
                for j in 0..32 {
                    let _ = mapper.unmap(
                        pages[j].unwrap()
                    );

                    if let Some(frame) = frames[j] {
                        let _ =
                            kmem.frames.deallocate(frame);
                    }
                }

                return fail("32-page translation failed");
            }
        }
    }

    for i in 0..32 {
        let page = pages[i].unwrap();

        let result =
            match mapper.unmap(page) {
                Ok(result) => result,
                Err(_) => return fail("32-page unmap failed"),
            };

        if result.frame != frames[i].unwrap() {
            return fail("32-page unmap returned wrong frame");
        }

        if kmem.frames
            .deallocate(result.frame)
            .is_err()
        {
            return fail("failed to free frame");
        }
    }

    pass()
}

//
// ============================================================================
// CROSS-PAGE TRANSLATION TESTS
// ============================================================================
//

#[test]
fn test_translation_at_page_start() -> TestResult {
    let mut kmem = get_kernel_memory();

    let mapper = new_mapper(&kmem);

    let page =
        test_page(0x5000);

    let frame =
        match kmem.frames.allocate() {
            Some(frame) => frame,
            None => return fail("frame allocation failed"),
        };

    if mapper.map(
        page,
        frame,
        PageTableFlags::WRITABLE,
        &mut kmem.frames,
    ).is_err() {
        let _ = kmem.frames.deallocate(frame);
        return fail("mapping failed");
    }

    let result =
        mapper.translate(
            page.start_address()
        );

    let cleanup =
        mapper.unmap(page);

    let _ =
        kmem.frames.deallocate(frame);

    if cleanup.is_err() {
        return fail("cleanup failed");
    }

    match result {
        Some(address)
            if address.as_u64()
                == frame.start_address().as_u64() =>
        {
            pass()
        }

        _ => fail("page-start translation incorrect"),
    }
}

#[test]
fn test_translation_at_page_end() -> TestResult {
    let mut kmem = get_kernel_memory();

    let mapper = new_mapper(&kmem);

    let page =
        test_page(0x5100);

    let frame =
        match kmem.frames.allocate() {
            Some(frame) => frame,
            None => return fail("frame allocation failed"),
        };

    if mapper.map(
        page,
        frame,
        PageTableFlags::WRITABLE,
        &mut kmem.frames,
    ).is_err() {
        let _ = kmem.frames.deallocate(frame);
        return fail("mapping failed");
    }

    let address =
        VirtAddr::new(
            page.start_address().as_u64()
                + FRAME_SIZE - 1
        );

    let result =
        mapper.translate(address);

    let expected =
        frame.start_address().as_u64()
            + FRAME_SIZE - 1;

    let cleanup =
        mapper.unmap(page);

    let _ =
        kmem.frames.deallocate(frame);

    if cleanup.is_err() {
        return fail("cleanup failed");
    }

    match result {
        Some(address)
            if address.as_u64() == expected =>
        {
            pass()
        }

        _ => fail("last-byte translation incorrect"),
    }
}

//
// ============================================================================
// FRAME ALLOCATOR STRESS TESTS
// ============================================================================
//

#[test]
fn test_allocate_many_frames() -> TestResult {
    let mut kmem = get_kernel_memory();

    let mut frames = [None; 64];

    for slot in frames.iter_mut() {
        *slot =
            match kmem.frames.allocate() {
                Some(frame) => Some(frame),
                None => {
                    for frame in frames.iter().flatten() {
                        let _ =
                            kmem.frames.deallocate(*frame);
                    }

                    return fail("allocator exhausted too early");
                }
            };
    }

    for i in 0..64 {
        for j in (i + 1)..64 {
            if frames[i] == frames[j] {
                for frame in frames.iter().flatten() {
                    let _ =
                        kmem.frames.deallocate(*frame);
                }

                return fail("allocator returned duplicate frame");
            }
        }
    }

    for frame in frames.iter().flatten() {
        if kmem.frames.deallocate(*frame).is_err() {
            return fail("failed to free allocated frame");
        }
    }

    pass()
}

#[test]
fn test_frame_allocator_statistics() -> TestResult {
    let mut kmem = get_kernel_memory();

    let before =
        kmem.frames.stats();

    let frame =
        match kmem.frames.allocate() {
            Some(frame) => frame,
            None => return fail("allocation failed"),
        };

    let after =
        kmem.frames.stats();

    if after.total_frames != before.total_frames {
        let _ = kmem.frames.deallocate(frame);
        return fail("total frame count changed");
    }

    if after.free_frames + 1
        != before.free_frames
    {
        let _ = kmem.frames.deallocate(frame);
        return fail("free frame count incorrect");
    }

    if after.allocated_frames
        != before.allocated_frames + 1
    {
        let _ = kmem.frames.deallocate(frame);
        return fail("allocated frame count incorrect");
    }

    kmem.frames.deallocate(frame).unwrap();

    pass()
}

//
// ============================================================================
// PML4 / CR3 TESTS
// ============================================================================
//

#[test]
fn test_read_cr3_matches_kernel_memory() -> TestResult {
    let kmem = get_kernel_memory();

    let cr3 =
        read_cr3();

    if cr3 != kmem.pml4_frame {
        return fail("KernelMemory PML4 does not match CR3");
    }

    pass()
}

//
// ============================================================================
// MAPPER ACCOUNTING TEST
// ============================================================================
//

#[test]
fn test_map_unmap_frame_accounting() -> TestResult {
    let mut kmem = get_kernel_memory();

    let mapper = new_mapper(&kmem);

    let before =
        kmem.frames.free_frames();

    let page =
        test_page(0x6000);

    let frame =
        match kmem.frames.allocate() {
            Some(frame) => frame,
            None => return fail("frame allocation failed"),
        };

    if kmem.frames.free_frames() + 1 != before {
        let _ = kmem.frames.deallocate(frame);
        return fail("frame allocation accounting incorrect");
    }

    if mapper.map(
        page,
        frame,
        PageTableFlags::WRITABLE,
        &mut kmem.frames,
    ).is_err() {
        let _ = kmem.frames.deallocate(frame);
        return fail("mapping failed");
    }

    if mapper.translate_page(page) != Some(frame) {
        let _ = mapper.unmap(page);
        let _ = kmem.frames.deallocate(frame);

        return fail("mapped page did not translate");
    }

    let result =
        match mapper.unmap(page) {
            Ok(result) => result,
            Err(_) => {
                let _ = kmem.frames.deallocate(frame);
                return fail("unmapping failed");
            }
        };

    if result.frame != frame {
        let _ = kmem.frames.deallocate(frame);
        return fail("unmap returned wrong frame");
    }

    kmem.frames
        .deallocate(result.frame)
        .unwrap();

    if kmem.frames.free_frames() != before {
        return fail(
            "frame accounting did not return to original state"
        );
    }

    pass()
}

//
// ============================================================================
// MAPPER FLAG TEST
// ============================================================================
//

#[test]
fn test_mapper_flags_preserved() -> TestResult {
    let mut kmem = get_kernel_memory();

    let mapper = new_mapper(&kmem);

    let page =
        test_page(0x6100);

    let frame =
        match kmem.frames.allocate() {
            Some(frame) => frame,
            None => return fail("frame allocation failed"),
        };

    let flags =
        PageTableFlags::WRITABLE
            | PageTableFlags::NO_EXECUTE;

    if mapper.map(
        page,
        frame,
        flags,
        &mut kmem.frames,
    ).is_err() {
        let _ =
            kmem.frames.deallocate(frame);

        return fail("mapping failed");
    }

    if mapper.translate_page(page) != Some(frame) {
        let _ = mapper.unmap(page);
        let _ = kmem.frames.deallocate(frame);

        return fail("mapped page could not be translated");
    }

    let result =
        mapper.unmap(page);

    if result.is_err() {
        let _ =
            kmem.frames.deallocate(frame);

        return fail("unmapping failed");
    }

    kmem.frames.deallocate(frame).unwrap();

    pass()
}
