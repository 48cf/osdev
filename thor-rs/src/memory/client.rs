use alloc::{boxed::Box, sync::Arc};
use async_trait::async_trait;

use crate::{
    arch::memory::UserCursorPolicy,
    memory::{
        self,
        cursor::PageCursor,
        space::{PageSpace, VirtualSpace},
        view::MemoryView,
    },
};

pub struct ClientPageSpace {
    space: PageSpace,
}

impl ClientPageSpace {
    pub fn new() -> Arc<Self> {
        let space = PageSpace::new_user();

        Arc::new(Self { space })
    }

    pub fn cursor(&self, address: u64) -> PageCursor<'_, UserCursorPolicy> {
        PageCursor::new(&self.space, address)
    }

    pub fn space(&self) -> &PageSpace {
        &self.space
    }
}

#[async_trait]
impl VirtualSpace for ClientPageSpace {
    async fn map_present_pages(
        &self,
        memory_view: &Arc<impl MemoryView>,
        virtual_address: u64,
        offset: usize,
        length: usize,
        access: super::PageAccess,
        caching: super::CachingMode,
    ) {
        memory::space::map_present_pages_with_cursor::<UserCursorPolicy>(
            &self.space,
            memory_view,
            virtual_address,
            offset,
            length,
            access,
            caching,
        )
        .await;
    }

    async fn fault_page(
        &self,
        _memory_view: &Arc<impl super::view::MemoryView>,
        _virtual_address: u64,
        _offset: usize,
        _access: super::PageAccess,
        _caching: super::CachingMode,
    ) {
        todo!()
    }
}
