//! Kernel memory-management tests.
//!
//! These tests exercise the public heap allocator through the
//! `ALLOCATOR` defined in `kmem::heap`.

use core::{
    alloc::{
        GlobalAlloc,
        Layout,
    },
    ptr,
};

use crate::{
    kmem::heap::ALLOCATOR,
    test,
    test::TestResult,
};

// ============================================================
// Helpers
// ============================================================

/// Allocate memory from the kernel heap.
#[inline]
unsafe fn allocate(
    layout: Layout,
) -> *mut u8 {
    ALLOCATOR.alloc(layout)
}

/// Free memory back to the kernel heap.
#[inline]
unsafe fn deallocate(
    ptr: *mut u8,
    layout: Layout,
) {
    ALLOCATOR.dealloc(
        ptr,
        layout,
    );
}

// ============================================================
// Basic allocation
// ============================================================

#[test]
fn kmem_allocates_small_block() -> TestResult {
    let layout =
        match Layout::from_size_align(
            8,
            8,
        ) {
            Ok(layout) => layout,
            Err(_) => {
                return TestResult::Fail(
                    "failed to construct allocation layout",
                );
            }
        };

    let ptr =
        unsafe {
            allocate(layout)
        };

    if ptr.is_null() {
        return TestResult::Fail(
            "small allocation returned null",
        );
    }

    unsafe {
        deallocate(
            ptr,
            layout,
        );
    }

    TestResult::Pass
}

#[test]
fn kmem_allocates_multiple_blocks() -> TestResult {
    let layout =
        match Layout::from_size_align(
            64,
            8,
        ) {
            Ok(layout) => layout,
            Err(_) => {
                return TestResult::Fail(
                    "failed to construct allocation layout",
                );
            }
        };

    let a =
        unsafe {
            allocate(layout)
        };

    let b =
        unsafe {
            allocate(layout)
        };

    let c =
        unsafe {
            allocate(layout)
        };

    if a.is_null()
        || b.is_null()
        || c.is_null()
    {
        unsafe {
            if !a.is_null() {
                deallocate(
                    a,
                    layout,
                );
            }

            if !b.is_null() {
                deallocate(
                    b,
                    layout,
                );
            }

            if !c.is_null() {
                deallocate(
                    c,
                    layout,
                );
            }
        }

        return TestResult::Fail(
            "one or more allocations returned null",
        );
    }

    if a == b
        || a == c
        || b == c
    {
        unsafe {
            deallocate(
                a,
                layout,
            );
            deallocate(
                b,
                layout,
            );
            deallocate(
                c,
                layout,
            );
        }

        return TestResult::Fail(
            "allocator returned duplicate addresses",
        );
    }

    unsafe {
        deallocate(
            a,
            layout,
        );
        deallocate(
            b,
            layout,
        );
        deallocate(
            c,
            layout,
        );
    }

    TestResult::Pass
}

// ============================================================
// Alignment
// ============================================================

#[test]
fn kmem_respects_alignment() -> TestResult {
    let layouts = [
        (1usize, 1usize),
        (8, 2),
        (16, 4),
        (32, 8),
        (32, 16),
        (64, 32),
        (128, 64),
        (256, 128),
    ];

    for &(size, alignment) in &layouts {
        let layout =
            match Layout::from_size_align(
                size,
                alignment,
            ) {
                Ok(layout) => layout,
                Err(_) => {
                    return TestResult::Fail(
                        "failed to construct alignment layout",
                    );
                }
            };

        let ptr =
            unsafe {
                allocate(layout)
            };

        if ptr.is_null() {
            return TestResult::Fail(
                "aligned allocation returned null",
            );
        }

        if (ptr as usize) % alignment != 0 {
            unsafe {
                deallocate(
                    ptr,
                    layout,
                );
            }

            return TestResult::Fail(
                "allocation does not satisfy requested alignment",
            );
        }

        unsafe {
            deallocate(
                ptr,
                layout,
            );
        }
    }

    TestResult::Pass
}

// ============================================================
// Memory access
// ============================================================

