type EirPtr = u64;
type EirSize = u64;

#[repr(C)]
#[derive(Debug)]
pub struct EirRegion {
    pub address: EirPtr,
    pub length: EirSize,
    pub order: EirSize,
    pub num_roots: EirSize,
    pub buddy_tree: EirPtr,
}

#[repr(C)]
pub struct EirModule {
    pub physical_base: EirPtr,
    pub length: EirSize,
    pub name_ptr: EirPtr,
    pub name_length: EirSize,
}

#[repr(C)]
#[derive(Debug)]
pub struct EirFramebuffer {
    pub address: EirPtr,
    pub early_window: EirPtr,
    pub pitch: EirSize,
    pub width: EirSize,
    pub height: EirSize,
    pub bpp: EirSize,
    pub kind: EirSize,
}

#[repr(C)]
#[derive(Debug)]
pub struct EirInfo {
    pub signature: u64,
    pub command_line: EirPtr,
    pub debug_flags: u32,
    pub padding: u32,

    pub hart_id: u64,

    pub num_regions: EirSize,
    pub region_info: EirPtr,
    pub module_info: EirPtr,

    pub dtb_ptr: EirPtr,
    pub dtb_size: EirSize,

    pub framebuffer: EirFramebuffer,

    pub acpi_rsdp: u64,
}

impl EirInfo {
    pub fn regions(&self) -> &[EirRegion] {
        unsafe {
            core::slice::from_raw_parts(
                self.region_info as *const EirRegion,
                self.num_regions as usize,
            )
        }
    }
}
