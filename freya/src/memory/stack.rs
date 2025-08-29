use crate::{
    arch::memory::PAGE_SIZE,
    memory::{self, KERNEL_PAGE_SPACE},
};

pub struct KernelStack {
    base: u64,
    top_address: u64,
    size: usize,
}

impl KernelStack {
    const SIZE: usize = 0x4000;

    pub fn new() -> Self {
        let virtual_address =
            memory::heap::allocate_virtual_memory(Self::SIZE).expect("Out of virtual memory");

        // Start at `PAGE_SIZE` bytes to account for the stack guard page
        let mut cursor = KERNEL_PAGE_SPACE.cursor(virtual_address + PAGE_SIZE as u64);

        for _ in (PAGE_SIZE..Self::SIZE).step_by(PAGE_SIZE) {
            let physical_page = memory::page::allocate(PAGE_SIZE).expect("Out of physical memory");

            cursor.map_page(
                physical_page,
                memory::PageAccess::READ | memory::PageAccess::WRITE,
                memory::CachingMode::Null,
            );
            cursor.advance_page();
        }

        Self {
            base: virtual_address,
            top_address: virtual_address + Self::SIZE as u64,
            size: Self::SIZE,
        }
    }

    pub fn top(&self) -> u64 {
        self.top_address
    }
}

impl Drop for KernelStack {
    fn drop(&mut self) {
        let mut cursor = KERNEL_PAGE_SPACE.cursor(self.base);

        for _ in (PAGE_SIZE..Self::SIZE).step_by(PAGE_SIZE) {
            let (physical_page, _) = cursor.unmap_page().expect("Failed to unmap page");

            memory::page::free(physical_page, PAGE_SIZE);

            cursor.advance_page();
        }

        memory::heap::free_virtual_memory(self.base, self.size);
    }
}
