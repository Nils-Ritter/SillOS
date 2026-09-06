//! A binary buddy allocator.
//!
//! Free memory is tracked as power-of-two sized blocks. Each order (block
//! size) has its own intrusive free list, stored inside the free blocks
//! themselves — no separate metadata heap is required.
//!
//! Allocating a block that isn't available at the requested order splits
//! the next larger free block in half, pushing the unused half ("buddy")
//! onto a lower free list. Freeing a block checks whether its buddy is
//! also free and, if so, merges the two into the next order up, repeating
//! until no further merge is possible. This keeps fragmentation bounded
//! without needing a coalescing pass like the linked-list allocator.

use core::{
    alloc::Layout,
    cmp,
    mem::size_of,
    ptr,
};

use super::MemoryAllocator;

/// Smallest block order the allocator will hand out (2^MIN_ORDER bytes).
const MIN_ORDER: usize = 6; // 64 bytes

/// Largest block order the allocator supports (2^MAX_ORDER bytes).
const MAX_ORDER: usize = 30; // 1 GiB

/// Number of distinct block sizes tracked by the allocator.
const ORDER_COUNT: usize = MAX_ORDER - MIN_ORDER + 1;

/// Intrusive free-list node stored inside a free block.
struct FreeListNode {
    next: *mut FreeListNode,
}

/// A binary buddy allocator.
///
/// Free memory is stored as `ORDER_COUNT` singly-linked lists, one per
/// block size (`2^MIN_ORDER ..= 2^max_order` bytes). The lists themselves
/// live inside the free blocks:
///
/// ```text
/// free_lists[order] -> [ FreeListNode | free memory ] -> [ FreeListNode | free memory ] -> null
/// ```
pub struct BuddyAllocator {
    heap_start: usize,
    heap_size: usize,
    max_order: usize,
    free_lists: [*mut FreeListNode; ORDER_COUNT],
    initialized: bool,
}

// `free_lists` holds raw pointers into heap memory owned exclusively by
// this allocator, so it is safe to move/share across threads under the
// same synchronization the caller already applies (e.g. a `Mutex`).
unsafe impl Send for BuddyAllocator {}

impl BuddyAllocator {
    /// Create an uninitialized buddy allocator.
    pub const fn new() -> Self {
        Self {
            heap_start: 0,
            heap_size: 0,
            max_order: MIN_ORDER,
            free_lists: [ptr::null_mut(); ORDER_COUNT],
            initialized: false,
        }
    }

    /// Initialize the allocator over a contiguous memory range.
    ///
    /// The range does not need to be a power-of-two size: it is greedily
    /// decomposed into the largest aligned power-of-two blocks that fit,
    /// so a `heap_size` like SillOS's 100 MiB heap works without waste
    /// beyond the final, sub-minimum-block remainder (if any).
    ///
    /// # Safety
    ///
    /// `heap_start..heap_start + heap_size` must be valid writable memory
    /// that is exclusively owned by this allocator, and `heap_start` must
    /// be aligned to at least `2^MIN_ORDER`.
    pub unsafe fn init(
        &mut self,
        heap_start: usize,
        heap_size: usize,
    ) {
        assert!(
            heap_start % Self::block_size(MIN_ORDER) == 0,
            "heap start is not aligned to the minimum block size"
        );

        assert!(
            heap_size >= Self::block_size(MIN_ORDER),
            "heap is too small"
        );

        self.heap_start = heap_start;
        self.heap_size = heap_size;
        self.max_order =
            Self::largest_order_for(heap_size).min(MAX_ORDER);
        self.free_lists = [ptr::null_mut(); ORDER_COUNT];

        let mut addr = heap_start;
        let mut remaining = heap_size;

        // Decompose the heap into the largest aligned power-of-two blocks
        // that fit, largest first, and seed each onto its free list.
        while remaining >= Self::block_size(MIN_ORDER) {
            let mut order = self.max_order;

            while order > MIN_ORDER {
                let size = Self::block_size(order);

                if size <= remaining && addr % size == 0 {
                    break;
                }

                order -= 1;
            }

            let size = Self::block_size(order);

            if size > remaining || addr % size != 0 {
                break;
            }

            unsafe {
                self.push_free_block(addr, order);
            }

            addr += size;
            remaining -= size;
        }

        self.initialized = true;
    }

    /// Block size in bytes for a given order.
    #[inline]
    const fn block_size(order: usize) -> usize {
        1 << order
    }

