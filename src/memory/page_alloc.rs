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
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:#018x}", self.0)
    }
}

/// A single 4 KiB virtual page.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Page {
    start_address: VirtAddr,
}

impl Page {
    /// Create a page from an aligned virtual address.
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

    /// Return the page containing `address`.
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

    /// Zero-based page number within the virtual address space.
    pub const fn number(self) -> u64 {
        self.start_address.as_u64() / FRAME_SIZE
    }

    /// Return the PML4 index used by x86-64 page tables.
    pub const fn pml4_index(self) -> usize {
        ((self.start_address.as_u64() >> 39) & 0x1ff) as usize
    }

    /// Return the PDPT index used by x86-64 page tables.
    pub const fn pdpt_index(self) -> usize {
        ((self.start_address.as_u64() >> 30) & 0x1ff) as usize
    }

    /// Return the page-directory index used by x86-64 page tables.
    pub const fn pd_index(self) -> usize {
        ((self.start_address.as_u64() >> 21) & 0x1ff) as usize
    }

    /// Return the page-table index used by x86-64 page tables.
    pub const fn pt_index(self) -> usize {
        ((self.start_address.as_u64() >> 12) & 0x1ff) as usize
    }
}

impl fmt::Debug for Page {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Page({:#018x})",
            self.start_address.as_u64()
        )
    }
}

/// A half-open virtual-address range.
///
/// The range is:
///
///     [start, end)
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
        if !start.is_aligned() || !end.is_aligned() {
            return None;
        }

        if start >= end {
            return None;
        }

        Some(Self { start, end })
    }

    pub const fn start(self) -> VirtAddr {
        self.start
    }

    pub const fn end(self) -> VirtAddr {
        self.end
    }

    /// Number of pages in this range.
    pub const fn page_count(self) -> u64 {
        (self.end.as_u64() - self.start.as_u64())
            / FRAME_SIZE
    }

    /// Return the page at `index`.
    pub fn page_at(self, index: u64) -> Option<Page> {
        if index >= self.page_count() {
            return None;
        }

        let address = self
            .start
            .as_u64()
            .checked_add(index * FRAME_SIZE)?;

        Page::from_start_address(
            VirtAddr::new(address)
        )
    }
}

/// Errors produced by the virtual page allocator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageAllocatorError {
    /// The supplied range was not page aligned.
    UnalignedRange,

    /// The supplied range was empty or backwards.
    InvalidRange,

    /// The range is too large for the bitmap representation.
    RangeTooLarge,

    /// The allocator cannot represent this many pages.
    TooManyPages,

    /// The bitmap itself could not be represented.
    BitmapTooLarge,

    /// The page does not belong to this allocator.
    InvalidPage,

    /// The page was already allocated.
    AlreadyAllocated,

    /// The page was already free.
    AlreadyFree,
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
/// Unlike the physical frame allocator, this allocator does not know or care
/// about physical memory. It only manages virtual address space.
pub struct BitmapPageAllocator {
    bitmap: &'static mut [u64],

    /// First virtual page represented by the bitmap.
    start_page: u64,

    /// Number of virtual pages represented.
    page_count: u64,

    /// Number of currently free pages.
    free_pages: u64,

    /// Bitmap word from which the next allocation begins.
    next_word: usize,
}

