use core::fmt;

/// Size of a single physical frame.
///
/// We use 4 KiB frames as the fundamental allocation unit. Larger pages
/// should be handled by the virtual-memory/page-table layer.
pub const FRAME_SIZE: u64 = 4096;

/// A physical address.
///
/// This is deliberately separate from a virtual address so that accidentally
/// mixing the two becomes harder.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct PhysAddr(u64);

impl PhysAddr {
    pub const fn new(address: u64) -> Self {
        Self(address)
    }

    pub const fn as_u64(self) -> u64 {
        self.0
    }

    pub const fn is_aligned(self) -> bool {
        self.0 % FRAME_SIZE == 0
    }
}

impl fmt::Debug for PhysAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:#x}", self.0)
    }
}

/// A single 4 KiB physical frame.
///
/// The contained address is always the start address of the frame.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct PhysFrame {
    start_address: PhysAddr,
}

impl PhysFrame {
    /// Construct a frame from an address, requiring the address to be aligned.
    pub const fn from_start_address(address: PhysAddr) -> Option<Self> {
        if address.is_aligned() {
            Some(Self {
                start_address: address,
            })
        } else {
            None
        }
    }

    /// Construct the frame containing an arbitrary physical address.
    pub const fn containing_address(address: PhysAddr) -> Self {
        Self {
            start_address: PhysAddr::new(
                address.as_u64() & !(FRAME_SIZE - 1),
            ),
        }
    }

    pub const fn start_address(self) -> PhysAddr {
        self.start_address
    }

    /// Zero-based physical frame number.
    pub const fn number(self) -> u64 {
        self.start_address.as_u64() / FRAME_SIZE
    }
}

impl fmt::Debug for PhysFrame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PhysFrame({:#x})", self.start_address.as_u64())
    }
}

/// Errors that can occur while initializing the frame allocator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameAllocatorError {
    /// The Limine memory map was empty.
    EmptyMemoryMap,

    /// A memory-map entry's base + length overflowed.
    InvalidMemoryMapEntry,

    /// The physical address space was too large to represent.
    AddressSpaceTooLarge,

    /// Calculating the bitmap size overflowed.
    BitmapSizeOverflow,

    /// No usable region was large enough to contain the bitmap.
    NoSpaceForBitmap,

    /// The HHDM offset plus the bitmap's physical address overflowed.
    HhdmAddressOverflow,

    /// The bitmap address was not aligned.
    BitmapMisaligned,

    /// The bitmap was too large for the current platform's `usize`.
    BitmapTooLarge,

    /// The bitmap overlapped something unexpectedly.
    BitmapOverlap,
}

/// Errors that can occur when manipulating individual frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameError {
    /// Frame is outside the physical address space represented by the bitmap.
    InvalidFrame,

    /// Attempted to free a frame that was already free.
    DoubleFree,

    /// Attempted to allocate a frame that was already allocated.
    AlreadyAllocated,
}

/// Basic interface expected by the virtual-memory subsystem.
///
/// Your page-table code should depend on this trait rather than knowing
/// that the implementation happens to use a bitmap.
pub trait FrameAllocator {
    fn allocate(&mut self) -> Option<PhysFrame>;

    fn deallocate(
        &mut self,
        frame: PhysFrame,
    ) -> Result<(), FrameError>;
}

/// Statistics useful for debugging the physical memory manager.
#[derive(Debug, Clone, Copy)]
pub struct FrameAllocatorStats {
    pub total_frames: u64,
    pub free_frames: u64,
    pub allocated_frames: u64,
    pub bitmap_start: PhysAddr,
    pub bitmap_size: u64,
}

