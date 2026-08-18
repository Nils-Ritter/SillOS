use core::fmt;

use super::frame_alloc::FRAME_SIZE;

/// A virtual address.
///
/// This type represents an address in the CPU's virtual address space.
/// It does not imply that the address is currently mapped.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct VirtAddr(u64);

impl VirtAddr {
    pub const fn new(address: u64) -> Self {
        Self(address)
    }

    pub const fn as_u64(self) -> u64 {
        self.0
    }

    pub const fn is_aligned(self) -> bool {
        self.0 % FRAME_SIZE == 0
    }

    pub const fn align_down(self) -> Self {
        Self::new(
            self.0 & !(FRAME_SIZE - 1)
        )
    }

    pub const fn align_up(self) -> Option<Self> {
        match self.0.checked_add(FRAME_SIZE - 1) {
            Some(value) => Some(Self::new(
                value & !(FRAME_SIZE - 1)
            )),
            None => None,
        }
    }
}

impl fmt::Debug for VirtAddr {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        write!(
            f,
            "{:#018x}",
            self.0
        )
    }
}

/// A single 4 KiB virtual page.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Page {
    start_address: VirtAddr,
}

impl Page {
    pub const fn from_start_address(
        address: VirtAddr,
    ) -> Option<Self> {
        if address.is_aligned() {
            Some(Self {
                start_address: address,
            })
        } else {
            None
        }
    }

    pub const fn containing_address(
        address: VirtAddr,
    ) -> Self {
        Self {
            start_address: address.align_down(),
        }
    }

    pub const fn start_address(self) -> VirtAddr {
        self.start_address
    }

    pub const fn number(self) -> u64 {
        self.start_address.as_u64() / FRAME_SIZE
    }

    pub const fn pml4_index(self) -> usize {
        ((self.start_address.as_u64() >> 39) & 0x1ff) as usize
    }

    pub const fn pdpt_index(self) -> usize {
        ((self.start_address.as_u64() >> 30) & 0x1ff) as usize
    }

    pub const fn pd_index(self) -> usize {
        ((self.start_address.as_u64() >> 21) & 0x1ff) as usize
    }

    pub const fn pt_index(self) -> usize {
        ((self.start_address.as_u64() >> 12) & 0x1ff) as usize
    }
}

impl fmt::Debug for Page {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        write!(
            f,
            "Page({:#018x})",
            self.start_address.as_u64()
        )
    }
}

/// A half-open virtual address range.
///
/// [start, end)
///
/// Both addresses must be page aligned.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PageRange {
    start: VirtAddr,
    end: VirtAddr,
}

impl PageRange {
    pub fn new(
        start: VirtAddr,
        end: VirtAddr,
    ) -> Option<Self> {
        if !start.is_aligned()
            || !end.is_aligned()
        {
            return None;
        }

        if start >= end {
            return None;
        }

        Some(Self {
            start,
            end,
        })
    }

    pub const fn start(self) -> VirtAddr {
        self.start
    }

    pub const fn end(self) -> VirtAddr {
        self.end
    }

    pub const fn page_count(self) -> u64 {
        (self.end.as_u64() - self.start.as_u64())
            / FRAME_SIZE
    }

    pub fn page_at(
        self,
        index: u64,
    ) -> Option<Page> {
        if index >= self.page_count() {
            return None;
        }

        let offset =
            index.checked_mul(FRAME_SIZE)?;

        let address =
            self.start
                .as_u64()
                .checked_add(offset)?;

        Page::from_start_address(
            VirtAddr::new(address)
        )
    }
}

/// Errors produced by the virtual-page allocator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageAllocatorError {
    UnalignedRange,
    InvalidRange,
    RangeTooLarge,
    TooManyPages,
    BitmapTooLarge,
    InvalidPage,
    AlreadyAllocated,
    AlreadyFree,
    ReservationOutsideRange,
}

/// Statistics about the virtual-page allocator.
#[derive(Debug, Clone, Copy)]
pub struct PageAllocatorStats {
    pub total_pages: u64,
    pub free_pages: u64,
    pub allocated_pages: u64,
}

/// Bitmap-backed virtual-page allocator.
///
/// Each bit represents one virtual page:
///
///     0 = free
///     1 = allocated
///
/// This allocator only manages virtual address ownership.
///
/// It does NOT:
///
/// - create page tables
/// - map physical frames
/// - unmap pages
/// - flush TLB entries
///
/// Those responsibilities belong to the mapper/address-space layer.
pub struct BitmapPageAllocator {
    bitmap: &'static mut [u64],

    /// First virtual page represented by this allocator.
    start_page: u64,

    /// Number of virtual pages represented.
    page_count: u64,

    /// Number of currently free pages.
    free_pages: u64,

