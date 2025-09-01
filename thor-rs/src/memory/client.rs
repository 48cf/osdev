use core::num::NonZeroU64;

use alloc::{boxed::Box, collections::btree_map::BTreeMap, sync::Arc};
use async_channel::Receiver;
use async_lock::Barrier;
use async_trait::async_trait;
use bitflags::bitflags;
use spin::Mutex;

use crate::{
    KernelError, KernelResult,
    arch::memory::{PAGE_SIZE, UserCursorPolicy},
    memory::{
        self, CachingMode, PageAccess,
        cursor::PageCursor,
        space::{PageSpace, VirtualSpace},
        view::{Eviction, MemorySlice, MemoryView},
    },
    per_cpu::CPU_DATA,
};

bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct UserAccessFlags: u32 {
        const READ = 1 << 0;
        const WRITE = 1 << 1;
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct MapFlags: u32 {
        const FIXED = 1 << 0;
        const PREFER_BOTTOM = 1 << 1;
        const PREFER_TOP = 1 << 2;
        const DONT_REQUIRE_BACKING = 1 << 10;
        const FIXED_NO_REPLACE = 1 << 11;
    }
}

impl UserAccessFlags {
    pub fn to_page_access(self) -> PageAccess {
        let mut access = PageAccess::empty();

        if self.contains(UserAccessFlags::READ) {
            access |= PageAccess::READ;
        }
        if self.contains(UserAccessFlags::WRITE) {
            access |= PageAccess::WRITE;
        }

        access
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct UserAccessRegion {
    start_ip: usize,
    end_ip: usize,
    fault_ip: usize,
    flags: UserAccessFlags,
}

impl UserAccessRegion {
    pub fn contains_address(&self, address: usize) -> bool {
        address >= self.start_ip && address < self.end_ip
    }

    pub fn fault_handler(&self) -> usize {
        self.fault_ip
    }

    pub fn flags(&self) -> UserAccessFlags {
        self.flags
    }
}

#[derive(Clone, Copy, Debug)]
struct VirtualHole {
    length: usize,
}

impl VirtualHole {
    fn new(length: usize) -> Self {
        Self { length }
    }
}

struct Mapping {
    slice: Arc<MemorySlice>,
    length: usize,
    flags: MapFlags,
    access: PageAccess,
}

impl Mapping {
    async fn handle_eviction(&self, rx: Receiver<(Eviction, Arc<Barrier>)>) {
        loop {
            crate::println!("Waiting for an eviction notice...");

            if let Ok((eviction, _barrier)) = rx.recv().await {
                crate::println!("Eviction: {:#?}", eviction);

                // barrier.
            }
        }
    }
}

struct ClientPageSpaceInner {
    holes: BTreeMap<u64, VirtualHole>,
    mappings: BTreeMap<u64, Arc<Mapping>>,
}

impl ClientPageSpaceInner {
    fn new() -> Self {
        const LOWER_HALF_BITS: usize = 47;

        let mut holes = BTreeMap::new();
        holes.insert(
            PAGE_SIZE as u64,
            VirtualHole::new((1 << LOWER_HALF_BITS) - PAGE_SIZE),
        );

        Self {
            holes,
            mappings: BTreeMap::new(),
        }
    }

    fn split_hole(&mut self, (start, hole): (u64, VirtualHole), offset: usize, length: usize) {
        self.holes.remove(&start);

        // If the hole starts before our requested address, reinsert the left part.
        if offset > 0 {
            self.holes.insert(start, VirtualHole::new(offset));
        }

        // If the hole extends beyond our requested mapping, reinsert the right part.
        if offset + length < hole.length {
            self.holes.insert(
                start + (offset + length) as u64,
                VirtualHole::new(hole.length - (offset + length)),
            );
        }
    }

    fn allocate_at(&mut self, address: u64, length: usize) -> Option<u64> {
        // Find the first hole where the key less than or equal to the requested address.
        if let Some((&start, &hole)) = self.holes.range(..=address).next_back() {
            let hole_end = start + hole.length as u64;
            let req_end = address + length as u64;

            // Make sure our requested mapping fits within the hole.
            if hole_end < req_end {
                return None;
            }

            self.split_hole((start, hole), (address - start) as usize, length);

            return Some(address);
        }

        None
    }

    fn allocate_anywhere(&mut self, length: usize, flags: MapFlags) -> Option<u64> {
        assert!(flags.contains(MapFlags::PREFER_BOTTOM) ^ flags.contains(MapFlags::PREFER_TOP));

        let mut candidate = None;

        if flags.contains(MapFlags::PREFER_BOTTOM) {
            for (start, hole) in self.holes.iter() {
                if hole.length >= length {
                    candidate = Some((*start, *hole));
                    break;
                }
            }
        } else {
            for (start, hole) in self.holes.iter().rev() {
                if hole.length >= length {
                    candidate = Some((*start, *hole));
                    break;
                }
            }
        }

        if let Some((start, hole)) = candidate {
            if flags.contains(MapFlags::PREFER_BOTTOM) {
                self.split_hole((start, hole), 0, length);

                Some(start)
            } else {
                self.split_hole((start, hole), hole.length - length, length);

                Some(start + (hole.length - length) as u64)
            }
        } else {
            None
        }
    }

    async fn map(
        &mut self,
        slice: Arc<MemorySlice>,
        virtual_address: Option<NonZeroU64>,
        offset: usize,
        length: usize,
        flags: MapFlags,
        access: PageAccess,
    ) -> KernelResult<(u64, Arc<Mapping>)> {
        if let Some(address) = virtual_address {
            assert!(address.get() & (PAGE_SIZE as u64 - 1) == 0);
        }

        assert!(offset & (PAGE_SIZE - 1) == 0);
        assert!(length & (PAGE_SIZE - 1) == 0);

        if offset + length > slice.length() {
            return Err(KernelError::OutOfBounds);
        }

        let address = if flags.contains(MapFlags::FIXED) {
            let Some(requested_address) = virtual_address else {
                return Err(KernelError::IllegalArgs);
            };

            self.allocate_at(requested_address.get(), length)
                .ok_or(KernelError::NoMemory)?
        } else if flags.contains(MapFlags::FIXED_NO_REPLACE) {
            todo!()
        } else {
            if let Some(requested_address) = virtual_address
                && let Some(allocated_address) = self.allocate_at(requested_address.get(), length)
            {
                allocated_address
            } else {
                self.allocate_anywhere(length, flags)
                    .ok_or(KernelError::NoMemory)?
            }
        };

        let mapping = Arc::new(Mapping {
            slice: slice.clone(),
            length,
            flags,
            access,
        });

        self.mappings.insert(address, mapping.clone());

        Ok((address, mapping))
    }
}

pub struct ClientPageSpace {
    space: PageSpace,
    inner: Mutex<ClientPageSpaceInner>,
}

impl ClientPageSpace {
    pub fn new() -> Arc<Self> {
        let space = PageSpace::new_user();

        Arc::new(Self {
            space,
            inner: Mutex::new(ClientPageSpaceInner::new()),
        })
    }

    pub fn cursor(&self, address: u64) -> PageCursor<'_, UserCursorPolicy> {
        PageCursor::new(&self.space, address)
    }

    pub fn space(&self) -> &PageSpace {
        &self.space
    }

    pub async fn handle_page_fault(
        &self,
        address: u64,
        fault_access: PageAccess,
    ) -> KernelResult<()> {
        if let Some((&start, mapping)) = self.inner.lock().mappings.range(..=address).next_back() {
            if address >= start + mapping.length as u64 || !mapping.access.contains(fault_access) {
                return Err(KernelError::Fault);
            }

            let slice = mapping.slice.clone();
            let offset = slice.offset() + (address - start) as usize;

            self.fault_page(
                slice.view(),
                address & !(PAGE_SIZE as u64 - 1),
                slice.offset() + offset,
                fault_access,
                slice.caching_mode(),
            )
            .await?;

            Ok(())
        } else {
            Err(KernelError::Fault)
        }
    }

    pub async fn map(
        &self,
        slice: Arc<MemorySlice>,
        virtual_address: Option<NonZeroU64>,
        offset: usize,
        length: usize,
        flags: MapFlags,
        access: PageAccess,
    ) -> KernelResult<u64> {
        let caching_mode = if slice.caching_mode() == CachingMode::WriteCombine {
            CachingMode::WriteCombine
        } else {
            CachingMode::Null
        };

        let (address, mapping) = self
            .inner
            .lock()
            .map(
                slice.clone(),
                virtual_address,
                offset,
                length,
                flags,
                access,
            )
            .await?;

        self.map_present_pages(
            slice.view(),
            address,
            slice.offset() + offset,
            length,
            access,
            caching_mode,
        )
        .await;

        if slice.view().base().can_evict_memory() {
            let (_, task) = async_task::spawn(
                async move {
                    let (tx, rx) = async_channel::unbounded();

                    slice.view().base().add_eviction_observer(tx);
                    mapping.handle_eviction(rx).await;
                },
                |runnable| CPU_DATA.get().work_queue().submit(runnable),
            );

            task.detach();
        }

        Ok(address)
    }
}

#[async_trait]
impl VirtualSpace for ClientPageSpace {
    async fn map_present_pages(
        &self,
        memory_view: &Arc<dyn MemoryView>,
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
        memory_view: &Arc<dyn MemoryView>,
        virtual_address: u64,
        offset: usize,
        access: super::PageAccess,
        caching: super::CachingMode,
    ) -> KernelResult<()> {
        memory_view.fault_in(offset, access).await?;

        let &(physical_address, caching_mode, kind) = memory_view
            .base()
            .contents()
            .await
            .get(&(offset / PAGE_SIZE))
            .ok_or(KernelError::Fault)?;

        assert!(kind.is_compatible(access));

        let mut cursor = self.cursor(virtual_address);

        cursor.map_page(
            physical_address,
            access,
            caching_mode.override_with(caching),
        );

        Ok(())
    }
}
