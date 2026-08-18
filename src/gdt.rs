use spin::Once;

use x86_64::{
    VirtAddr,
    instructions::{
        segmentation::{CS, SS, Segment},
        tables::load_tss,
    },
    structures::{
        gdt::{
            Descriptor,
            GlobalDescriptorTable,
            SegmentSelector,
        },
        tss::TaskStateSegment,
    },
};

pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;

const DOUBLE_FAULT_STACK_SIZE: usize = 4096 * 5;

#[repr(align(16))]
struct Stack([u8; DOUBLE_FAULT_STACK_SIZE]);

static mut DOUBLE_FAULT_STACK: Stack =
    Stack([0; DOUBLE_FAULT_STACK_SIZE]);

static TSS: Once<TaskStateSegment> = Once::new();

static GDT: Once<(GlobalDescriptorTable, Selectors)> =
    Once::new();

#[derive(Clone, Copy)]
struct Selectors {
    code_selector: SegmentSelector,
    data_selector: SegmentSelector,
    tss_selector: SegmentSelector,
}

pub fn init() {
    // ========================================================
    // TSS
    // ========================================================

    let tss = TSS.call_once(|| {
        let mut tss = TaskStateSegment::new();

        let stack_start = VirtAddr::from_ptr(
            unsafe { core::ptr::addr_of!(DOUBLE_FAULT_STACK.0) }
        );

        let stack_end =
            stack_start + DOUBLE_FAULT_STACK_SIZE as u64;

        tss.interrupt_stack_table[
            DOUBLE_FAULT_IST_INDEX as usize
        ] = stack_end;

        tss
    });

    // ========================================================
    // GDT
    // ========================================================

    let (gdt, selectors) = GDT.call_once(|| {
        let mut gdt = GlobalDescriptorTable::new();

        // GDT entry 0:
        // null

        // GDT entry 1:
        // kernel code
        let code_selector =
            gdt.append(
                Descriptor::kernel_code_segment()
            );

        // GDT entry 2:
        // kernel data
        let data_selector =
            gdt.append(
                Descriptor::kernel_data_segment()
            );

        // GDT entries 3 + 4:
        // TSS
        let tss_selector =
            gdt.append(
                Descriptor::tss_segment(tss)
            );

        (
            gdt,
            Selectors {
                code_selector,
                data_selector,
                tss_selector,
            },
        )
    });

    // ========================================================
    // Load GDT
    // ========================================================

    gdt.load();

    // ========================================================
    // Reload segment registers
    // ========================================================

    unsafe {
        // CS = GDT entry 1
        CS::set_reg(selectors.code_selector);

        // SS = GDT entry 2
        SS::set_reg(selectors.data_selector);

        // Load TSS
        load_tss(selectors.tss_selector);
    }
}
