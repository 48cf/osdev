use core::sync::atomic::{AtomicUsize, Ordering};

use alloc::{collections::btree_map::BTreeMap, sync::Arc};
use spin::Mutex;

use crate::memory::view::MemoryView;

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Handle(usize);

impl Default for Handle {
    fn default() -> Self {
        Self::NULL
    }
}

impl Handle {
    pub const NULL: Self = Self(0 as isize as usize);
    pub const THIS_UNIVERSE: Self = Self(-1 as isize as usize);
    pub const THIS_THREAD: Self = Self(-2 as isize as usize);
    pub const ZERO_MEMORY: Self = Self(-3 as isize as usize);

    pub fn from_id(id: usize) -> Self {
        Self(id)
    }

    pub fn id(&self) -> usize {
        self.0
    }
}

#[derive(Clone)]
pub enum Descriptor {
    MemoryView(Arc<dyn MemoryView>),
}

struct UniverseInner {
    handles: BTreeMap<Handle, Descriptor>,
}

pub struct Universe {
    next_handle: AtomicUsize,
    inner: Mutex<UniverseInner>,
}

impl Universe {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            inner: Mutex::new(UniverseInner {
                handles: BTreeMap::new(),
            }),
            next_handle: AtomicUsize::new(0),
        })
    }

    pub fn attach_descriptor(&self, descriptor: Descriptor) -> Handle {
        let handle = Handle(self.next_handle.fetch_add(1, Ordering::SeqCst));
        self.inner.lock().handles.insert(handle, descriptor);
        handle
    }

    pub fn detach_descriptor(&self, handle: Handle) {
        self.inner.lock().handles.remove(&handle);
    }

    pub fn get(&self, handle: Handle) -> Option<Descriptor> {
        self.inner.lock().handles.get(&handle).cloned()
    }
}