    /// Bitmap word from which allocation starts.
    next_word: usize,
}

impl BitmapPageAllocator {
    /// Create a virtual page allocator.
    ///
    /// `range` is the virtual address space controlled by this allocator.
    ///
    /// `bitmap_storage` must contain at least the number of u64 words
    /// returned by `bitmap_words_for_pages()`.
    ///
    /// # Safety
    ///
    /// `bitmap_storage` must remain valid and writable for the lifetime
    /// of this allocator.
    pub unsafe fn new(
        range: PageRange,
        bitmap_storage: &'static mut [u64],
    ) -> Result<Self, PageAllocatorError> {
        let page_count =
            range.page_count();

        if page_count == 0 {
            return Err(
                PageAllocatorError::InvalidRange
            );
        }

        let required_words =
            bitmap_words_for_pages(
                page_count
            )?;

        if bitmap_storage.len()
            < required_words
        {
            return Err(
                PageAllocatorError::BitmapTooLarge
            );
        }

        bitmap_storage[..required_words]
            .fill(0);

        //
        // Mark unused bits in the final bitmap word as allocated.
        //

        let remaining_bits =
            page_count % 64;

        if remaining_bits != 0 {
            let last_word =
                required_words - 1;

            bitmap_storage[last_word] =
                !((1u64 << remaining_bits) - 1);
        }

        Ok(Self {
            bitmap:
                &mut bitmap_storage[..required_words],

            start_page:
                range.start.as_u64()
                    / FRAME_SIZE,

            page_count,

            free_pages:
                page_count,

            next_word: 0,
        })
    }

    pub const fn total_pages(&self) -> u64 {
        self.page_count
    }

    pub const fn free_pages(&self) -> u64 {
        self.free_pages
    }

    pub const fn allocated_pages(&self) -> u64 {
        self.page_count
            - self.free_pages
    }

    pub const fn stats(
        &self,
    ) -> PageAllocatorStats {
        PageAllocatorStats {
            total_pages:
                self.page_count,

            free_pages:
                self.free_pages,

            allocated_pages:
                self.allocated_pages(),
        }
    }

    /// Allocate one virtual page.
    pub fn allocate(
        &mut self,
    ) -> Option<Page> {
        if self.free_pages == 0 {
            return None;
        }

        let word_count =
            self.bitmap.len();

        //
        // First search from the cursor.
        //

        for word_index
            in self.next_word..word_count
        {
            let word =
                self.bitmap[word_index];

            if word == u64::MAX {
                continue;
            }

            let bit_index =
                (!word)
                    .trailing_zeros()
                    as usize;

            let local_page_index =
                word_index
                    .checked_mul(64)?
                    .checked_add(
                        bit_index
                    )?;

            if local_page_index
                >= self.page_count as usize
            {
                continue;
            }

            self.bitmap[word_index] |=
                1u64 << bit_index;

            self.free_pages -= 1;

            self.next_word =
                word_index;

            let page_number =
                self.start_page
                    .checked_add(
                        local_page_index as u64
                    )?;

            let address =
                page_number
                    .checked_mul(
                        FRAME_SIZE
                    )?;

            return Page::from_start_address(
                VirtAddr::new(address)
            );
        }

        //
        // We may have wrapped around.
        //
        // This matters when pages are freed before the current cursor.
        //

        for word_index in
            0..self.next_word
        {
            let word =
                self.bitmap[word_index];

            if word == u64::MAX {
                continue;
            }

            let bit_index =
                (!word)
                    .trailing_zeros()
                    as usize;

            let local_page_index =
                word_index
                    .checked_mul(64)?
                    .checked_add(
                        bit_index
                    )?;

            if local_page_index
                >= self.page_count as usize
            {
                continue;
            }

            self.bitmap[word_index] |=
                1u64 << bit_index;

            self.free_pages -= 1;

            self.next_word =
                word_index;

            let page_number =
                self.start_page
                    .checked_add(
                        local_page_index as u64
                    )?;

            let address =
                page_number
                    .checked_mul(
                        FRAME_SIZE
                    )?;

            return Page::from_start_address(
                VirtAddr::new(address)
            );
        }

        debug_assert!(
            false,
            "page allocator bitmap/free count is inconsistent"
        );

        None
    }

    /// Free a virtual page.
    ///
    /// This only releases the virtual address.
    pub fn deallocate(
        &mut self,
        page: Page,
    ) -> Result<(), PageAllocatorError> {
        let index =
            self.page_index(page)?;

        let word_index =
            index / 64;

        let bit_index =
            index % 64;

        let mask =
            1u64 << bit_index;

        if self.bitmap[word_index]
            & mask
            == 0
        {
            return Err(
                PageAllocatorError::AlreadyFree
            );
        }

        self.bitmap[word_index] &= !mask;

        self.free_pages += 1;

        if word_index < self.next_word {
            self.next_word =
                word_index;
        }

        Ok(())
    }

