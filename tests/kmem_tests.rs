use crate::{
    kmem,
    memory::{
        frame_alloc::FrameAllocator,
        page_alloc::{
            Page,
            VirtAddr,
        },
        page_table::{
            Mapper,
            MapperError,
            PageTableFlags,
        },
    },
};

use crate::test::{
    test,
    TestResult,
};


/// Test basic physical-frame allocation and deallocation.
#[test]
fn test_frame_allocate_deallocate() -> TestResult {
    let mut kmem = kmem::init();

    let frame = match kmem.frames.allocate() {
        Some(frame) => frame,
        None => {
            return TestResult::Fail(
                "failed to allocate physical frame"
            );
        }
    };

    if kmem.frames.deallocate(frame).is_err() {
        return TestResult::Fail(
            "failed to deallocate physical frame"
        );
    }

    TestResult::Pass
}


/// Test that a freed frame can be allocated again.
#[test]
fn test_frame_reuse() -> TestResult {
    let mut kmem = kmem::init();

    let first = match kmem.frames.allocate() {
        Some(frame) => frame,
        None => {
            return TestResult::Fail(
                "failed to allocate first frame"
            );
        }
    };

    if kmem.frames.deallocate(first).is_err() {
        return TestResult::Fail(
            "failed to deallocate first frame"
        );
    }

    let second = match kmem.frames.allocate() {
        Some(frame) => frame,
        None => {
            return TestResult::Fail(
                "failed to reallocate frame"
            );
        }
    };

    if kmem.frames.deallocate(second).is_err() {
        return TestResult::Fail(
            "failed to deallocate second frame"
        );
    }

    TestResult::Pass
}


/// Test basic page -> frame mapping.
#[test]
fn test_page_map() -> TestResult {
    let mut kmem = kmem::init();

    let mapper = unsafe {
        Mapper::new(
            kmem.pml4_frame,
            kmem.hhdm_offset,
        )
    };

    let frame = match kmem.frames.allocate() {
        Some(frame) => frame,
        None => {
            return TestResult::Fail(
                "failed to allocate frame"
            );
        }
    };

    let page = Page::containing_address(
        VirtAddr::new(
            0xffff_9000_0000_0000
        )
    );

    if mapper
        .map(
            page,
            frame,
            PageTableFlags::WRITABLE
                | PageTableFlags::NO_EXECUTE,
            &mut kmem.frames,
        )
        .is_err()
    {
        let _ =
            kmem.frames.deallocate(frame);

        return TestResult::Fail(
            "failed to map page"
        );
    }

    if mapper.translate_page(page)
        != Some(frame)
    {
        let _ = mapper.unmap(page);
        let _ = kmem.frames.deallocate(frame);

        return TestResult::Fail(
            "page translation returned wrong frame"
        );
    }

    let unmapped =
        match mapper.unmap(page) {
            Ok(result) => result,
            Err(_) => {
                let _ =
                    kmem.frames.deallocate(frame);

                return TestResult::Fail(
                    "failed to unmap page"
                );
            }
        };

    if unmapped.frame != frame {
        let _ =
            kmem.frames.deallocate(frame);

        return TestResult::Fail(
            "unmap returned wrong frame"
        );
    }

    if kmem.frames.deallocate(frame)
        .is_err()
    {
        return TestResult::Fail(
            "failed to return frame"
        );
    }

    TestResult::Pass
}


/// A page must not be mapped twice.
#[test]
fn test_double_map_rejected() -> TestResult {
    let mut kmem = kmem::init();

    let mapper = unsafe {
        Mapper::new(
            kmem.pml4_frame,
            kmem.hhdm_offset,
        )
    };

    let frame_a =
        match kmem.frames.allocate() {
            Some(frame) => frame,
            None => {
                return TestResult::Fail(
                    "failed to allocate frame A"
                );
            }
        };

    let frame_b =
        match kmem.frames.allocate() {
            Some(frame) => frame,
            None => {
                let _ =
                    kmem.frames.deallocate(
                        frame_a
                    );

                return TestResult::Fail(
                    "failed to allocate frame B"
                );
            }
        };

    let page = Page::containing_address(
        VirtAddr::new(
            0xffff_9000_0000_0000
        )
    );

    if mapper
        .map(
            page,
            frame_a,
            PageTableFlags::WRITABLE,
            &mut kmem.frames,
        )
        .is_err()
    {
        let _ =
            kmem.frames.deallocate(frame_a);

        let _ =
            kmem.frames.deallocate(frame_b);

        return TestResult::Fail(
            "failed to map first frame"
        );
    }

    let result = mapper.map(
        page,
        frame_b,
        PageTableFlags::WRITABLE,
        &mut kmem.frames,
    );

    if result
        != Err(MapperError::PageAlreadyMapped)
    {
        let _ = mapper.unmap(page);
        let _ =
            kmem.frames.deallocate(frame_a);
        let _ =
            kmem.frames.deallocate(frame_b);

        return TestResult::Fail(
            "double mapping was accepted"
        );
    }

    let unmapped =
        match mapper.unmap(page) {
            Ok(result) => result,
            Err(_) => {
                let _ =
                    kmem.frames.deallocate(
                        frame_a
                    );
                let _ =
                    kmem.frames.deallocate(
                        frame_b
                    );

                return TestResult::Fail(
                    "failed to unmap page"
                );
            }
        };

    if unmapped.frame != frame_a {
        let _ =
            kmem.frames.deallocate(frame_a);
        let _ =
            kmem.frames.deallocate(frame_b);

        return TestResult::Fail(
            "unmap returned wrong frame"
        );
    }

    if kmem.frames.deallocate(frame_a)
        .is_err()
    {
        return TestResult::Fail(
            "failed to free frame A"
        );
    }

    if kmem.frames.deallocate(frame_b)
        .is_err()
    {
        return TestResult::Fail(
            "failed to free frame B"
        );
    }

    TestResult::Pass
}