impl BitmapPageAllocator {
    /// Create a virtual page allocator.
    ///
    /// `range` specifies the virtual address space managed by this allocator.
    ///
    /// `bitmap_storage` must point to writable memory that is large enough
    /// for the bitmap returned by `bitmap_size_for_pages()`.
    ///
    /// # Safety
    ///
    /// `bitmap_storage` must point to valid writable memory for the duration
    /// of this allocator's lifetime.
    pub unsafe fn new(
        range: PageRange,
        bitmap_storage: &'static mut [u64],
    ) -> Result<Self, PageAllocatorError> {
        let page_count =
            range.page_count();

        if page_count == 0 {
            return Err(PageAllocatorError::InvalidRange);
        }

        let required_words =
            bitmap_words_for_pages(page_count)?;

        if bitmap_storage.len() < required_words {
            return Err(PageAllocatorError::BitmapTooLarge);
        }

        //
        // Start with every virtual page free.
        //

        bitmap_storage[..required_words]
            .fill(0);

        //
        // If the final word has unused bits beyond page_count, mark them
        // allocated so that they can never accidentally be returned.
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
            bitmap: &mut bitmap_storage[..required_words],
            start_page: range.start.as_u64() / FRAME_SIZE,
            page_count,
            free_pages: page_count,
            next_word: 0,
        })
    }

    /// Total number of virtual pages managed.
    pub const fn total_pages(&self) -> u64 {
        self.page_count
    }

    /// Number of currently free pages.
    pub const fn free_pages(&self) -> u64 {
        self.free_pages
    }

    /// Number of allocated pages.
    pub const fn allocated_pages(&self) -> u64 {
        self.page_count - self.free_pages
    }

    /// Return allocator statistics.
    pub const fn stats(&self) -> PageAllocatorStats {
        PageAllocatorStats {
            total_pages: self.page_count,
            free_pages: self.free_pages,
            allocated_pages: self.allocated_pages(),
        }
    }

    /// Allocate one virtual page.
    pub fn allocate(&mut self) -> Option<Page> {
        if self.free_pages == 0 {
            return None;
        }

        for word_index in
            self.next_word..self.bitmap.len()
        {
            let word =
                self.bitmap[word_index];

            if word == u64::MAX {
                continue;
            }

            let bit_index =
                (!word).trailing_zeros() as usize;

            let local_page_index =
                word_index
                    .checked_mul(64)?
                    .checked_add(bit_index)?;

            if local_page_index >= self.page_count as usize {
                continue;
            }

            //
            // Mark allocated.
            //

            self.bitmap[word_index] |=
                1u64 << bit_index;

            self.free_pages -= 1;

            //
            // Keep scanning from here on the next allocation.
            //

            self.next_word =
                word_index;

            let page_number =
                self.start_page
                    .checked_add(
                        local_page_index as u64
                    )?;

            let address =
                page_number
                    .checked_mul(FRAME_SIZE)?;

            return Page::from_start_address(
                VirtAddr::new(address)
            );
        }

        //
        // free_pages said there was a page available, but the bitmap didn't
        // contain one. This means our internal bookkeeping is inconsistent.
        //

        debug_assert!(
            false,
            "page allocator bitmap/free count is inconsistent"
        );

        None
    }

    /// Free a virtual page.
    ///
    /// This only releases the virtual address. It does NOT:
    ///
    /// - unmap the page table entry
    /// - free the physical frame
    /// - invalidate the TLB
    ///
    /// Those responsibilities belong to the page-table/address-space layer.
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

        if self.bitmap[word_index] & mask == 0 {
            return Err(
                PageAllocatorError::AlreadyFree
            );
        }

        self.bitmap[word_index] &= !mask;

        self.free_pages += 1;

        //
        // If this page occurs before our current scan position, make it
        // eligible for allocation immediately.
        //

        if word_index < self.next_word {
            self.next_word =
                word_index;
        }

        Ok(())
    }

    /// Check whether a page is currently allocated.
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

    /// Convert an absolute page number into a bitmap index.
    fn page_index(
        &self,
        page: Page,
    ) -> Result<usize, PageAllocatorError> {
        let page_number =
            page.number();

        if page_number < self.start_page {
            return Err(
                PageAllocatorError::InvalidPage
            );
        }

        let index =
            page_number - self.start_page;

        if index >= self.page_count {
            return Err(
                PageAllocatorError::InvalidPage
            );
        }

        usize::try_from(index)
            .map_err(|_| PageAllocatorError::InvalidPage)
    }

    /// Mark a range of pages as allocated.
    ///
    /// Useful when reserving areas such as:
    ///
    /// - kernel image
    /// - HHDM
    /// - MMIO
    /// - bootloader mappings
    ///
    /// This should generally be done during initialization, before the
    /// allocator is exposed to the rest of the kernel.
    pub fn reserve(
        &mut self,
        range: PageRange,
    ) -> Result<(), PageAllocatorError> {
        let start =
            self.page_index(
                Page::from_start_address(
                    range.start()
                )
                .unwrap()
            )?;

        let end_page =
            Page::from_start_address(
                VirtAddr::new(
                    range.end().as_u64()
                )
            )
            .unwrap();

        let end =
            self.page_index(end_page)
                .unwrap_or(self.page_count as usize);

        if end < start {
            return Err(
                PageAllocatorError::InvalidRange
            );
        }

        for index in start..end {
            let word_index =
                index / 64;

            let bit_index =
                index % 64;

            let mask =
                1u64 << bit_index;

            if self.bitmap[word_index] & mask != 0 {
                return Err(
                    PageAllocatorError::AlreadyAllocated
                );
            }

            self.bitmap[word_index] |= mask;

            self.free_pages -= 1;
        }

        //
        // The next allocation scan should be allowed to start from the
        // beginning if the reservation was before the cursor.
        //

        if start / 64 < self.next_word {
            self.next_word =
                start / 64;
        }

        Ok(())
    }

    /// Debug-only consistency verification.
    #[cfg(debug_assertions)]
    pub fn verify(&self) {
        let mut free_pages = 0u64;

        for (word_index, &word)
            in self.bitmap.iter().enumerate()
        {
            let mut free_bits =
                !word;

            //
            // Ignore padding bits in the final word.
            //

            if word_index + 1 == self.bitmap.len() {
                let valid_bits =
                    self.page_count % 64;

                if valid_bits != 0 {
                    free_bits &=
                        (1u64 << valid_bits) - 1;
                }
            }

            free_pages +=
                free_bits.count_ones() as u64;
        }

        assert_eq!(
            free_pages,
            self.free_pages,
            "page allocator free-page count is inconsistent"
        );
    }
}

/// Calculate how many u64 words are needed for `page_count` pages.
fn bitmap_words_for_pages(
    page_count: u64,
) -> Result<usize, PageAllocatorError> {
    //
    // ceil(page_count / 64)
    //

    let words =
        page_count
            .checked_add(63)
            .ok_or(
                PageAllocatorError::TooManyPages
            )?
            / 64;

    usize::try_from(words)
        .map_err(|_| PageAllocatorError::TooManyPages)
}
