//! Kernel memory management.
//!
//! This module provides the foundations for kernel memory management:
//!
//! - Limine HHDM access
//! - Physical frame allocation
//! - Page-table mapping
//! - Kernel heap initialization
//!
//! The individual allocators live in their own modules.

pub mod frame;
pub mod heap;
mod memtests;

pub use frame::BootInfoFrameAllocator;
pub use heap::{init_heap, LockedHeap, HEAP_SIZE, HEAP_START};

use limine::request::{HhdmRequest, MemmapRequest};
use x86_64::{
    registers::control::Cr3,
    structures::paging::{OffsetPageTable, PageTable},
    VirtAddr,
};

use crate::console_println_color;

/// Limine HHDM request.
///
/// The HHDM maps physical memory into a contiguous virtual address range.
#[used]
#[unsafe(link_section = ".limine_reqs")]
pub static HHDM_REQUEST: HhdmRequest = HhdmRequest::new();

/// Limine memory map request.
#[used]
#[unsafe(link_section = ".limine_reqs")]
pub static MEMMAP_REQUEST: MemmapRequest = MemmapRequest::new();

/// Initialize the kernel memory subsystem.
///
/// This initializes:
///
/// 1. The physical-memory offset supplied by Limine.
/// 2. An `OffsetPageTable` using the currently active page table.
/// 3. The physical frame allocator.
///
/// The heap is initialized separately using [`init_heap`].
///
/// # Safety
///
/// The caller must ensure that:
///
/// - Limine's HHDM response exists.
/// - Limine's memory map response exists.
/// - The current CR3 contains a valid level-4 page table.
/// - The HHDM covers the physical memory that will be accessed.
pub unsafe fn init() -> (
    OffsetPageTable<'static>,
    BootInfoFrameAllocator,
) {
    let hhdm = HHDM_REQUEST
        .response()
        .expect("Limine did not provide an HHDM response");

    let memory_map = MEMMAP_REQUEST
        .response()
        .expect("Limine did not provide a memory-map response");

    let physical_memory_offset =
        VirtAddr::new(hhdm.offset);

    let mapper = unsafe {
        init_mapper(physical_memory_offset)
    };

    let frame_allocator = unsafe {
        BootInfoFrameAllocator::new(memory_map.entries())
    };

    (mapper, frame_allocator)
}

/// Initialize an `OffsetPageTable` using the currently active page table.
///
/// Limine's HHDM gives us a direct mapping from physical memory to virtual
/// memory, which is exactly what `OffsetPageTable` requires.
///
/// # Safety
///
/// The supplied offset must point to a valid HHDM mapping and the currently
/// active CR3 must contain a valid level-4 page table.
pub unsafe fn init_mapper(
    physical_memory_offset: VirtAddr,
) -> OffsetPageTable<'static> {
    let (level_4_frame, _) = Cr3::read();

    let level_4_table_addr =
        physical_memory_offset
            + level_4_frame
                .start_address()
                .as_u64();

    let level_4_table_ptr =
        level_4_table_addr
            .as_mut_ptr::<PageTable>();

    let level_4_table = unsafe {
        &mut *level_4_table_ptr
    };

    unsafe {
        OffsetPageTable::new(
            level_4_table,
            physical_memory_offset,
        )
    }
}

/// Convert a physical address into its HHDM virtual address.
///
/// # Safety
///
/// The supplied physical address must be backed by memory mapped by
/// Limine's HHDM.
pub unsafe fn phys_to_virt(
    physical_address: u64,
) -> VirtAddr {
    let hhdm = HHDM_REQUEST
        .response()
        .expect("Limine did not provide an HHDM response");

    VirtAddr::new(
        hhdm.offset
            .checked_add(physical_address)
            .expect("HHDM address overflow"),
    )
}

/// Convert a physical address into a pointer through the HHDM.
///
/// # Safety
///
/// The physical address must refer to valid memory covered by the HHDM.
///
/// The returned pointer is only valid while the corresponding physical
/// memory remains mapped.
pub unsafe fn phys_to_ptr<T>(
    physical_address: u64,
) -> *mut T {
    unsafe {
        phys_to_virt(physical_address)
            .as_mut_ptr::<T>()
    }
}