/// An unmapped page must not translate.
#[test]
fn test_unmapped_translation() -> TestResult {
    let kmem = kmem::init();

    let mapper = unsafe {
        Mapper::new(
            kmem.pml4_frame,
            kmem.hhdm_offset,
        )
    };

    let page = Page::containing_address(
        VirtAddr::new(
            0xffff_9000_0000_0000
        )
    );

    if mapper.translate_page(page)
        .is_some()
    {
        return TestResult::Fail(
            "unmapped page translated"
        );
    }

    TestResult::Pass
}


/// Test virtual-address translation including
/// the offset inside the page.
#[test]
fn test_address_translation() -> TestResult {
    let mut kmem = kmem::init();

    let mapper = unsafe {
        Mapper::new(
            kmem.pml4_frame,
            kmem.hhdm_offset,
        )
    };

    let frame = match kmem.frames.allocate() {
        Some(frame) => frame,
        None => {
            return TestResult::Fail(
                "failed to allocate frame"
            );
        }
    };

    let page = Page::containing_address(
        VirtAddr::new(
            0xffff_9000_0000_0000
        )
    );

    if mapper
        .map(
            page,
            frame,
            PageTableFlags::WRITABLE,
            &mut kmem.frames,
        )
        .is_err()
    {
        let _ =
            kmem.frames.deallocate(frame);

        return TestResult::Fail(
            "failed to map page"
        );
    }

    let offset = 1234u64;

    let virtual_address =
        VirtAddr::new(
            page.start_address().as_u64()
                + offset
        );

    let expected =
        frame.start_address().as_u64()
            + offset;

    let actual =
        match mapper.translate(
            virtual_address
        ) {
            Some(address) =>
                address.as_u64(),

            None => {
                let _ =
                    mapper.unmap(page);

                let _ =
                    kmem.frames
                        .deallocate(frame);

                return TestResult::Fail(
                    "address failed to translate"
                );
            }
        };

    if actual != expected {
        let _ =
            mapper.unmap(page);

        let _ =
            kmem.frames.deallocate(frame);

        return TestResult::Fail(
            "translated address is incorrect"
        );
    }

    let unmapped =
        match mapper.unmap(page) {
            Ok(result) => result,
            Err(_) => {
                let _ =
                    kmem.frames.deallocate(frame);

                return TestResult::Fail(
                    "failed to unmap page"
                );
            }
        };

    if kmem.frames
        .deallocate(unmapped.frame)
        .is_err()
    {
        return TestResult::Fail(
            "failed to free frame"
        );
    }

    TestResult::Pass
}


/// Map several pages to different physical frames.
#[test]
fn test_multiple_page_mappings() -> TestResult {
    let mut kmem = kmem::init();

    let mapper = unsafe {
        Mapper::new(
            kmem.pml4_frame,
            kmem.hhdm_offset,
        )
    };

    const COUNT: usize = 8;

    let base =
        0xffff_9000_0000_0000u64;

    let mut pages = [None; COUNT];
    let mut frames = [None; COUNT];

    for i in 0..COUNT {
        let frame =
            match kmem.frames.allocate() {
                Some(frame) => frame,
                None => {
                    cleanup(
                        &mut kmem,
                        &mapper,
                        &pages,
                        &frames,
                    );

                    return TestResult::Fail(
                        "failed to allocate frame"
                    );
                }
            };

        let page =
            Page::containing_address(
                VirtAddr::new(
                    base + (i as u64 * 0x1000)
                )
            );

        if mapper
            .map(
                page,
                frame,
                PageTableFlags::WRITABLE
                    | PageTableFlags::NO_EXECUTE,
                &mut kmem.frames,
            )
            .is_err()
        {
            let _ =
                kmem.frames.deallocate(frame);

            cleanup(
                &mut kmem,
                &mapper,
                &pages,
                &frames,
            );

            return TestResult::Fail(
                "failed to map page"
            );
        }

        pages[i] = Some(page);
        frames[i] = Some(frame);
    }

    for i in 0..COUNT {
        if mapper.translate_page(
            pages[i].unwrap()
        ) != frames[i]
        {
            cleanup(
                &mut kmem,
                &mapper,
                &pages,
                &frames,
            );

            return TestResult::Fail(
                "page translated to wrong frame"
            );
        }
    }

    cleanup(
        &mut kmem,
        &mapper,
        &pages,
        &frames,
    );

    TestResult::Pass
}


/// Verify the frame allocator's internal bookkeeping.
#[test]
fn test_frame_allocator_integrity() -> TestResult {
    let kmem = kmem::init();

    kmem.frames.verify();

    TestResult::Pass
}


fn cleanup(
    kmem: &mut kmem::KernelMemory,
    mapper: &Mapper,
    pages: &[Option<Page>],
    frames: &[Option<
        crate::memory::frame_alloc::PhysFrame
    >],
) {
    for i in 0..pages.len() {
        if let Some(page) = pages[i] {
            if let Ok(result) =
                mapper.unmap(page)
            {
                let _ =
                    kmem.frames.deallocate(
                        result.frame
                    );
            }
        } else if let Some(frame) = frames[i] {
            let _ =
                kmem.frames.deallocate(frame);
        }
    }
}
