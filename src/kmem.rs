use limine::request::{MemmapRequest, HhdmRequest};

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