#[test]
fn kmem_allocated_memory_is_writable() -> TestResult {
    let layout =
        match Layout::from_size_align(
            4096,
            8,
        ) {
            Ok(layout) => layout,
            Err(_) => {
                return TestResult::Fail(
                    "failed to construct layout",
                );
            }
        };

    let ptr =
        unsafe {
            allocate(layout)
        };

    if ptr.is_null() {
        return TestResult::Fail(
            "allocation returned null",
        );
    }

    unsafe {
        for i in 0..4096 {
            ptr.add(i)
                .write((i & 0xff) as u8);
        }

        for i in 0..4096 {
            let value =
                ptr.add(i)
                    .read();

            if value
                != (i & 0xff) as u8
            {
                deallocate(
                    ptr,
                    layout,
                );

                return TestResult::Fail(
                    "memory read-back did not match written data",
                );
            }
        }

        deallocate(
            ptr,
            layout,
        );
    }

    TestResult::Pass
}

// ============================================================
// Free
// ============================================================

#[test]
fn kmem_can_free_memory() -> TestResult {
    let layout =
        match Layout::from_size_align(
            128,
            8,
        ) {
            Ok(layout) => layout,
            Err(_) => {
                return TestResult::Fail(
                    "failed to construct layout",
                );
            }
        };

    let ptr =
        unsafe {
            allocate(layout)
        };

    if ptr.is_null() {
        return TestResult::Fail(
            "initial allocation failed",
        );
    }

    unsafe {
        deallocate(
            ptr,
            layout,
        );
    }

    let ptr2 =
        unsafe {
            allocate(layout)
        };

    if ptr2.is_null() {
        return TestResult::Fail(
            "allocation after free failed",
        );
    }

    unsafe {
        deallocate(
            ptr2,
            layout,
        );
    }

    TestResult::Pass
}

#[test]
fn kmem_reuses_freed_memory() -> TestResult {
    let layout =
        match Layout::from_size_align(
            256,
            8,
        ) {
            Ok(layout) => layout,
            Err(_) => {
                return TestResult::Fail(
                    "failed to construct layout",
                );
            }
        };

    let first =
        unsafe {
            allocate(layout)
        };

    if first.is_null() {
        return TestResult::Fail(
            "initial allocation failed",
        );
    }

    unsafe {
        deallocate(
            first,
            layout,
        );
    }

    let second =
        unsafe {
            allocate(layout)
        };

    if second.is_null() {
        return TestResult::Fail(
            "allocation after free failed",
        );
    }

    let reused =
        first == second;

    unsafe {
        deallocate(
            second,
            layout,
        );
    }

    if !reused {
        return TestResult::Fail(
            "allocator did not reuse recently freed memory",
        );
    }

    TestResult::Pass
}

// ============================================================
// Different sizes
// ============================================================

#[test]
fn kmem_handles_different_sizes() -> TestResult {
    let sizes = [
        1usize,
        2,
        4,
        8,
        16,
        32,
        64,
        128,
        256,
        512,
        1024,
    ];

    let mut allocations =
        [ptr::null_mut::<u8>(); 11];

    let mut layouts =
        [None::<Layout>; 11];

    for i in 0..sizes.len() {
        let layout =
            match Layout::from_size_align(
                sizes[i],
                8,
            ) {
                Ok(layout) => layout,
                Err(_) => {
                    return TestResult::Fail(
                        "failed to construct size layout",
                    );
                }
            };

        let allocation =
            unsafe {
                allocate(layout)
            };

        if allocation.is_null() {
            for j in 0..i {
                if !allocations[j].is_null() {
                    unsafe {
                        deallocate(
                            allocations[j],
                            layouts[j].unwrap(),
                        );
                    }
                }
            }

            return TestResult::Fail(
                "allocation failed for one of the requested sizes",
            );
        }

        allocations[i] =
            allocation;

        layouts[i] =
            Some(layout);
    }

    for i in 0..sizes.len() {
        unsafe {
            deallocate(
                allocations[i],
                layouts[i].unwrap(),
            );
        }
    }

    TestResult::Pass
}

// ============================================================
// Many allocations
// ============================================================