/// Print a detailed summary of the kernel's memory configuration.
///
/// This is intended for debugging and early kernel development. It does
/// not modify the memory-management state.
pub fn mem_analyze() {
    use crate::fb::Color;

    console_println_color!(
        Color::GREEN,
        ""
    );

    console_println_color!(
        Color::GREEN,
        "========================================"
    );

    console_println_color!(
        Color::GREEN,
        "       KERNEL MEMORY ANALYSIS"
    );

    console_println_color!(
        Color::GREEN,
        "========================================"
    );

    // --------------------------------------------------------
    // HHDM
    // --------------------------------------------------------

    let hhdm =
        match HHDM_REQUEST.response() {
            Some(response) => response,
            None => {
                console_println_color!(
                    Color::GREEN,
                    "HHDM: NOT AVAILABLE"
                );

                console_println_color!(
                    Color::GREEN,
                    "========================================"
                );

                return;
            }
        };

    console_println_color!(
        Color::GREEN,
        ""
    );

    console_println_color!(
        Color::GREEN,
        "[HHDM]"
    );

    console_println_color!(
        Color::GREEN,
        "  Offset:       {:#018x}",
        hhdm.offset
    );

    // --------------------------------------------------------
    // Memory map
    // --------------------------------------------------------

    let memory_map =
        match MEMMAP_REQUEST.response() {
            Some(response) => response,
            None => {
                console_println_color!(
                    Color::GREEN,
                    "Memory map: NOT AVAILABLE"
                );

                console_println_color!(
                    Color::GREEN,
                    "========================================"
                );

                return;
            }
        };

    let entries =
        memory_map.entries();

    console_println_color!(
        Color::GREEN,
        ""
    );

    console_println_color!(
        Color::GREEN,
        "[MEMORY MAP]"
    );

    console_println_color!(
        Color::GREEN,
        "  Entries:      {}",
        entries.len()
    );

    let mut total_memory: u64 = 0;
    let mut usable_memory: u64 = 0;
    let mut reserved_memory: u64 = 0;
    let mut reclaimable_memory: u64 = 0;
    let mut bootloader_memory: u64 = 0;
    let mut kernel_and_modules: u64 = 0;
    let mut bad_memory: u64 = 0;
    let mut framebuffer_memory: u64 = 0;
    let mut other_memory: u64 = 0;

    let mut usable_regions: usize = 0;

    let mut first_usable_base: Option<u64> = None;
    let mut last_usable_end: Option<u64> = None;

    for entry in entries {
        let base = entry.base;
        let length = entry.length;

        let end =
            base.saturating_add(length);

        total_memory =
            total_memory.saturating_add(length);

        match entry.type_ {
            limine::memmap::MEMMAP_USABLE => {
                usable_memory =
                    usable_memory.saturating_add(length);

                usable_regions += 1;

                if first_usable_base.is_none() {
                    first_usable_base =
                        Some(base);
                }

                last_usable_end =
                    Some(
                        last_usable_end
                            .unwrap_or(0)
                            .max(end)
                    );
            }

            limine::memmap::MEMMAP_RESERVED => {
                reserved_memory =
                    reserved_memory.saturating_add(length);
            }

            limine::memmap::MEMMAP_ACPI_RECLAIMABLE => {
                reclaimable_memory =
                    reclaimable_memory.saturating_add(length);
            }

            limine::memmap::MEMMAP_BOOTLOADER_RECLAIMABLE => {
                bootloader_memory =
                    bootloader_memory.saturating_add(length);
            }

            limine::memmap::MEMMAP_EXECUTABLE_AND_MODULES => {
                kernel_and_modules =
                    kernel_and_modules.saturating_add(length);
            }

            limine::memmap::MEMMAP_BAD_MEMORY => {
                bad_memory =
                    bad_memory.saturating_add(length);
            }

            limine::memmap::MEMMAP_FRAMEBUFFER => {
                framebuffer_memory =
                    framebuffer_memory.saturating_add(length);
            }

            _ => {
                other_memory =
                    other_memory.saturating_add(length);
            }
        }
    }

    console_println_color!(
        Color::GREEN,
        "  Total physical: {} KiB ({} MiB)",
        total_memory / 1024,
        total_memory / (1024 * 1024)
    );

    console_println_color!(
        Color::GREEN,
        "  Usable:         {} KiB ({} MiB)",
        usable_memory / 1024,
        usable_memory / (1024 * 1024)
    );

    console_println_color!(
        Color::GREEN,
        "  Usable regions: {}",
        usable_regions
    );

    console_println_color!(
        Color::GREEN,
        "  Reserved:       {} KiB",
        reserved_memory / 1024
    );

    console_println_color!(
        Color::GREEN,
        "  ACPI reclaim:   {} KiB",
        reclaimable_memory / 1024
    );

    console_println_color!(
        Color::GREEN,
        "  Bootloader:     {} KiB",
        bootloader_memory / 1024
    );

    console_println_color!(
        Color::GREEN,
        "  Kernel/modules: {} KiB",
        kernel_and_modules / 1024
    );

    console_println_color!(
        Color::GREEN,
        "  Framebuffer:    {} KiB",
        framebuffer_memory / 1024
    );

    console_println_color!(
        Color::GREEN,
        "  Bad memory:     {} KiB",
        bad_memory / 1024
    );

    console_println_color!(
        Color::GREEN,
        "  Other:          {} KiB",
        other_memory / 1024
    );

    // --------------------------------------------------------
    // Physical frames
    // --------------------------------------------------------

    console_println_color!(
        Color::GREEN,
        ""
    );

    console_println_color!(
        Color::GREEN,
        "[PHYSICAL FRAMES]"
    );

    let frame_count =
        usable_memory / frame::FRAME_SIZE;

    console_println_color!(
        Color::GREEN,
        "  Frame size:     {} bytes",
        frame::FRAME_SIZE
    );
}
