use super::frame_alloc::{
    BitmapFrameAllocator,
    FrameAllocator,
    PhysFrame,
};

use super::page_alloc::{
    BitmapPageAllocator,
    Page,
    PageAllocatorError,
    PageRange,
    VirtAddr,
};

use super::page_table::{
    Mapper,
    MapperError,
    PageTableFlags,
};

/// Errors produced by the kernel address-space manager.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressSpaceError {
    PageAllocator(
        PageAllocatorError
    ),

    Mapper(
        MapperError
    ),

    OutOfVirtualPages,

    OutOfPhysicalFrames,

    MappingFailed,

    InvalidRange,

    NotMapped,
}

impl From<PageAllocatorError>
    for AddressSpaceError
{
    fn from(
        error: PageAllocatorError,
    ) -> Self {
        Self::PageAllocator(error)
    }
}

impl From<MapperError>
    for AddressSpaceError
{
    fn from(
        error: MapperError,
    ) -> Self {
        Self::Mapper(error)
    }
}

/// A page range together with the physical frames backing it.
#[derive(Debug, Clone, Copy)]
pub struct Allocation {
    pub start: Page,
    pub page_count: usize,
}

impl Allocation {
    pub fn page_at(
        &self,
        index: usize,
    ) -> Option<Page> {
        if index >= self.page_count {
            return None;
        }

        let index =
            index as u64;

        let address =
            self.start
                .start_address()
                .as_u64()
                .checked_add(
                    index * 4096
                )?;

        Page::from_start_address(
            VirtAddr::new(address)
        )
    }
}

/// Kernel virtual-memory manager.
///
/// Responsibilities:
///
/// - allocate virtual pages
/// - allocate physical frames
/// - establish mappings
/// - remove mappings
/// - return physical frames
///
/// It deliberately does NOT implement a heap.
///
/// The future heap allocator can use this type to obtain contiguous
/// virtual memory without knowing anything about page tables.
pub struct KernelAddressSpace {
    pub pages: BitmapPageAllocator,
    pub mapper: Mapper,
}

impl KernelAddressSpace {
    /// Create the kernel address-space manager.
    ///
    /// # Safety
    ///
    /// `mapper` must refer to the currently active address space.
    pub unsafe fn new(
        pages: BitmapPageAllocator,
        mapper: Mapper,
    ) -> Self {
        Self {
            pages,
            mapper,
        }
    }

    /// Allocate and map one page.
    pub fn allocate_page(
        &mut self,
        frame_allocator:
            &mut BitmapFrameAllocator,
        flags: PageTableFlags,
    ) -> Result<
        Page,
        AddressSpaceError,
    > {
        let page =
            self.pages
                .allocate()
                .ok_or(
                    AddressSpaceError::OutOfVirtualPages
                )?;

        match self.mapper.map_new(
            page,
            flags,
            frame_allocator,
        ) {
            Ok(_) => Ok(page),

            Err(error) => {
                let _ =
                    self.pages
                        .deallocate(page);

                Err(
                    AddressSpaceError::Mapper(
                        error
                    )
                )
            }
        }
    }

    /// Allocate and map multiple pages.
    ///
    /// The pages are contiguous in virtual address space.
    pub fn allocate_pages(
        &mut self,
        count: usize,
        frame_allocator:
            &mut BitmapFrameAllocator,
        flags: PageTableFlags,
    ) -> Result<
        Allocation,
        AddressSpaceError,
    > {
        if count == 0 {
            return Err(
                AddressSpaceError::InvalidRange
            );
        }

        //
        // First obtain the virtual pages.
        //

        let mut pages = [None; 64];

        if count > pages.len() {
            return Err(
                AddressSpaceError::InvalidRange
            );
        }

        for slot in
            pages.iter_mut().take(count)
        {
            *slot =
                self.pages.allocate();

            if slot.is_none() {
                //
                // Roll back virtual allocations.
                //

                for allocated
                    in pages.iter().flatten()
                {
                    let _ =
                        self.pages
                            .deallocate(
                                *allocated
                            );
                }

                return Err(
                    AddressSpaceError::OutOfVirtualPages
                );
            }
        }

        //
        // Ensure they are actually contiguous.
        //
        // The bitmap allocator normally allocates sequentially, but this
        // check makes the contract explicit.
        //

        let first =
            pages[0]
                .expect("first page missing");

        for index in 1..count {
            let expected =
                first
                    .start_address()
                    .as_u64()
                    .checked_add(
                        index as u64 * 4096
                    )
                    .ok_or(
                        AddressSpaceError::InvalidRange
                    )?;

            if pages[index]
                .expect("allocated page missing")
                .start_address()
                .as_u64()
                != expected
            {
                //
                // Roll back virtual pages.
                //

                for allocated
                    in pages.iter().flatten()
                {
                    let _ =
                        self.pages
                            .deallocate(
                                *allocated
                            );
                }

                return Err(
                    AddressSpaceError::InvalidRange
                );
            }
        }

        //
        // Map the pages.
        //

        let mut mapped = 0usize;

        for index in 0..count {
            let page =
                pages[index]
                    .expect(
                        "allocated page missing"
                    );

            match self.mapper.map_new(
                page,
                flags,
                frame_allocator,
            ) {
                Ok(_) => {
                    mapped += 1;
                }

                Err(error) => {
                    //
                    // Roll back all mappings that succeeded.
                    //

                    for rollback_index
                        in 0..mapped
                    {
                        let rollback_page =
                            pages[
                                rollback_index
                            ]
                            .expect(
                                "rollback page missing"
                            );

                        if let Ok(result) =
                            self.mapper.unmap(
                                rollback_page
                            )
                        {
                            let _ =
                                frame_allocator
                                    .deallocate(
                                        result.frame
                                    );
                        }
                    }

                    //
                    // Release all virtual pages.
                    //

                    for allocated
                        in pages.iter().flatten()
                    {
                        let _ =
                            self.pages
                                .deallocate(
                                    *allocated
                                );
                    }

                    return Err(
                        AddressSpaceError::Mapper(
                            error
                        )
                    );
                }
            }
        }

        Ok(
            Allocation {
                start: first,
                page_count: count,
            }
        )
    }

    /// Unmap and release one page.
    pub fn deallocate_page(
        &mut self,
        page: Page,
        frame_allocator:
            &mut BitmapFrameAllocator,
    ) -> Result<
        (),
        AddressSpaceError,
    > {
        let result =
            self.mapper
                .unmap(page)
                .map_err(
                    AddressSpaceError::Mapper
                )?;

        frame_allocator
            .deallocate(result.frame)
            .map_err(|_| {
                AddressSpaceError::OutOfPhysicalFrames
            })?;

        self.pages
            .deallocate(page)?;

        Ok(())
    }

    /// Reserve a virtual range without creating mappings.
    ///
    /// This is useful for regions that already have mappings established
    /// by Limine or the boot environment.
    pub fn reserve(
        &mut self,
        range: PageRange,
    ) -> Result<
        (),
        AddressSpaceError,
    > {
        self.pages
            .reserve(range)?;

        Ok(())
    }

    /// Translate a virtual address.
    pub fn translate(
        &self,
        address: VirtAddr,
    ) -> Option<
        super::frame_alloc::PhysAddr
    > {
        self.mapper
            .translate(address)
    }

    /// Check whether a virtual page is owned by the allocator.
    pub fn is_allocated(
        &self,
        page: Page,
    ) -> Result<
        bool,
        AddressSpaceError,
    > {
        Ok(
            self.pages
                .is_allocated(page)?
        )
    }
}
