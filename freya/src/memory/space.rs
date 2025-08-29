use alloc::sync::Arc;
use spin::Mutex;

use crate::{
    arch::memory::{KernelCursorPolicy, PAGE_SIZE, UserCursorPolicy},
    memory::cursor::Cursor,
};

struct PageSpaceInner;

pub struct PageSpace {
    root_table: u64,
    inner: Mutex<PageSpaceInner>,
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
                inner: Mutex::new(PageSpaceInner),
            },
        }
    }

    pub fn cursor(&self, address: u64) -> Cursor<'_, KernelCursorPolicy> {
        Cursor::new(&self.space, address)
    }
}

impl UserPageSpace {
    pub fn cursor(&self, address: u64) -> Cursor<'_, UserCursorPolicy> {
        Cursor::new(&self.space, address)
    }
}