/// Bitmap-backed physical frame allocator.
///
/// Bitmap representation:
///
///     0 = free
///     1 = allocated
///
/// Initialization is deliberately conservative:
///
///     1. Every frame starts as allocated.
///     2. Only Limine USABLE regions are marked free.
///     3. The bitmap itself is reserved again.
///
/// Consequently, anything Limine does not explicitly identify as usable
/// can never accidentally be returned by this allocator.
pub struct BitmapFrameAllocator {
    /// One bit per physical frame.
    bitmap: &'static mut [u64],

    /// Number of physical frames represented by the bitmap.
    frame_count: u64,

    /// Number of currently free frames.
    free_frames: u64,

    /// Physical address occupied by the bitmap.
    bitmap_start: PhysAddr,

    /// Size of the bitmap reservation.
    bitmap_size: u64,

    /// Bitmap word at which the next allocation scan begins.
    ///
    /// This avoids starting at frame zero for every allocation.
    next_word: usize,
}

impl BitmapFrameAllocator {
    /// Initialize the frame allocator from Limine's memory map.
    ///
    /// `hhdm_offset` must be the offset returned by Limine's HHDM response.
    ///
    /// # Safety
    ///
    /// The caller must guarantee that `hhdm_offset` describes a valid direct
    /// physical-memory mapping for the physical memory containing the bitmap.
    ///
    /// This function also assumes that the Limine memory-map response remains
    /// valid while initialization is taking place.
    pub unsafe fn new(
        memory_map: &[&limine::memmap::Entry],
        hhdm_offset: u64,
    ) -> Result<Self, FrameAllocatorError> {
        if memory_map.is_empty() {
            return Err(FrameAllocatorError::EmptyMemoryMap);
        }

        //
        // ---------------------------------------------------------------
        // 1. Determine the highest physical address.
        // ---------------------------------------------------------------
        //

        let highest_address =
            highest_physical_address(memory_map)?;

        //
        // ---------------------------------------------------------------
        // 2. Determine how many 4 KiB frames exist.
        // ---------------------------------------------------------------
        //

        let frame_count = highest_address
            .checked_add(FRAME_SIZE - 1)
            .ok_or(FrameAllocatorError::AddressSpaceTooLarge)?
            / FRAME_SIZE;

        //
        // ---------------------------------------------------------------
        // 3. Determine bitmap size.
        // ---------------------------------------------------------------
        //
        // One bit per frame.
        //

        let bitmap_size =
            bitmap_size_for_frames(frame_count)?;

        //
        // The bitmap itself is reserved in whole physical frames.
        //

        let bitmap_size_aligned =
            align_up(bitmap_size, FRAME_SIZE)
                .ok_or(FrameAllocatorError::BitmapSizeOverflow)?;

        //
        // ---------------------------------------------------------------
        // 4. Find a usable physical region for the bitmap.
        // ---------------------------------------------------------------
        //

        let bitmap_start =
            find_bitmap_location(
                memory_map,
                bitmap_size_aligned,
            )
            .ok_or(FrameAllocatorError::NoSpaceForBitmap)?;

        if !bitmap_start.is_aligned() {
            return Err(FrameAllocatorError::BitmapMisaligned);
        }

        //
        // ---------------------------------------------------------------
        // 5. Convert the bitmap's physical address to a virtual address
        //    through Limine's HHDM.
        // ---------------------------------------------------------------
        //

        let bitmap_virtual =
            hhdm_offset
                .checked_add(bitmap_start.as_u64())
                .ok_or(FrameAllocatorError::HhdmAddressOverflow)?;

        //
        // ---------------------------------------------------------------
        // 6. Turn that memory into our bitmap.
        // ---------------------------------------------------------------
        //

        let bitmap_words_u64 =
            bitmap_size_aligned / 8;

        let bitmap_words =
            usize::try_from(bitmap_words_u64)
                .map_err(|_| FrameAllocatorError::BitmapTooLarge)?;

        let bitmap_ptr =
            bitmap_virtual as *mut u64;

        // SAFETY:
        //
        // The physical range was selected from a Limine USABLE region.
        // Limine's HHDM maps that physical memory at hhdm_offset + physical.
        //
        // The bitmap is immediately treated as reserved below, so no future
        // allocation can hand this memory out.
        let bitmap =
            core::slice::from_raw_parts_mut(
                bitmap_ptr,
                bitmap_words,
            );

        //
        // Start with EVERYTHING allocated.
        //

        bitmap.fill(u64::MAX);

        let mut allocator = Self {
            bitmap,
            frame_count,
            free_frames: 0,
            bitmap_start,
            bitmap_size: bitmap_size_aligned,
            next_word: 0,
        };

        //
        // ---------------------------------------------------------------
        // 7. Make only Limine USABLE memory available.
        // ---------------------------------------------------------------
        //

        for entry in memory_map {
            if entry.type_ != limine::memmap::MEMMAP_USABLE {
                continue;
            }

            let region_start =
                align_up(entry.base, FRAME_SIZE)
                    .ok_or(
                        FrameAllocatorError::InvalidMemoryMapEntry
                    )?;

            let region_end =
                entry
                    .base
                    .checked_add(entry.length)
                    .ok_or(
                        FrameAllocatorError::InvalidMemoryMapEntry
                    )?;

            let region_end =
                align_down(region_end, FRAME_SIZE);

            if region_start >= region_end {
                continue;
            }

            allocator.mark_range_free(
                PhysAddr::new(region_start),
                PhysAddr::new(region_end),
            )?;
        }

        //
        // ---------------------------------------------------------------
        // 8. Reserve the bitmap itself.
        // ---------------------------------------------------------------
        //

        let bitmap_end =
            bitmap_start
                .as_u64()
                .checked_add(bitmap_size_aligned)
                .ok_or(FrameAllocatorError::BitmapSizeOverflow)?;

        allocator.mark_range_allocated(
            bitmap_start,
            PhysAddr::new(bitmap_end),
        )?;

        Ok(allocator)
    }

