//! Kernel heap allocator.
//!
//! The heap consists of three layers:
//!
//! ```text
//!     Kernel allocation
//!            |
//!            v
//!     Linked-list allocator
//!            |
//!            v
//!     Virtual heap pages
//!            |
//!            v
//!     Physical frames
//! ```
//!
//! The allocator is based on the linked-list allocator described in
//! Philipp Oppermann's Rust OS development series.

use core::{
    alloc::{
        GlobalAlloc,
        Layout,
    },
    mem::{
        align_of,
        size_of,
    },
    ptr,
};

use spin::Mutex;

use x86_64::{
    structures::paging::{
        FrameAllocator,
        mapper::MapToError,
        Mapper,
        Page,
        PageTableFlags,
        Size4KiB,
    },
    VirtAddr,
};

/// Start address of the kernel heap.
pub const HEAP_START: usize =
    0x_4444_4444_0000;

/// Initial kernel heap size.
///
/// This can be increased later or replaced with a dynamically growing heap.
pub const HEAP_SIZE: usize =
    1024 * 1024 * 100; // 100 MiB

/// Global kernel allocator.
#[global_allocator]
pub static ALLOCATOR: LockedHeap =
    LockedHeap::new();

/// Initialize the kernel heap.
///
/// This maps the heap's virtual address range to physical frames and then
/// initializes the linked-list allocator.
///
/// # Safety
///
/// The caller must guarantee that:
///
/// - `mapper` is a valid page-table mapper.
/// - `frame_allocator` returns valid unused physical frames.
/// - The heap virtual-address range is not already in use.
/// - The mapped frames are exclusively owned by the heap.
pub unsafe fn init_heap(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), MapToError<Size4KiB>> {
    let heap_start =
        VirtAddr::new(HEAP_START as u64);

    let heap_end =
        heap_start
            + (HEAP_SIZE as u64)
            - 1;

    let heap_start_page =
        Page::containing_address(heap_start);

    let heap_end_page =
        Page::containing_address(heap_end);

    let flags =
        PageTableFlags::PRESENT
        | PageTableFlags::WRITABLE;

    for page in Page::range_inclusive(
        heap_start_page,
        heap_end_page,
    ) {
        let frame =
            frame_allocator
                .allocate_frame()
                .ok_or(
                    MapToError::FrameAllocationFailed,
                )?;

        unsafe {
            mapper
                .map_to(
                    page,
                    frame,
                    flags,
                    frame_allocator,
                )?
                .flush();
        }
    }

    unsafe {
        ALLOCATOR.inner.lock().init(
            HEAP_START,
            HEAP_SIZE,
        );
    }

    Ok(())
}

/// Thread-safe wrapper around the heap allocator.
pub struct LockedHeap {
    inner: Mutex<ListAllocator>,
}

impl LockedHeap {
    /// Create an uninitialized heap allocator.
    pub const fn new() -> Self {
        Self {
            inner: Mutex::new(
                ListAllocator::new(),
            ),
        }
    }

    /// Initialize the heap allocator.
    ///
    /// # Safety
    ///
    /// The supplied memory range must be valid writable memory and must not
    /// overlap any existing allocation.
    pub unsafe fn init(
        &self,
        heap_start: usize,
        heap_size: usize,
    ) {
        unsafe {
            self.inner
                .lock()
                .init(
                    heap_start,
                    heap_size,
                );
        }
    }
}

unsafe impl GlobalAlloc for LockedHeap {
    unsafe fn alloc(
        &self,
        layout: Layout,
    ) -> *mut u8 {
        self.inner
            .lock()
            .alloc(layout)
    }

    unsafe fn dealloc(
        &self,
        ptr: *mut u8,
        layout: Layout,
    ) {
        self.inner
            .lock()
            .dealloc(
                ptr,
                layout,
            );
    }
}

/// A simple first-fit linked-list allocator.
///
/// Free memory is stored as a linked list of `ListNode`s. The nodes
/// themselves live inside the free regions.
///
/// ```text
/// +-------------------+
/// | ListNode           |
/// | size / next        |
/// +-------------------+
/// | free memory        |
/// |                   |
/// +-------------------+
///          |
///          v
/// +-------------------+
/// | ListNode           |
/// | size / next        |
/// +-------------------+
/// | free memory        |
/// +-------------------+
/// ```
struct ListAllocator {
    head: ListNode,
    initialized: bool,
}

impl ListAllocator {
    const fn new() -> Self {
        Self {
            head: ListNode {
                size: 0,
                next: None,
            },
            initialized: false,
        }
    }