    /// Index into `free_lists` for a given order.
    #[inline]
    const fn index_for(order: usize) -> usize {
        order - MIN_ORDER
    }

    /// Largest order whose block size is `<= size`.
    fn largest_order_for(size: usize) -> usize {
        let mut order = MIN_ORDER;

        while order < MAX_ORDER
            && Self::block_size(order + 1) <= size
        {
            order += 1;
        }

        order
    }

    /// Smallest order able to satisfy an allocation of `size` bytes.
    fn order_for_size(size: usize) -> Option<usize> {
        let size = cmp::max(size, size_of::<FreeListNode>());
        let mut order = MIN_ORDER;

        while Self::block_size(order) < size {
            if order == MAX_ORDER {
                return None;
            }

            order += 1;
        }

        Some(order)
    }

    /// Push a free block of the given order onto its free list.
    ///
    /// # Safety
    ///
    /// `addr` must point to a valid, unused, and properly aligned region
    /// of at least `block_size(order)` bytes.
    unsafe fn push_free_block(
        &mut self,
        addr: usize,
        order: usize,
    ) {
        let node = addr as *mut FreeListNode;
        let index = Self::index_for(order);

        unsafe {
            (*node).next = self.free_lists[index];
        }

        self.free_lists[index] = node;
    }

    /// Pop a free block of the given order from its free list, if any.
    fn pop_free_block(&mut self, order: usize) -> Option<usize> {
        let index = Self::index_for(order);
        let node = self.free_lists[index];

        if node.is_null() {
            return None;
        }

        unsafe {
            self.free_lists[index] = (*node).next;
        }

        Some(node as usize)
    }

    /// Remove a specific block from its free list, returning whether it
    /// was found.
    fn remove_free_block(
        &mut self,
        addr: usize,
        order: usize,
    ) -> bool {
        let index = Self::index_for(order);
        let target = addr as *mut FreeListNode;

        let mut prev: *mut FreeListNode = ptr::null_mut();
        let mut current = self.free_lists[index];

        while !current.is_null() {
            if current == target {
                let next = unsafe { (*current).next };

                if prev.is_null() {
                    self.free_lists[index] = next;
                } else {
                    unsafe {
                        (*prev).next = next;
                    }
                }

                return true;
            }

            prev = current;

            current = unsafe { (*current).next };
        }

        false
    }

    /// Allocate a block of the given order, splitting a larger free block
    /// if none of the exact size is available.
    fn allocate_order(&mut self, order: usize) -> Option<usize> {
        if order > self.max_order {
            return None;
        }

        if let Some(addr) = self.pop_free_block(order) {
            return Some(addr);
        }

        // No free block of this order; split the next larger one and
        // keep the unused half for future allocations.
        let addr = self.allocate_order(order + 1)?;
        let buddy_addr = addr + Self::block_size(order);

        unsafe {
            self.push_free_block(buddy_addr, order);
        }

        Some(addr)
    }

    /// Compute the address of a block's buddy at the given order.
    #[inline]
    fn buddy_of(&self, addr: usize, order: usize) -> usize {
        let offset = addr - self.heap_start;

        self.heap_start + (offset ^ Self::block_size(order))
    }
}

impl MemoryAllocator for BuddyAllocator {
    unsafe fn alloc(&mut self, layout: Layout) -> *mut u8 {
        if !self.initialized {
            return ptr::null_mut();
        }

        let size = cmp::max(layout.size(), layout.align());

        let order = match Self::order_for_size(size) {
            Some(order) => order,
            None => return ptr::null_mut(),
        };

        match self.allocate_order(order) {
            Some(addr) => addr as *mut u8,
            None => ptr::null_mut(),
        }
    }

    unsafe fn dealloc(&mut self, ptr: *mut u8, layout: Layout) {
        if ptr.is_null() || !self.initialized {
            return;
        }

        let size = cmp::max(layout.size(), layout.align());

        let order = match Self::order_for_size(size) {
            Some(order) => order,
            None => return,
        };

        let mut addr = ptr as usize;
        let mut order = order;

        // Repeatedly try to merge with the buddy block until the buddy
        // is not free or we've reached the largest tracked order.
        while order < self.max_order {
            let buddy = self.buddy_of(addr, order);

            if self.remove_free_block(buddy, order) {
                addr = cmp::min(addr, buddy);
                order += 1;
            } else {
                break;
            }
        }

        unsafe {
            self.push_free_block(addr, order);
        }
    }
}