    /// Number of physical frames represented by this allocator.
    pub const fn total_frames(&self) -> u64 {
        self.frame_count
    }

    /// Number of currently free frames.
    pub const fn free_frames(&self) -> u64 {
        self.free_frames
    }

    /// Number of currently allocated frames.
    pub const fn allocated_frames(&self) -> u64 {
        self.frame_count - self.free_frames
    }

    /// Return useful allocator statistics.
    pub const fn stats(&self) -> FrameAllocatorStats {
        FrameAllocatorStats {
            total_frames: self.frame_count,
            free_frames: self.free_frames,
            allocated_frames: self.allocated_frames(),
            bitmap_start: self.bitmap_start,
            bitmap_size: self.bitmap_size,
        }
    }

    /// Physical address occupied by the bitmap.
    pub const fn bitmap_start(&self) -> PhysAddr {
        self.bitmap_start
    }

    /// Size of the bitmap reservation.
    pub const fn bitmap_size(&self) -> u64 {
        self.bitmap_size
    }

    /// Check whether a particular frame is currently free.
    pub fn is_free(
        &self,
        frame: PhysFrame,
    ) -> Result<bool, FrameError> {
        let index =
            self.frame_index(frame)?;

        let word =
            index / 64;

        let bit =
            index % 64;

        Ok(
            self.bitmap[word]
                & (1u64 << bit)
                == 0
        )
    }

    /// Convert a frame into a bitmap index.
    fn frame_index(
        &self,
        frame: PhysFrame,
    ) -> Result<usize, FrameError> {
        let frame_number =
            frame.number();

        if frame_number >= self.frame_count {
            return Err(FrameError::InvalidFrame);
        }

        usize::try_from(frame_number)
            .map_err(|_| FrameError::InvalidFrame)
    }