    /// Initialize the allocator over a contiguous memory range.
    ///
    /// # Safety
    ///
    /// `heap_start..heap_start + heap_size` must be valid writable memory
    /// that is exclusively owned by this allocator.
    unsafe fn init(
        &mut self,
        heap_start: usize,
        heap_size: usize,
    ) {
        assert!(
            heap_start
                % align_of::<ListNode>()
                == 0,
            "heap start is not properly aligned"
        );

        assert!(
            heap_size >= size_of::<ListNode>(),
            "heap is too small"
        );

        self.initialized = true;

        unsafe {
            self.add_free_region(
                heap_start,
                heap_size,
            );
        }
    }

    /// Add a region to the free list.
    ///
    /// # Safety
    ///
    /// The supplied memory range must be valid, aligned, and unused.
    unsafe fn add_free_region(
        &mut self,
        addr: usize,
        size: usize,
    ) {
        assert!(
            addr
                % align_of::<ListNode>()
                == 0,
            "free region is not aligned"
        );

        assert!(
            size >= size_of::<ListNode>(),
            "free region is too small"
        );

        let node_ptr =
            addr as *mut ListNode;

        unsafe {
            node_ptr.write(ListNode {
                size,
                next: self.head.next.take(),
            });

            self.head.next =
                Some(&mut *node_ptr);
        }
    }

    /// Allocate a block from the heap.
    fn alloc(
        &mut self,
        layout: Layout,
    ) -> *mut u8 {
        if !self.initialized {
            return ptr::null_mut();
        }

        let layout =
            match adjust_layout(layout) {
                Some(layout) => layout,
                None => {
                    return ptr::null_mut()
                }
            };

        let mut current =
            &mut self.head;

        while let Some(region) =
            current.next.as_mut()
        {
            if let Ok(alloc_start) =
                Self::alloc_from_region(
                    region,
                    layout,
                )
            {
                let alloc_end =
                    match alloc_start
                        .checked_add(
                            layout.size(),
                        )
                    {
                        Some(value) => value,
                        None => {
                            return ptr::null_mut()
                        }
                    };

                let region_end =
                    region
                        .start()
                        .checked_add(
                            region.size,
                        )
                        .unwrap();

                let excess_size =
                    region_end
                        - alloc_end;

                let next =
                    region.next.take();

                if excess_size
                    >= size_of::<ListNode>()
                {
                    let excess_start =
                        alloc_end;

                    let excess =
                        excess_start
                            as *mut ListNode;

                    unsafe {
                        excess.write(
                            ListNode {
                                size: excess_size,
                                next,
                            },
                        );

                        current.next =
                            Some(&mut *excess);
                    }
                } else {
                    // The remaining bytes are too small to represent a
                    // free-list node, so consume the entire region.
                    current.next = next;
                }

                return alloc_start as *mut u8;
            }

            current =
                current.next
                    .as_mut()
                    .unwrap();
        }

        ptr::null_mut()
    }

    /// Deallocate a previously allocated block.
    fn dealloc(
        &mut self,
        ptr: *mut u8,
        layout: Layout,
    ) {
        if ptr.is_null() {
            return;
        }

        let layout =
            match adjust_layout(layout) {
                Some(layout) => layout,
                None => return,
            };

        unsafe {
            self.add_free_region(
                ptr as usize,
                layout.size(),
            );
        }

        // TODO:
        //
        // Merge adjacent free regions.
        //
        // Without coalescing, repeated allocation/deallocation can
        // eventually fragment the heap.
    }

    /// Attempt to allocate a block from a specific free region.
    fn alloc_from_region(
        region: &ListNode,
        layout: Layout,
    ) -> Result<usize, ()> {
        let alloc_start =
            align_up(
                region.start(),
                layout.align(),
            );

        let alloc_end =
            alloc_start
                .checked_add(
                    layout.size(),
                )
                .ok_or(())?;

        let region_end =
            region
                .start()
                .checked_add(
                    region.size,
                )
                .ok_or(())?;

        if alloc_end > region_end {
            return Err(());
        }

        let excess_size =
            region_end - alloc_end;

        // If there is remaining space, it must be large enough to hold
        // another ListNode. Otherwise this allocation would leave an
        // unusable fragment.
        if excess_size > 0
            && excess_size < size_of::<ListNode>()
        {
            return Err(());
        }

        Ok(alloc_start)
    }
}

/// A node describing a free memory region.
struct ListNode {
    size: usize,
    next: Option<&'static mut ListNode>,
}

impl ListNode {
    fn start(&self) -> usize {
        self as *const Self as usize
    }
}

/// Adjust an allocation layout so that it can safely be represented by
/// the linked-list allocator.
fn adjust_layout(
    layout: Layout,
) -> Option<Layout> {
    let size =
        layout
            .size()
            .max(size_of::<ListNode>());

    let align =
        layout
            .align()
            .max(align_of::<ListNode>());

    Layout::from_size_align(
        size,
        align,
    )
    .ok()
}

/// Align an address upwards.
#[inline]
fn align_up(
    addr: usize,
    align: usize,
) -> usize {
    debug_assert!(
        align.is_power_of_two()
    );

    (addr + align - 1)
        & !(align - 1)
}
