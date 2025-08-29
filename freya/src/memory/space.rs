use alloc::sync::Arc;

use crate::{
    arch::memory::{KernelCursorPolicy, PAGE_SIZE, UserCursorPolicy},
    memory::{CachingMode, PageAccess, cursor::Cursor},
};

struct PageSpaceInner;

pub struct PageSpace {
    root_table: u64,
    inner: (), // SpinMutex<PageSpaceInner>,
}

impl PageSpace {
    pub fn root_table(&self) -> u64 {
        self.root_table
    }
}

pub struct KernelPageSpace {
    space: PageSpace,
}

unsafe impl Send for KernelPageSpace {}
unsafe impl Sync for KernelPageSpace {}

pub struct UserPageSpace {
    space: Arc<PageSpace>,
}

impl KernelPageSpace {
    pub fn new() -> Self {
        let mut cr3: u64;

        unsafe {
            core::arch::asm!("mov {}, cr3", out(reg) cr3);
        }

        Self {
            space: PageSpace {
                root_table: cr3 & !(PAGE_SIZE as u64 - 1),
                inner: (),
            },
        }
    }

    pub fn cursor(&self, address: u64) -> Cursor<'_, KernelCursorPolicy> {
        Cursor::new(&self.space, address)
    }

    fn map_single_page(
        &mut self,
        virtual_address: u64,
        physical_address: u64,
        access: PageAccess,
        caching: CachingMode,
    ) {
        assert!(virtual_address & (PAGE_SIZE as u64 - 1) == 0);
        assert!(physical_address & (PAGE_SIZE as u64 - 1) == 0);

        let mut cursor = self.cursor(virtual_address);

        cursor.map_page(physical_address, access, caching);
    }

    fn unmap_single_page(&mut self, virtual_address: u64) -> Option<u64> {
        let cursor = self.cursor(virtual_address);

        cursor.unmap_page().map(|(address, _status)| address)
    }
}

impl UserPageSpace {
    pub fn cursor(&self, address: u64) -> Cursor<'_, UserCursorPolicy> {
        Cursor::new(&self.space, address)
    }
}
