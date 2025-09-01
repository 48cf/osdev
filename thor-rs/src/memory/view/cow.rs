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

pub struct CopyOnWriteMemory {
    base: MemoryViewBase,
    underlying_view: Arc<dyn MemoryView>,
    offset: usize,
    length: usize,
}

impl CopyOnWriteMemory {
    pub fn new(underlying_view: Arc<dyn MemoryView>, offset: usize, length: usize) -> Arc<Self> {
        Arc::new(Self {
            base: MemoryViewBase::new_with_eviction_queue(),
            underlying_view,
            offset,
            length,
        })
    }
}

#[async_trait]
impl MemoryView for CopyOnWriteMemory {
    fn base(&self) -> &MemoryViewBase {
        &self.base
    }

    async fn fault_in(&self, offset: usize, _access: PageAccess) -> KernelResult<()> {
        let page_index = offset / PAGE_SIZE;

        if page_index * PAGE_SIZE >= self.length {
            return Err(KernelError::Fault);
        }

        let mut contents = self.base.contents_mut().await;

        if !contents.contains_key(&page_index) {
            let physical_page = memory::page::allocate(PAGE_SIZE).ok_or(KernelError::NoMemory)?;
            let accessor = PageAccessor::new(physical_page);
            let slice =
                unsafe { core::slice::from_raw_parts_mut(accessor.as_mut::<u8>(), PAGE_SIZE) };

            self.underlying_view
                .copy_from(self.offset + page_index * PAGE_SIZE, slice)
                .await?;

            contents.insert(
                page_index,
                (physical_page, CachingMode::Null, MemoryViewPageKind::Owned),
            );
        }

        Ok(())
    }
}
