use core::num::NonZeroU64;

use alloc::{boxed::Box, collections::btree_map::BTreeMap, sync::Arc};
use async_trait::async_trait;
use bitflags::bitflags;
use spin::Mutex;

use crate::{
    Error, Result,
    arch::memory::{PAGE_SIZE, UserCursorPolicy},
    memory::{
        self, CachingMode, PageAccess,
        cursor::PageCursor,
        space::{PageSpace, VirtualSpace},
        view::{MemorySlice, MemoryView},
    },
};

bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct MapFlags: u32 {
        const FIXED = 1 << 0;
        const PREFER_BOTTOM = 1 << 1;
        const PREFER_TOP = 1 << 2;
        const DONT_REQUIRE_BACKING = 1 << 10;
        const FIXED_NO_REPLACE = 1 << 11;
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

struct VirtualMapping {
    slice: Arc<MemorySlice>,
    length: usize,
    flags: MapFlags,
    access: PageAccess,
}

struct ClientPageSpaceInner {
    holes: BTreeMap<u64, VirtualHole>,
    mappings: BTreeMap<u64, VirtualMapping>,
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
    ) -> Result<u64> {
        if let Some(address) = virtual_address {
            assert!(address.get() & (PAGE_SIZE as u64 - 1) == 0);
        }

        assert!(offset & (PAGE_SIZE - 1) == 0);
        assert!(length & (PAGE_SIZE - 1) == 0);

        if offset + length > slice.length() {
            return Err(Error::OutOfBounds);
        }

        let address = if flags.contains(MapFlags::FIXED) {
            let Some(requested_address) = virtual_address else {
                return Err(Error::IllegalArgs);
            };

            self.allocate_at(requested_address.get(), length)
                .ok_or(Error::NoMemory)?
        } else if flags.contains(MapFlags::FIXED_NO_REPLACE) {
            todo!()
        } else {
            if let Some(requested_address) = virtual_address
                && let Some(allocated_address) = self.allocate_at(requested_address.get(), length)
            {
                allocated_address
            } else {
                self.allocate_anywhere(length, flags)
                    .ok_or(Error::NoMemory)?
            }
        };

        self.mappings.insert(
            address,
            VirtualMapping {
                slice: slice.clone(),
                length,
                flags,
                access,
            },
        );

        Ok(address)
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

    pub async fn map(
        &self,
        slice: Arc<MemorySlice>,
        virtual_address: Option<NonZeroU64>,
        offset: usize,
        length: usize,
        flags: MapFlags,
        access: PageAccess,
    ) -> Result<u64> {
        let caching_mode = if slice.caching_mode() == CachingMode::WriteCombine {
            CachingMode::WriteCombine
        } else {
            CachingMode::Null
        };

        let address = self
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
        _memory_view: &Arc<dyn MemoryView>,
        _virtual_address: u64,
        _offset: usize,
        _access: super::PageAccess,
        _caching: super::CachingMode,
    ) {
        todo!()
    }
}
