//! Physical frame allocation.
//!
//! This allocator uses Limine's memory map as the source of usable
//! physical memory.
//
//! The implementation is intentionally simple: frames are allocated
//! sequentially and are not currently reclaimed.
//
//! This is suitable for the early stages of the kernel. It can later be
//! replaced by a bitmap, buddy allocator, or another physical allocator.

use limine::memmap::{
    Entry,
    MEMMAP_USABLE,
};

use x86_64::{
    structures::paging::{
        FrameAllocator,
        PhysFrame,
        Size4KiB,
    },
    PhysAddr,
};

/// Size of an x86_64 4 KiB frame.
pub const FRAME_SIZE: u64 = 4096;

/// Physical frame allocator backed by Limine's memory map.
pub struct BootInfoFrameAllocator {
    memory_map: &'static [&'static Entry],

    /// Index of the memory-map region currently being allocated from.
    current_region: usize,

    /// Physical address of the next frame in the current region.
    next_frame: u64,
}

impl BootInfoFrameAllocator {
    /// Create a frame allocator from Limine's memory map.
    ///
    /// # Safety
    ///
    /// The memory map must remain valid for the lifetime of this allocator.
    ///
    /// The caller must also ensure that memory marked as usable by Limine
    /// is actually available for allocation.
    pub unsafe fn new(
        memory_map: &'static [&'static Entry],
    ) -> Self {
        Self {
            memory_map,
            current_region: 0,
            next_frame: 0,
        }
    }

    /// Return the total amount of usable physical memory.
    pub fn usable_memory(&self) -> u64 {
        self.memory_map
            .iter()
            .filter(|region| {
                region.type_ == MEMMAP_USABLE
            })
            .map(|region| region.length)
            .sum()
    }

    /// Find the next usable memory region.
    fn next_usable_region(
        &mut self,
    ) -> Option<&'static Entry> {
        while self.current_region
            < self.memory_map.len()
        {
            let region =
                self.memory_map[self.current_region];

            if region.type_ == MEMMAP_USABLE {
                return Some(region);
            }

            self.current_region += 1;
            self.next_frame = 0;
        }

        None
    }
}

unsafe impl FrameAllocator<Size4KiB>
    for BootInfoFrameAllocator
{
    fn allocate_frame(
        &mut self,
    ) -> Option<PhysFrame<Size4KiB>> {
        loop {
            let region =
                self.next_usable_region()?;

            let region_start =
                align_up(
                    region.base,
                    FRAME_SIZE,
                );

            let region_end =
                align_down(
                    region
                        .base
                        .checked_add(region.length)?,
                    FRAME_SIZE,
                );

            // Start at the beginning of this region.
            if self.next_frame == 0 {
                self.next_frame =
                    region_start;
            }

            // Check whether the region still has frames.
            if self.next_frame < region_end {
                let frame =
                    PhysFrame::containing_address(
                        PhysAddr::new(
                            self.next_frame,
                        ),
                    );

                self.next_frame +=
                    FRAME_SIZE;

                return Some(frame);
            }

            // Move to the next memory-map region.
            self.current_region += 1;
            self.next_frame = 0;
        }
    }
}

/// Align an address upwards.
#[inline]
const fn align_up(
    value: u64,
    alignment: u64,
) -> u64 {
    (value + alignment - 1)
        & !(alignment - 1)
}

/// Align an address downwards.
#[inline]
const fn align_down(
    value: u64,
    alignment: u64,
) -> u64 {
    value & !(alignment - 1)
}
