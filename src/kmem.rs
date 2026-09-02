use limine::request::{MemmapRequest, HhdmRequest};
use x86_64::{PhysAddr, VirtAddr, registers::control::Cr3, structures::paging::{OffsetPageTable, PageTable}};

use crate::{console_println, serial_println};

trait MemoryAllocator{
    fn kmallloc(){}
    fn kdellloc(){}
}

#[used]
#[unsafe(link_section = ".limine_reqs")]
static HHDM_REQUEST: HhdmRequest = HhdmRequest::new();

#[used]
#[unsafe(link_section = ".limine_reqs")]
static MEMORY_MAP_REQUEST: MemmapRequest = MemmapRequest::new();

#[inline]
pub fn hhdm_offset() -> u64 {
    return HHDM_REQUEST.response().expect("HHDM unavaliable").offset;
}

#[inline]
pub fn phys_to_virt(phys: u64) -> u64 {
    return phys + hhdm_offset();

}

pub fn init(physical_mem_offset: VirtAddr) -> OffsetPageTable<'static>{
    unsafe {
        let l4_table = active_level_4_table(physical_mem_offset);
        return OffsetPageTable::new(l4_table, physical_mem_offset);
    }
}

unsafe fn active_level_4_table(physical_mem_offset: VirtAddr) -> &'static mut PageTable {
    use x86_64::registers::control::Cr3;
    let (level_4_table_frame, _) = Cr3::read();
    let phys = level_4_table_frame.start_address();
    let virt = physical_mem_offset + phys.as_u64();
    let page_table_ptr: *mut PageTable = virt.as_mut_ptr();
    unsafe { return  &mut *page_table_ptr; }
}

pub unsafe fn translate_addr(addr: VirtAddr, physical_mem_offset: VirtAddr) -> Option<PhysAddr>{
    return translate_addr_inner(addr, physical_mem_offset);
}

fn translate_addr_inner(addr: VirtAddr, physical_mem_offset: VirtAddr) -> Option<PhysAddr>{
    use x86_64::structures::paging::page_table::FrameError;
    use x86_64::registers::control::Cr3;

    let (level_4_table_frame, _) = Cr3::read();
    let table_indexes = [addr.p4_index(), addr.p3_index(), addr.p2_index(), addr.p1_index()];
    let mut frame = level_4_table_frame;

    for &index in &table_indexes {
        let virt = physical_mem_offset + frame.start_address().as_u64();
        let table_ptr: *const PageTable = virt.as_ptr();
        let table = unsafe { &*table_ptr };

        let entry = &table[index];
        frame = match entry.frame(){
            Ok(frame) => frame,
            Err(FrameError::FrameNotPresent) => return None,
            Err(FrameError::HugeFrame) => panic!("[FATAL] Huge pages are not supported yet.")
        };
    }

    return Some(frame.start_address() + u64::from(addr.page_offset()));
}