    /// Check whether a page is allocated.
    pub fn is_allocated(
        &self,
        page: Page,
    ) -> Result<bool, PageAllocatorError> {
        let index =
            self.page_index(page)?;

        let word_index =
            index / 64;

        let bit_index =
            index % 64;

        Ok(
            self.bitmap[word_index]
                & (1u64 << bit_index)
                != 0
        )
    }

    /// Reserve an entire virtual range.
    ///
    /// The range must be completely contained inside this allocator's range.
    pub fn reserve(
        &mut self,
        range: PageRange,
    ) -> Result<(), PageAllocatorError> {
        let start_page =
            range.start()
                .as_u64()
                / FRAME_SIZE;

        let end_page =
            range.end()
                .as_u64()
                / FRAME_SIZE;

        if start_page < self.start_page {
            return Err(
                PageAllocatorError::ReservationOutsideRange
            );
        }

        let relative_start =
            start_page
                - self.start_page;

        let relative_end =
            end_page
                .checked_sub(
                    self.start_page
                )
                .ok_or(
                    PageAllocatorError::ReservationOutsideRange
                )?;

        if relative_end
            > self.page_count
        {
            return Err(
                PageAllocatorError::ReservationOutsideRange
            );
        }

        if relative_start
            >= relative_end
        {
            return Err(
                PageAllocatorError::InvalidRange
            );
        }

        let start =
            usize::try_from(
                relative_start
            )
            .map_err(|_| {
                PageAllocatorError::TooManyPages
            })?;

        let end =
            usize::try_from(
                relative_end
            )
            .map_err(|_| {
                PageAllocatorError::TooManyPages
            })?;

        //
        // Check everything first.
        //
        // This means a failed reservation does not partially modify the
        // bitmap.
        //

        for index in start..end {
            let word_index =
                index / 64;

            let bit_index =
                index % 64;

            let mask =
                1u64 << bit_index;

            if self.bitmap[word_index]
                & mask
                != 0
            {
                return Err(
                    PageAllocatorError::AlreadyAllocated
                );
            }
        }

        //
        // Now perform the reservation.
        //

        for index in start..end {
            let word_index =
                index / 64;

            let bit_index =
                index % 64;

            self.bitmap[word_index] |=
                1u64 << bit_index;

            self.free_pages -= 1;
        }

        let start_word =
            start / 64;

        if start_word < self.next_word {
            self.next_word =
                start_word;
        }

        Ok(())
    }

    fn page_index(
        &self,
        page: Page,
    ) -> Result<usize, PageAllocatorError> {
        let page_number =
            page.number();

        if page_number
            < self.start_page
        {
            return Err(
                PageAllocatorError::InvalidPage
            );
        }

        let index =
            page_number
                - self.start_page;

        if index
            >= self.page_count
        {
            return Err(
                PageAllocatorError::InvalidPage
            );
        }

        usize::try_from(index)
            .map_err(|_| {
                PageAllocatorError::InvalidPage
            })
    }

    #[cfg(debug_assertions)]
    pub fn verify(&self) {
        let mut free_pages = 0u64;

        for (
            word_index,
            &word,
        ) in self.bitmap.iter().enumerate()
        {
            let mut free_bits =
                !word;

            if word_index + 1
                == self.bitmap.len()
            {
                let valid_bits =
                    self.page_count % 64;

                if valid_bits != 0 {
                    free_bits &=
                        (1u64 << valid_bits) - 1;
                }
            }

            free_pages +=
                free_bits.count_ones()
                    as u64;
        }

        assert_eq!(
            free_pages,
            self.free_pages,
            "page allocator free-page count is inconsistent"
        );
    }
}

/// Calculate the number of u64 words needed for a bitmap.
pub fn bitmap_words_for_pages(
    page_count: u64,
) -> Result<usize, PageAllocatorError> {
    let words =
        page_count
            .checked_add(63)
            .ok_or(
                PageAllocatorError::TooManyPages
            )?
            / 64;

    usize::try_from(words)
        .map_err(|_| {
            PageAllocatorError::TooManyPages
        })
}

/// Calculate the number of bytes needed for a bitmap.
pub fn bitmap_bytes_for_pages(
    page_count: u64,
) -> Result<u64, PageAllocatorError> {
    let words =
        bitmap_words_for_pages(
            page_count
        )?;

    let words =
        u64::try_from(words)
            .map_err(|_| {
                PageAllocatorError::BitmapTooLarge
            })?;

    words.checked_mul(8)
        .ok_or(
            PageAllocatorError::BitmapTooLarge
        )
}
