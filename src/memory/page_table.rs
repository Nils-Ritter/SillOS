use core::fmt;

use super::frame_alloc::{
    BitmapFrameAllocator,
    FrameAllocator,
    PhysAddr,
    PhysFrame,
    FRAME_SIZE,
};

use super::page_alloc::{
    Page,
    VirtAddr,
};

pub const PAGE_TABLE_ENTRIES: usize = 512;
pub const PAGE_TABLE_SIZE: usize = 4096;

/// x86-64 page table entry flags.
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct PageTableFlags(u64);

impl PageTableFlags {
    pub const EMPTY: Self = Self(0);

    pub const PRESENT: Self = Self(1 << 0);
    pub const WRITABLE: Self = Self(1 << 1);
    pub const USER_ACCESSIBLE: Self = Self(1 << 2);
    pub const WRITE_THROUGH: Self = Self(1 << 3);
    pub const NO_CACHE: Self = Self(1 << 4);
    pub const ACCESSED: Self = Self(1 << 5);
    pub const DIRTY: Self = Self(1 << 6);
    pub const HUGE_PAGE: Self = Self(1 << 7);
    pub const GLOBAL: Self = Self(1 << 8);

    pub const NO_EXECUTE: Self = Self(1 << 63);

    pub const fn bits(self) -> u64 {
        self.0
    }

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn remove(self, other: Self) -> Self {
        Self(self.0 & !other.0)
    }
}

impl core::ops::BitOr for PageTableFlags {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self {
        self.union(rhs)
    }
}

impl core::ops::BitOrAssign for PageTableFlags {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl core::ops::BitAnd for PageTableFlags {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}

impl fmt::Debug for PageTableFlags {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        write!(f, "PageTableFlags({:#x})", self.0)
    }
}

/// An x86-64 page-table entry.
///
/// Bits 12..=51 contain the physical address.
/// The remaining bits contain architecture-defined flags.
#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct PageTableEntry(u64);

impl PageTableEntry {
    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn bits(self) -> u64 {
        self.0
    }

    pub const fn is_present(self) -> bool {
        self.0 & PageTableFlags::PRESENT.bits() != 0
    }

    pub const fn flags(self) -> PageTableFlags {
        PageTableFlags(
            self.0
                & (
                    0x8000_0000_0000_0000
                        | 0x0000_0000_0000_0fff
                ),
        )
    }

    pub fn frame(self) -> Option<PhysFrame> {
        if !self.is_present() {
            return None;
        }

        let address =
            self.0 & 0x000f_ffff_ffff_f000;

        PhysFrame::from_start_address(
            PhysAddr::new(address)
        )
    }

    pub fn set(
        &mut self,
        frame: PhysFrame,
        flags: PageTableFlags,
    ) {
        self.0 =
            (frame.start_address().as_u64()
                & 0x000f_ffff_ffff_f000)
                | flags.bits();
    }

    pub fn clear(&mut self) {
        self.0 = 0;
    }
}

impl fmt::Debug for PageTableEntry {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        if let Some(frame) = self.frame() {
            f.debug_struct("PageTableEntry")
                .field("frame", &frame)
                .field("flags", &self.flags())
                .finish()
        } else {
            f.write_str("PageTableEntry::Empty")
        }
    }
}

/// One 4 KiB x86-64 page table.
///
/// Used for:
///
/// - PML4
/// - PDPT
/// - Page directory
/// - Page table
#[repr(C, align(4096))]
pub struct PageTable {
    entries: [PageTableEntry; PAGE_TABLE_ENTRIES],
}

impl PageTable {
    pub const fn new() -> Self {
        Self {
            entries: [
                PageTableEntry::empty();
                PAGE_TABLE_ENTRIES
            ],
        }
    }

    pub fn zero(&mut self) {
        for entry in &mut self.entries {
            entry.clear();
        }
    }

    pub fn entry(
        &self,
        index: usize,
    ) -> &PageTableEntry {
        &self.entries[index]
    }

    pub fn entry_mut(
        &mut self,
        index: usize,
    ) -> &mut PageTableEntry {
        &mut self.entries[index]
    }

    pub fn entries(
        &self,
    ) -> &[PageTableEntry; PAGE_TABLE_ENTRIES] {
        &self.entries
    }

    pub fn entries_mut(
        &mut self,
    ) -> &mut [PageTableEntry; PAGE_TABLE_ENTRIES] {
        &mut self.entries
    }
}

