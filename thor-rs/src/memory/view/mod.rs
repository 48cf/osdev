mod allocated;
mod cow;
mod immediate;
mod zero;

use core::sync::atomic::{AtomicUsize, Ordering};

use alloc::{boxed::Box, collections::btree_map::BTreeMap, sync::Arc};
use async_channel::Sender;
use async_lock::Barrier;
use async_trait::async_trait;
use hashbrown::HashMap;
use spin::Lazy;

pub use allocated::AllocatedMemory;
pub use cow::CopyOnWriteMemory;
pub use immediate::ImmediateMemory;
pub use zero::ZeroMemory;

use crate::{
    KernelError, KernelResult,
    arch::memory::PAGE_SIZE,
    memory::{CachingMode, PageAccess, accessor::PageAccessor},
};

pub static ZERO_MEMORY: Lazy<Arc<ZeroMemory>> = Lazy::new(|| ZeroMemory::new());

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

#[derive(Debug)]
pub struct Eviction {
    offset: usize,
    length: usize,
}

type EvictionObserver = Sender<(Eviction, Arc<Barrier>)>;

struct EvictionQueue {
    next_handle: AtomicUsize,
    observers: async_lock::Mutex<BTreeMap<usize, EvictionObserver>>,
}

impl EvictionQueue {
    fn new() -> Self {
        Self {
            next_handle: AtomicUsize::new(0),
            observers: async_lock::Mutex::new(BTreeMap::new()),
        }
    }

    async fn add_observer(&self, observer: EvictionObserver) -> usize {
        let mut observers = self.observers.lock().await;
        let handle = self.next_handle.fetch_add(1, Ordering::SeqCst);
        observers.insert(handle, observer);
        handle
    }

    async fn remove_observer(&self, handle: usize) {
        let mut observers = self.observers.lock().await;
        observers.remove(&handle);
    }
}

pub struct MemoryViewBase {
    contents: async_lock::RwLock<MemoryViewContents>,
    eviction_queue: Option<EvictionQueue>,
}

impl MemoryViewBase {
    pub fn new() -> Self {
        Self {
            contents: async_lock::RwLock::new(HashMap::new()),
            eviction_queue: None,
        }
    }

    pub fn new_with_eviction_queue() -> Self {
        Self {
            contents: async_lock::RwLock::new(HashMap::new()),
            eviction_queue: Some(EvictionQueue::new()),
        }
    }

    pub fn can_evict_memory(&self) -> bool {
        self.eviction_queue.is_some()
    }

    pub async fn add_eviction_observer(&self, sender: EvictionObserver) -> usize {
        let queue = self
            .eviction_queue
            .as_ref()
            .expect("This memory view does not support eviction");

        queue.add_observer(sender).await
    }

    pub async fn remove_eviction_observer(&self, handle: usize) {
        let queue = self
            .eviction_queue
            .as_ref()
            .expect("This memory view does not support eviction");

        queue.remove_observer(handle).await;
    }

    pub async fn contents(&self) -> async_lock::RwLockReadGuard<'_, MemoryViewContents> {
        self.contents.read().await
    }

    pub async fn contents_mut(&self) -> async_lock::RwLockWriteGuard<'_, MemoryViewContents> {
        self.contents.write().await
    }
}