#[test]
fn kmem_handles_many_allocations() -> TestResult {
    let layout =
        match Layout::from_size_align(
            64,
            8,
        ) {
            Ok(layout) => layout,
            Err(_) => {
                return TestResult::Fail(
                    "failed to construct layout",
                );
            }
        };

    let mut allocations =
        [ptr::null_mut::<u8>(); 128];

    for i in 0..allocations.len() {
        let allocation =
            unsafe {
                allocate(layout)
            };

        if allocation.is_null() {
            for ptr in allocations {
                if !ptr.is_null() {
                    unsafe {
                        deallocate(
                            ptr,
                            layout,
                        );
                    }
                }
            }

            return TestResult::Fail(
                "allocator failed during repeated allocation",
            );
        }

        allocations[i] =
            allocation;
    }

    // Ensure every allocation has a unique address.
    for i in 0..allocations.len() {
        for j in (i + 1)..allocations.len() {
            if allocations[i]
                == allocations[j]
            {
                for ptr in allocations {
                    if !ptr.is_null() {
                        unsafe {
                            deallocate(
                                ptr,
                                layout,
                            );
                        }
                    }
                }

                return TestResult::Fail(
                    "allocator returned duplicate addresses",
                );
            }
        }
    }

    for ptr in allocations {
        unsafe {
            deallocate(
                ptr,
                layout,
            );
        }
    }

    TestResult::Pass
}

// ============================================================
// Fragmentation
// ============================================================

#[test]
fn kmem_handles_fragmentation() -> TestResult {
    let layout =
        match Layout::from_size_align(
            128,
            8,
        ) {
            Ok(layout) => layout,
            Err(_) => {
                return TestResult::Fail(
                    "failed to construct layout",
                );
            }
        };

    let a =
        unsafe {
            allocate(layout)
        };

    let b =
        unsafe {
            allocate(layout)
        };

    let c =
        unsafe {
            allocate(layout)
        };

    if a.is_null()
        || b.is_null()
        || c.is_null()
    {
        unsafe {
            if !a.is_null() {
                deallocate(
                    a,
                    layout,
                );
            }

            if !b.is_null() {
                deallocate(
                    b,
                    layout,
                );
            }

            if !c.is_null() {
                deallocate(
                    c,
                    layout,
                );
            }
        }

        return TestResult::Fail(
            "initial allocations failed",
        );
    }

    // Free the middle allocation.
    unsafe {
        deallocate(
            b,
            layout,
        );
    }

    let d =
        unsafe {
            allocate(layout)
        };

    if d.is_null() {
        unsafe {
            deallocate(
                a,
                layout,
            );
            deallocate(
                c,
                layout,
            );
        }

        return TestResult::Fail(
            "allocator failed after fragmentation",
        );
    }

    unsafe {
        deallocate(
            a,
            layout,
        );
        deallocate(
            c,
            layout,
        );
        deallocate(
            d,
            layout,
        );
    }

    TestResult::Pass
}

// ============================================================
// Large allocation
// ============================================================

#[test]
fn kmem_allocates_large_block() -> TestResult {
    let layout =
        match Layout::from_size_align(
            16 * 1024,
            16,
        ) {
            Ok(layout) => layout,
            Err(_) => {
                return TestResult::Fail(
                    "failed to construct large layout",
                );
            }
        };

    let ptr =
        unsafe {
            allocate(layout)
        };

    if ptr.is_null() {
        return TestResult::Fail(
            "large allocation failed",
        );
    }

    if (ptr as usize) % 16 != 0 {
        unsafe {
            deallocate(
                ptr,
                layout,
            );
        }

        return TestResult::Fail(
            "large allocation has invalid alignment",
        );
    }

    unsafe {
        deallocate(
            ptr,
            layout,
        );
    }

    TestResult::Pass
}

// ============================================================
// High alignment
// ============================================================

#[test]
fn kmem_handles_high_alignment() -> TestResult {
    let layout =
        match Layout::from_size_align(
            128,
            256,
        ) {
            Ok(layout) => layout,
            Err(_) => {
                return TestResult::Fail(
                    "failed to construct high-alignment layout",
                );
            }
        };

    let ptr =
        unsafe {
            allocate(layout)
        };

    if ptr.is_null() {
        return TestResult::Fail(
            "high-alignment allocation failed",
        );
    }

    if (ptr as usize) % 256 != 0 {
        unsafe {
            deallocate(
                ptr,
                layout,
            );
        }

        return TestResult::Fail(
            "allocation is not 256-byte aligned",
        );
    }

    unsafe {
        deallocate(
            ptr,
            layout,
        );
    }

    TestResult::Pass
}