impl Default for PageTable {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapperError {
    PageAlreadyMapped,
    PageNotMapped,
    InvalidPageTable,
    InvalidFrame,
    OutOfFrames,
    HugePage,
    HhdmAddressOverflow,
}

#[derive(Debug, Clone, Copy)]
pub struct UnmapResult {
    pub frame: PhysFrame,
}

/// x86-64 page-table mapper.
///
/// The mapper does NOT borrow or own the frame allocator.
///
/// This is intentional. A mapper can therefore coexist with direct
/// allocations/deallocations from the physical frame allocator.
///
/// The frame allocator is passed only to operations that may need to
/// allocate intermediate page tables.
pub struct Mapper {
    pml4_frame: PhysFrame,
    hhdm_offset: u64,
}

impl Mapper {
    /// Create a mapper for a PML4.
    ///
    /// # Safety
    ///
    /// `pml4_frame` must point to the active/valid x86-64 PML4 and
    /// `hhdm_offset` must provide access to physical memory.
    pub unsafe fn new(
        pml4_frame: PhysFrame,
        hhdm_offset: u64,
    ) -> Self {
        Self {
            pml4_frame,
            hhdm_offset,
        }
    }

    pub const fn pml4_frame(&self) -> PhysFrame {
        self.pml4_frame
    }

    fn phys_to_virt(
        &self,
        address: PhysAddr,
    ) -> Result<*mut u8, MapperError> {
        let virtual_address =
            self.hhdm_offset
                .checked_add(address.as_u64())
                .ok_or(
                    MapperError::HhdmAddressOverflow
                )?;

        Ok(virtual_address as *mut u8)
    }

    /// Access a physical page table through the HHDM.
    ///
    /// # Safety
    ///
    /// `frame` must contain a valid page table and the HHDM must map it.
    unsafe fn table_from_frame(
        &self,
        frame: PhysFrame,
    ) -> &'static mut PageTable {
        let pointer =
            self.phys_to_virt(
                frame.start_address()
            )
            .expect("HHDM address overflow")
            as *mut PageTable;

        &mut *pointer
    }

