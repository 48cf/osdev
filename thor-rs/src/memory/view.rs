use core::num::NonZeroU64;

use alloc::{boxed::Box, sync::Arc, vec::Vec};
use async_trait::async_trait;
use hashbrown::HashMap;

use crate::{
    KernelError, KernelResult,
    arch::memory::PAGE_SIZE,
    memory::{self, CachingMode, PageAccess, accessor::PageAccessor},
};

#[derive(Clone)]
pub struct MemorySlice {
    view: Arc<dyn MemoryView>,
    offset: usize,
    length: usize,
    caching_mode: CachingMode,
}

impl MemorySlice {
    pub fn new(
        view: Arc<dyn MemoryView>,
        offset: usize,
        length: usize,
        caching_mode: CachingMode,
    ) -> Arc<Self> {
        Arc::new(Self {
            view,
            offset,
            length,
            caching_mode,
        })
    }

    pub fn view(&self) -> &Arc<dyn MemoryView> {
        &self.view
    }

    pub fn offset(&self) -> usize {
        self.offset
    }

    pub fn length(&self) -> usize {
        self.length
    }

    pub fn caching_mode(&self) -> CachingMode {
        self.caching_mode
    }
}

#[async_trait]
pub trait MemoryView: Sync + Send {
    fn base(&self) -> &MemoryViewBase;

    async fn fault_in(&self, offset: usize, access: PageAccess) -> KernelResult<()>;

    async fn copy_to(&self, offset: usize, buffer: &[u8]) -> KernelResult<()> {
        let mut progress = 0;

        while progress < buffer.len() {
            let offset = offset + progress;

            self.fault_in(offset, PageAccess::WRITE).await?;

            let (physical_address, _, kind) = self
                .base()
                .contents()
                .await
                .get(&(offset / PAGE_SIZE))
                .copied()
                .ok_or(KernelError::Fault)?;

            assert!(kind.is_compatible(PageAccess::WRITE));

            let accessor = PageAccessor::new(physical_address);
            let page_offset = offset & (PAGE_SIZE - 1);
            let chunk_size = (PAGE_SIZE - page_offset).min(buffer.len() - progress);

            unsafe {
                accessor
                    .as_mut::<u8>()
                    .add(page_offset)
                    .copy_from_nonoverlapping(buffer.as_ptr().add(progress), chunk_size);
            }

            progress += chunk_size;
        }

        Ok(())
    }

    async fn copy_from(&self, offset: usize, buffer: &mut [u8]) -> KernelResult<()> {
        let mut progress = 0;

        while progress < buffer.len() {
            let offset = offset + progress;

            self.fault_in(offset, PageAccess::READ).await?;

            let (physical_address, _, _) = self
                .base()
                .contents()
                .await
                .get(&(offset / PAGE_SIZE))
                .copied()
                .ok_or(KernelError::Fault)?;

            let accessor = PageAccessor::new(physical_address);
            let page_offset = offset & (PAGE_SIZE - 1);
            let chunk_size = (PAGE_SIZE - page_offset).min(buffer.len() - progress);

            unsafe {
                accessor
                    .as_mut::<u8>()
                    .add(page_offset)
                    .copy_to_nonoverlapping(buffer.as_mut_ptr().add(progress), chunk_size);
            }

            progress += chunk_size;
        }

        Ok(())
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MemoryViewPageKind {
    /// Page is borrowed from another view, it cannot be faulted in for write operations.
    Borrowed,
    /// Page is owned by this view, it can be faulted in for write operations.
    Owned,
}

impl MemoryViewPageKind {
    pub fn is_compatible(self, access: PageAccess) -> bool {
        // If the access includes write, the page must be owned.
        // In any other case it can be either owned or borrowed.
        if access.contains(PageAccess::WRITE) {
            self == MemoryViewPageKind::Owned
        } else {
            true
        }
    }
}

type MemoryViewContents = HashMap<usize, (u64, CachingMode, MemoryViewPageKind)>;

pub struct MemoryViewBase {
    contents: async_lock::RwLock<MemoryViewContents>,
}

impl MemoryViewBase {
    pub fn new() -> Self {
        Self {
            contents: async_lock::RwLock::new(HashMap::new()),
        }
    }

    pub async fn contents(&self) -> async_lock::RwLockReadGuard<'_, MemoryViewContents> {
        self.contents.read().await
    }

    pub async fn contents_mut(&self) -> async_lock::RwLockWriteGuard<'_, MemoryViewContents> {
        self.contents.write().await
    }
}

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

pub struct AllocatedMemory {
    base: MemoryViewBase,
    chunk_count: usize,
    chunk_size: usize,
}

impl AllocatedMemory {
    fn new_with_params(desired_length: usize, desired_chunk_size: usize) -> Arc<Self> {
        let chunk_size = 1 << (64 - (desired_chunk_size - 1).leading_zeros());
        let length = desired_length.next_multiple_of(chunk_size);

        Arc::new(Self {
            base: MemoryViewBase::new(),
            chunk_count: length / chunk_size,
            chunk_size,
        })
    }

    pub fn new(length: usize) -> Arc<Self> {
        Self::new_with_params(length, PAGE_SIZE)
    }

    pub fn new_contiguous(length: usize) -> Arc<Self> {
        Self::new_with_params(length, length)
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
                memory::page::allocate(self.chunk_size).ok_or(KernelError::NoMemory)?;

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
