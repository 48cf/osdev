use alloc::boxed::Box;
use async_trait::async_trait;
use hashbrown::HashMap;

use crate::{
    Error, Result,
    arch::memory::PAGE_SIZE,
    memory::{self, CachingMode, accessor::PageAccessor},
};

#[async_trait]
pub trait MemoryView: Sync + Send {
    fn base(&self) -> &MemoryViewBase;

    async fn fault_in(&self, offset: usize) -> Result<()>;

    // fn peek_range(&self, offset: usize) -> Option<PhysicalRange>;

    // async fn lock_range(&self, offset: usize, length: usize);
    // async fn unlock_range(&self, offset: usize, length: usize);
    // async fn fetch_range(&self, offset: usize) -> Result<PhysicalRange>;

    async fn copy_to(&self, offset: usize, buffer: &[u8]) -> Result<()>;
    async fn copy_from(&self, offset: usize, buffer: &mut [u8]) -> Result<()>;
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

    pub async fn lock(&self) -> async_lock::RwLockReadGuard<'_, MemoryViewContents> {
        self.contents.read().await
    }

    pub async fn lock_write(&self) -> async_lock::RwLockWriteGuard<'_, MemoryViewContents> {
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

    // fn peek_range(&self, offset: usize) -> Option<PhysicalRange> {
    //     let pages = self.pages.lock();
    //     let page_index = offset / PAGE_SIZE;
    //     let page_offset = offset % PAGE_SIZE;

    //     pages.get(page_index).map(|address| {
    //         let address = address + page_offset as u64;
    //         let remaining_length = PAGE_SIZE - page_offset;

    //         (address, remaining_length, CachingMode::Null)
    //     })
    // }

    // async fn lock_range(&self, _offset: usize, _length: usize) {
    //     // TODO
    // }

    // async fn unlock_range(&self, _offset: usize, _length: usize) {
    //     // TODO
    // }

    // async fn fetch_range(&self, offset: usize) -> Result<PhysicalRange> {
    //     self.peek_range(offset).ok_or(Error::Fault)
    // }

    async fn copy_to(&self, offset: usize, buffer: &[u8]) -> Result<()> {
        let mut progress = 0;

        while progress < buffer.len() {
            self.fault_in(offset + progress).await?;

            let (physical_address, _) = self
                .base()
                .lock()
                .await
                .get(&((offset + progress) / PAGE_SIZE))
                .copied()
                .ok_or(Error::Fault)?;

            let accessor = PageAccessor::new(physical_address & !(PAGE_SIZE as u64 - 1));
            let chunk_size = PAGE_SIZE.min(buffer.len() - progress);

            unsafe {
                accessor
                    .as_mut::<u8>()
                    .add(physical_address as usize & (PAGE_SIZE - 1))
                    .copy_from_nonoverlapping(buffer.as_ptr().add(progress), chunk_size);
            }

            progress += chunk_size;
        }

        Ok(())
    }

    async fn copy_from(&self, _offset: usize, _buffer: &mut [u8]) -> Result<()> {
        todo!()
    }
}
