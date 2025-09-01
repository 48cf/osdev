use alloc::{boxed::Box, sync::Arc};
use async_trait::async_trait;

use crate::{
    KernelError, KernelResult,
    arch::memory::PAGE_SIZE,
    memory::{
        self, CachingMode, PageAccess,
        accessor::PageAccessor,
        view::{MemoryView, MemoryViewBase, MemoryViewPageKind},
    },
};

pub struct ImmediateMemory {
    base: MemoryViewBase,
    page_count: usize,
}

impl ImmediateMemory {
    pub fn new(length: usize) -> Arc<Self> {
        let page_count = (length + PAGE_SIZE - 1) / PAGE_SIZE;

        let mut base = MemoryViewBase::new();

        for i in 0..page_count {
            let physical_page = memory::page::allocate(PAGE_SIZE).expect("Out of physical memory");
            let accessor = PageAccessor::new(physical_page);

            unsafe {
                accessor.as_mut::<u8>().write_bytes(0, PAGE_SIZE);
            }

            base.contents.get_mut().insert(
                i,
                (physical_page, CachingMode::Null, MemoryViewPageKind::Owned),
            );
        }

        Arc::new(Self { base, page_count })
    }
}

#[async_trait]
impl MemoryView for ImmediateMemory {
    fn base(&self) -> &MemoryViewBase {
        &self.base
    }

    async fn fault_in(&self, offset: usize, _access: PageAccess) -> KernelResult<()> {
        // Fault in is a no-op for immediate memory.
        if offset / PAGE_SIZE >= self.page_count {
            Err(KernelError::Fault)
        } else {
            Ok(())
        }
    }
}
