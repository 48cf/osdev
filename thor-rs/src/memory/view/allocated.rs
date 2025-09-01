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

pub struct AllocatedMemory {
    base: MemoryViewBase,
    chunk_count: usize,
    chunk_size: usize,
    address_bits: usize,
}

impl AllocatedMemory {
    fn new_with_params(
        desired_length: usize,
        desired_chunk_size: usize,
        address_bits: usize,
    ) -> Arc<Self> {
        let chunk_size = 1 << (64 - (desired_chunk_size - 1).leading_zeros());
        let length = desired_length.next_multiple_of(chunk_size);

        Arc::new(Self {
            base: MemoryViewBase::new(),
            chunk_count: length / chunk_size,
            chunk_size,
            address_bits,
        })
    }

    pub fn new(length: usize, address_bits: usize) -> Arc<Self> {
        Self::new_with_params(length, PAGE_SIZE, address_bits)
    }

    pub fn new_contiguous(length: usize, address_bits: usize) -> Arc<Self> {
        Self::new_with_params(length, length, address_bits)
    }
}

#[async_trait]
impl MemoryView for AllocatedMemory {
    fn base(&self) -> &MemoryViewBase {
        &self.base
    }

    async fn fault_in(&self, offset: usize, _access: PageAccess) -> KernelResult<()> {
        let chunk_index = offset / self.chunk_size;
        let chunk_page_index = (chunk_index * self.chunk_size) / PAGE_SIZE;

        if chunk_index >= self.chunk_count {
            return Err(KernelError::Fault);
        }

        let mut contents = self.base.contents_mut().await;

        if !contents.contains_key(&chunk_page_index) {
            let physical_page =
                memory::page::allocate_restricted(self.chunk_size, self.address_bits)
                    .ok_or(KernelError::NoMemory)?;

            let accessor = PageAccessor::new(physical_page);

            unsafe {
                accessor.as_mut::<u8>().write_bytes(0, self.chunk_size);
            }

            // Populate entries for all pages in a chunk.
            for i in 0..(self.chunk_size / PAGE_SIZE) {
                assert!(!contents.contains_key(&(chunk_page_index + i)));

                contents.insert(
                    chunk_page_index + i,
                    (
                        physical_page + (i * PAGE_SIZE) as u64,
                        CachingMode::Null,
                        MemoryViewPageKind::Owned,
                    ),
                );
            }
        }

        Ok(())
    }
}
