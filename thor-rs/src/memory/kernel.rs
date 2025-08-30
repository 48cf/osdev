use spin::Lazy;

use crate::{
    arch::memory::{KernelCursorPolicy, PAGE_SIZE},
    memory::{cursor::PageCursor, space::PageSpace},
};

pub static KERNEL_PAGE_SPACE: Lazy<KernelPageSpace> = Lazy::new(|| KernelPageSpace::new());

pub struct KernelPageSpace {
    space: PageSpace,
}

unsafe impl Send for KernelPageSpace {}
unsafe impl Sync for KernelPageSpace {}

impl KernelPageSpace {
    pub fn new() -> Self {
        let mut cr3: u64;

        unsafe {
            core::arch::asm!("mov {}, cr3", out(reg) cr3);
        }

        Self {
            space: unsafe { PageSpace::new_from_root_physical(cr3 & !(PAGE_SIZE as u64 - 1)) },
        }
    }

    pub fn cursor(&self, address: u64) -> PageCursor<'_, KernelCursorPolicy> {
        PageCursor::new(&self.space, address)
    }

    pub fn space(&self) -> &PageSpace {
        &self.space
    }
}