    /// Mark a frame as free.
    fn mark_free(
        &mut self,
        frame: PhysFrame,
    ) -> Result<(), FrameError> {
        let index =
            self.frame_index(frame)?;

        let word_index =
            index / 64;

        let bit_index =
            index % 64;

        let mask =
            1u64 << bit_index;

        //
        // Bit is already zero → frame is already free.
        //

        if self.bitmap[word_index] & mask == 0 {
            return Err(FrameError::DoubleFree);
        }

        self.bitmap[word_index] &= !mask;

        self.free_frames += 1;

        //
        // If the freed frame is before our current scan position, start
        // searching there next time.
        //

        if word_index < self.next_word {
            self.next_word = word_index;
        }

        Ok(())
    }

    /// Mark a frame as allocated.
    fn mark_allocated(
        &mut self,
        frame: PhysFrame,
    ) -> Result<(), FrameError> {
        let index =
            self.frame_index(frame)?;

        let word_index =
            index / 64;

        let bit_index =
            index % 64;

        let mask =
            1u64 << bit_index;

        //
        // Bit already one → already allocated.
        //

        if self.bitmap[word_index] & mask != 0 {
            return Err(FrameError::AlreadyAllocated);
        }

        self.bitmap[word_index] |= mask;

        self.free_frames -= 1;

        Ok(())
    }

    /// Mark a whole aligned physical range as free.
    fn mark_range_free(
        &mut self,
        start: PhysAddr,
        end: PhysAddr,
    ) -> Result<(), FrameAllocatorError> {
        debug_assert!(start.is_aligned());
        debug_assert!(end.is_aligned());
        debug_assert!(start <= end);

        let mut address =
            start.as_u64();

        while address < end.as_u64() {
            let frame =
                PhysFrame::from_start_address(
                    PhysAddr::new(address)
                )
                .expect("aligned address must produce a frame");

            self.mark_free(frame)
                .map_err(|_| {
                    FrameAllocatorError::InvalidMemoryMapEntry
                })?;

            address += FRAME_SIZE;
        }

        Ok(())
    }

    /// Mark a whole aligned physical range as allocated.
    fn mark_range_allocated(
        &mut self,
        start: PhysAddr,
        end: PhysAddr,
    ) -> Result<(), FrameAllocatorError> {
        debug_assert!(start.is_aligned());
        debug_assert!(end.is_aligned());
        debug_assert!(start <= end);

        let mut address =
            start.as_u64();

        while address < end.as_u64() {
            let frame =
                PhysFrame::from_start_address(
                    PhysAddr::new(address)
                )
                .expect("aligned address must produce a frame");

            self.mark_allocated(frame)
                .map_err(|_| {
                    FrameAllocatorError::BitmapOverlap
                })?;

            address += FRAME_SIZE;
        }

        Ok(())
    }

    /// Find and allocate one free frame.
    fn allocate_inner(
        &mut self,
    ) -> Option<PhysFrame> {
        if self.free_frames == 0 {
            return None;
        }

        let word_count =
            self.bitmap.len();

        //
        // Search from the current cursor.
        //

        for word_index in self.next_word..word_count {
            let word =
                self.bitmap[word_index];

            //
            // All 64 frames represented by this word are allocated.
            //

            if word == u64::MAX {
                continue;
            }

            //
            // Find the first zero bit.
            //

            let bit_index =
                (!word).trailing_zeros() as usize;

            //
            // `trailing_ones()` above is NOT what we want if the first bit
            // is zero. Use the inverted word to find the first free bit.
            //

            let bit_index =
                (!word).trailing_zeros() as usize;

            let frame_index =
                word_index
                    .checked_mul(64)?
                    .checked_add(bit_index)?;

            //
            // The final u64 may contain padding bits beyond frame_count.
            //

            if frame_index >= self.frame_count as usize {
                continue;
            }

            //
            // Mark allocated.
            //

            self.bitmap[word_index] |=
                1u64 << bit_index;

            self.free_frames -= 1;

            self.next_word =
                word_index;

            let address =
                (frame_index as u64)
                    .checked_mul(FRAME_SIZE)?;

            return PhysFrame::from_start_address(
                PhysAddr::new(address)
            );
        }

        //
        // The bookkeeping said there were free frames, but none were found.
        //
        // This indicates internal corruption.
        //

        debug_assert!(
            false,
            "frame allocator bitmap/free-frame count inconsistent"
        );

        None
    }

