use alloc::{boxed::Box, sync::Arc};
use async_trait::async_trait;
use hashbrown::HashMap;

use crate::{
    Error, Result,
    arch::memory::PAGE_SIZE,
    memory::{self, CachingMode, accessor::PageAccessor},
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

    async fn fault_in(&self, offset: usize) -> Result<()>;

    async fn copy_to(&self, offset: usize, buffer: &[u8]) -> Result<()> {
        let mut progress = 0;

        while progress < buffer.len() {
            let offset = offset + progress;

            self.fault_in(offset).await?;

            let (physical_address, _) = self
                .base()
                .contents()
                .await
                .get(&(offset / PAGE_SIZE))
                .copied()
                .ok_or(Error::Fault)?;

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

    async fn copy_from(&self, offset: usize, buffer: &mut [u8]) -> Result<()> {
        let mut progress = 0;

        while progress < buffer.len() {
            let offset = offset + progress;

            self.fault_in(offset).await?;

            let (physical_address, _) = self
                .base()
                .contents()
                .await
                .get(&(offset / PAGE_SIZE))
                .copied()
                .ok_or(Error::Fault)?;

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

type MemoryViewContents = HashMap<usize, (u64, CachingMode)>;

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
    pub fn new(length: usize) -> Self {
        let page_count = (length + PAGE_SIZE - 1) / PAGE_SIZE;

        let mut base = MemoryViewBase::new();

        for i in 0..page_count {
            let physical_page = memory::page::allocate(PAGE_SIZE).expect("Out of physical memory");
            let accessor = PageAccessor::new(physical_page);

            unsafe {
                accessor.as_mut::<u8>().write_bytes(0, PAGE_SIZE);
            }

            base.contents
                .get_mut()
                .insert(i, (physical_page, CachingMode::Null));
        }

        Self { base, page_count }
    }
}

#[async_trait]
impl MemoryView for ImmediateMemory {
    fn base(&self) -> &MemoryViewBase {
        &self.base
    }

    async fn fault_in(&self, offset: usize) -> Result<()> {
        // Fault in is a no-op for immediate memory.
        if offset / PAGE_SIZE >= self.page_count {
            Err(Error::Fault)
        } else {
            Ok(())
        }
    }
}