    /// Get the current PML4.
    ///
    /// # Safety
    ///
    /// The configured PML4 must be valid and accessible through the HHDM.
    unsafe fn pml4(
        &self,
    ) -> &'static mut PageTable {
        self.table_from_frame(
            self.pml4_frame
        )
    }

    /// Allocate and initialize an intermediate page table.
    fn allocate_table(
        &self,
        frame_allocator: &mut BitmapFrameAllocator,
    ) -> Result<PhysFrame, MapperError> {
        let frame =
            frame_allocator
                .allocate()
                .ok_or(
                    MapperError::OutOfFrames
                )?;

        let table = unsafe {
            self.table_from_frame(frame)
        };

        table.zero();

        Ok(frame)
    }

    /// Get or create the PDPT.
    fn get_or_create_pdpt(
        &self,
        page: Page,
        frame_allocator: &mut BitmapFrameAllocator,
    ) -> Result<&'static mut PageTable, MapperError> {
        let index =
            page.pml4_index();

        let existing = unsafe {
            self.pml4()
                .entry(index)
                .frame()
        };

        let frame = match existing {
            Some(frame) => {
                let entry = unsafe {
                    self.pml4()
                        .entry(index)
                };

                if entry
                    .flags()
                    .contains(PageTableFlags::HUGE_PAGE)
                {
                    return Err(
                        MapperError::HugePage
                    );
                }

                frame
            }

            None => {
                let frame =
                    self.allocate_table(
                        frame_allocator
                    )?;

                unsafe {
                    self.pml4()
                        .entry_mut(index)
                        .set(
                            frame,
                            PageTableFlags::PRESENT
                                | PageTableFlags::WRITABLE,
                        );
                }

                frame
            }
        };

        Ok(unsafe {
            self.table_from_frame(frame)
        })
    }

    /// Get or create the page directory.
    fn get_or_create_pd(
        &self,
        page: Page,
        frame_allocator: &mut BitmapFrameAllocator,
    ) -> Result<&'static mut PageTable, MapperError> {
        let pdpt =
            self.get_or_create_pdpt(
                page,
                frame_allocator,
            )?;

        let index =
            page.pdpt_index();

        let entry =
            pdpt.entry(index);

        if entry.is_present()
            && entry
                .flags()
                .contains(PageTableFlags::HUGE_PAGE)
        {
            return Err(
                MapperError::HugePage
            );
        }

        let frame = match entry.frame() {
            Some(frame) => frame,

            None => {
                let frame =
                    self.allocate_table(
                        frame_allocator
                    )?;

                pdpt.entry_mut(index)
                    .set(
                        frame,
                        PageTableFlags::PRESENT
                            | PageTableFlags::WRITABLE,
                    );

                frame
            }
        };

        Ok(unsafe {
            self.table_from_frame(frame)
        })
    }

    /// Get or create the page table.
    fn get_or_create_pt(
        &self,
        page: Page,
        frame_allocator: &mut BitmapFrameAllocator,
    ) -> Result<&'static mut PageTable, MapperError> {
        let pd =
            self.get_or_create_pd(
                page,
                frame_allocator,
            )?;

        let index =
            page.pd_index();

        let entry =
            pd.entry(index);

        if entry.is_present()
            && entry
                .flags()
                .contains(PageTableFlags::HUGE_PAGE)
        {
            return Err(
                MapperError::HugePage
            );
        }

        let frame = match entry.frame() {
            Some(frame) => frame,

            None => {
                let frame =
                    self.allocate_table(
                        frame_allocator
                    )?;

                pd.entry_mut(index)
                    .set(
                        frame,
                        PageTableFlags::PRESENT
                            | PageTableFlags::WRITABLE,
                    );

                frame
            }
        };

        Ok(unsafe {
            self.table_from_frame(frame)
        })
    }

    /// Get or create a PTE.
    fn get_or_create_pte(
        &self,
        page: Page,
        frame_allocator: &mut BitmapFrameAllocator,
    ) -> Result<&'static mut PageTableEntry, MapperError> {
        let pt =
            self.get_or_create_pt(
                page,
                frame_allocator,
            )?;

        Ok(
            pt.entry_mut(
                page.pt_index()
            )
        )
    }

    /// Find an existing PTE.
    ///
    /// This function NEVER allocates page tables.
    fn find_pte(
        &self,
        page: Page,
    ) -> Result<&'static mut PageTableEntry, MapperError> {
        let pml4 = unsafe {
            self.pml4()
        };

        let pml4_entry =
            pml4.entry(
                page.pml4_index()
            );

        if !pml4_entry.is_present() {
            return Err(
                MapperError::PageNotMapped
            );
        }

        let pdpt_frame =
            pml4_entry
                .frame()
                .ok_or(
                    MapperError::InvalidPageTable
                )?;

        let pdpt = unsafe {
            self.table_from_frame(
                pdpt_frame
            )
        };

        let pdpt_entry =
            pdpt.entry(
                page.pdpt_index()
            );

        if !pdpt_entry.is_present() {
            return Err(
                MapperError::PageNotMapped
            );
        }

        if pdpt_entry
            .flags()
            .contains(PageTableFlags::HUGE_PAGE)
        {
            return Err(
                MapperError::HugePage
            );
        }

        let pd_frame =
            pdpt_entry
                .frame()
                .ok_or(
                    MapperError::InvalidPageTable
                )?;

        let pd = unsafe {
            self.table_from_frame(
                pd_frame
            )
        };

        let pd_entry =
            pd.entry(
                page.pd_index()
            );

        if !pd_entry.is_present() {
            return Err(
                MapperError::PageNotMapped
            );
        }

        if pd_entry
            .flags()
            .contains(PageTableFlags::HUGE_PAGE)
        {
            return Err(
                MapperError::HugePage
            );
        }

        let pt_frame =
            pd_entry
                .frame()
                .ok_or(
                    MapperError::InvalidPageTable
                )?;

        let pt = unsafe {
            self.table_from_frame(
                pt_frame
            )
        };

        Ok(
            pt.entry_mut(
                page.pt_index()
            )
        )
    }

    /// Map a 4 KiB virtual page to a physical frame.
    ///
    /// Intermediate page tables are allocated as necessary.
    pub fn map(
        &self,
        page: Page,
        frame: PhysFrame,
        flags: PageTableFlags,
        frame_allocator: &mut BitmapFrameAllocator,
    ) -> Result<(), MapperError> {
        let entry =
            self.get_or_create_pte(
                page,
                frame_allocator,
            )?;

        if entry.is_present() {
            return Err(
                MapperError::PageAlreadyMapped
            );
        }

        entry.set(
            frame,
            flags | PageTableFlags::PRESENT,
        );

        flush_tlb(
            page.start_address()
        );

        Ok(())
    }

    /// Unmap a 4 KiB virtual page.
    ///
    /// The physical frame is returned to the caller.
    ///
    /// This function does NOT return the frame to the frame allocator.
    pub fn unmap(
        &self,
        page: Page,
    ) -> Result<UnmapResult, MapperError> {
        let entry =
            self.find_pte(page)?;

        if !entry.is_present() {
            return Err(
                MapperError::PageNotMapped
            );
        }

        let frame =
            entry.frame()
                .ok_or(
                    MapperError::InvalidFrame
                )?;

        entry.clear();

        flush_tlb(
            page.start_address()
        );

        Ok(UnmapResult { frame })
    }

    /// Translate a virtual address to a physical address.
    ///
    /// Returns `None` if the address is unmapped.
    pub fn translate(
        &self,
        address: VirtAddr,
    ) -> Option<PhysAddr> {
        if !is_canonical(
            address.as_u64()
        ) {
            return None;
        }

        let page =
            Page::containing_address(
                address
            );

        let offset =
            address.as_u64()
                & (FRAME_SIZE - 1);

        let pml4 = unsafe {
            self.pml4()
        };

        let pml4_entry =
            pml4.entry(
                page.pml4_index()
            );

        if !pml4_entry.is_present() {
            return None;
        }

        let pdpt_frame =
            pml4_entry.frame()?;

        let pdpt = unsafe {
            self.table_from_frame(
                pdpt_frame
            )
        };

        let pdpt_entry =
            pdpt.entry(
                page.pdpt_index()
            );

        if !pdpt_entry.is_present() {
            return None;
        }

        // 1 GiB pages are intentionally not implemented yet.
        if pdpt_entry
            .flags()
            .contains(PageTableFlags::HUGE_PAGE)
        {
            return None;
        }

        let pd_frame =
            pdpt_entry.frame()?;

        let pd = unsafe {
            self.table_from_frame(
                pd_frame
            )
        };

        let pd_entry =
            pd.entry(
                page.pd_index()
            );

        if !pd_entry.is_present() {
            return None;
        }

        // 2 MiB pages are intentionally not implemented yet.
        if pd_entry
            .flags()
            .contains(PageTableFlags::HUGE_PAGE)
        {
            return None;
        }

        let pt_frame =
            pd_entry.frame()?;

        let pt = unsafe {
            self.table_from_frame(
                pt_frame
            )
        };

        let pte =
            pt.entry(
                page.pt_index()
            );

        let frame =
            pte.frame()?;

        Some(
            PhysAddr::new(
                frame.start_address()
                    .as_u64()
                    + offset
            )
        )
    }

    /// Translate a virtual page to its physical frame.
    pub fn translate_page(
        &self,
        page: Page,
    ) -> Option<PhysFrame> {
        self.translate(
            page.start_address()
        )
        .and_then(|address| {
            PhysFrame::from_start_address(
                PhysAddr::new(
                    address.as_u64()
                        & !(FRAME_SIZE - 1)
                )
            )
        })
    }
}

/// Read CR3 and return the physical frame containing the active PML4.
pub fn read_cr3() -> PhysFrame {
    let value: u64;

    unsafe {
        core::arch::asm!(
            "mov {}, cr3",
            out(reg) value,
            options(
                nomem,
                nostack,
                preserves_flags
            )
        );
    }

    let address =
        value & 0x000f_ffff_ffff_f000;

    PhysFrame::from_start_address(
        PhysAddr::new(address)
    )
    .expect(
        "CR3 contains an invalid PML4 address"
    )
}

/// Check whether an x86-64 virtual address is canonical.
///
/// This assumes a 48-bit virtual address space.
pub const fn is_canonical(
    address: u64,
) -> bool {
    let upper =
        address >> 48;

    upper == 0 || upper == 0xffff
}

/// Invalidate a single TLB entry.
#[inline]
fn flush_tlb(
    address: VirtAddr,
) {
    #[cfg(target_arch = "x86_64")]
    unsafe {
        core::arch::asm!(
            "invlpg [{}]",
            in(reg) address.as_u64(),
            options(
                nostack,
                preserves_flags
            )
        );
    }

    #[cfg(not(target_arch = "x86_64"))]
    {
        let _ = address;
    }
}