    /// Debug-only consistency check.
    ///
    /// This scans the entire bitmap and verifies that the cached free-frame
    /// count is correct.
    #[cfg(debug_assertions)]
    pub fn verify(&self) {
        let mut free_count = 0u64;

        for (word_index, &word) in
            self.bitmap.iter().enumerate()
        {
            let mut free_bits =
                !word;

            //
            // Ignore padding bits beyond frame_count in the final word.
            //

            if word_index + 1 == self.bitmap.len() {
                let valid_bits =
                    self.frame_count % 64;

                if valid_bits != 0 {
                    free_bits &=
                        (1u64 << valid_bits) - 1;
                }
            }

            free_count +=
                free_bits.count_ones() as u64;
        }

        assert_eq!(
            free_count,
            self.free_frames,
            "frame allocator free-frame count is inconsistent"
        );
    }
}

impl FrameAllocator for BitmapFrameAllocator {
    fn allocate(&mut self) -> Option<PhysFrame> {
        self.allocate_inner()
    }

    fn deallocate(
        &mut self,
        frame: PhysFrame,
    ) -> Result<(), FrameError> {
        self.mark_free(frame)
    }
}

//
// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------
//

/// Calculate how many bytes are needed for a bitmap containing one bit per
/// physical frame.
fn bitmap_size_for_frames(
    frame_count: u64,
) -> Result<u64, FrameAllocatorError> {
    //
    // ceil(frame_count / 8)
    //
    let adjusted =
        frame_count
            .checked_add(7)
            .ok_or(
                FrameAllocatorError::BitmapSizeOverflow
            )?;

    Ok(adjusted / 8)
}

/// Find a usable region large enough to contain the bitmap.
///
/// We deliberately place the bitmap at the beginning of the first suitable
/// usable region. That keeps initialization simple and deterministic.
fn find_bitmap_location(
    memory_map: &[&limine::memmap::Entry],
    bitmap_size: u64,
) -> Option<PhysAddr> {
    for entry in memory_map {
        if entry.type_ != limine::memmap::MEMMAP_USABLE {
            continue;
        }

        let region_start =
            align_up(
                entry.base,
                FRAME_SIZE,
            )?;

        let region_end =
            entry.base.checked_add(
                entry.length
            )?;

        let bitmap_end =
            region_start.checked_add(
                bitmap_size
            )?;

        if bitmap_end <= region_end {
            return Some(
                PhysAddr::new(region_start)
            );
        }
    }

    None
}

/// Find the highest physical address represented by the Limine memory map.
fn highest_physical_address(
    memory_map: &[&limine::memmap::Entry],
) -> Result<u64, FrameAllocatorError> {
    let mut highest = 0u64;

    for entry in memory_map {
        let end =
            entry
                .base
                .checked_add(entry.length)
                .ok_or(
                    FrameAllocatorError::InvalidMemoryMapEntry
                )?;

        highest =
            highest.max(end);
    }

    Ok(highest)
}

/// Round `value` upward to `alignment`.
///
/// `alignment` must be a power of two.
fn align_up(
    value: u64,
    alignment: u64,
) -> Option<u64> {
    debug_assert!(
        alignment.is_power_of_two()
    );

    value
        .checked_add(alignment - 1)
        .map(|value| {
            value & !(alignment - 1)
        })
}

/// Round `value` downward to `alignment`.
///
/// `alignment` must be a power of two.
fn align_down(
    value: u64,
    alignment: u64,
) -> u64 {
    debug_assert!(
        alignment.is_power_of_two()
    );

    value & !(alignment - 1)
}
