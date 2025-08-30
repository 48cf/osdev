use crate::memory::stack::KernelStack;

#[repr(C, packed)]
pub struct Tss {
    reserved0: u32,
    rsp: [u64; 3],
    reserved1: u32,
    reserved2: u32,
    ist: [u64; 7],
    reserved3: u32,
    reserved4: u32,
    reserved5: u16,
    io_map_offset: u16,
}

impl Tss {
    pub fn new(kernel_stack: &KernelStack) -> Self {
        Self {
            reserved0: 0,
            rsp: [kernel_stack.top(), 0, 0],
            reserved1: 0,
            reserved2: 0,
            ist: [0; 7],
            reserved3: 0,
            reserved4: 0,
            reserved5: 0,
            io_map_offset: 0,
        }
    }
}
