use core::{
    ptr::NonNull,
    sync::atomic::{AtomicUsize, Ordering},
};

use spin::{Lazy, Mutex};

use crate::{
    arch::memory::{PAGE_SHIFT, PAGE_SIZE},
    boot::eir::{EirInfo, EirRegion},
};

struct BuddyAllocator {
    base_address: u64,
    order: usize,
    num_roots: usize,
    tree: &'static mut [i8],
}

impl BuddyAllocator {
    const PAGE_SHIFT: usize = 12;

    fn new(region: &EirRegion) -> Self {
        let mut size = 0;
        for order in 0..=region.order {
            size += region.num_roots << (region.order - order);
        }

        let tree =
            unsafe { core::slice::from_raw_parts_mut(region.buddy_tree as *mut i8, size as usize) };

        Self {
            base_address: region.address as u64,
            order: region.order as usize,
            num_roots: region.num_roots as usize,
            tree,
        }
    }

    fn allocate(&mut self, order: usize, address_bits: usize) -> Option<u64> {
        if order > self.order {
            return None;
        }

        let mut offset = 0;
        let mut current_order = self.order;
        let mut index = self.find_allocatable_chunk(
            self.tree,
            0,
            self.num_roots,
            current_order,
            order,
            address_bits,
        )?;

        while current_order != order {
            offset += self.num_roots << (self.order - current_order);
            current_order -= 1;
            index = self.find_allocatable_chunk(
                &self.tree[offset..],
                index * 2,
                2,
                current_order,
                order,
                address_bits,
            )?;
        }

        assert_eq!(self.tree[offset + index], order as i8);
        self.tree[offset + index] = -1;

        let mut update_index = index;

        while current_order < self.order {
            update_index /= 2;

            let free_order =
                Self::scan_free_chunks(&self.tree[offset..], update_index * 2, 2, current_order);

            current_order += 1;
            offset -= self.num_roots << (self.order - current_order);

            self.tree[offset + update_index] = free_order;
        }

        let physical = self.base_address + (index << (order + Self::PAGE_SHIFT)) as u64;

        if address_bits != usize::BITS as usize {
            assert!((physical >> address_bits) == 0);
        }

        Some(physical)
    }

    fn free(&mut self, address: u64, order: usize) {
        assert!(address >= self.base_address);
        assert!(order >= 0 && order <= self.order);

        let index = ((address - self.base_address) >> Self::PAGE_SHIFT) as usize;

        assert!(index % (1 << order) == 0);

        let mut current_order = self.order;
        let mut offset = 0;

        while current_order != order {
            offset += self.num_roots << (self.order - current_order);
            current_order -= 1;
        }

        let mut update_index = index >> order;

        assert_eq!(self.tree[offset + update_index], -1);
        self.tree[offset + update_index] = current_order as i8;

        while update_index != 0 {
            update_index /= 2;

            let free_order =
                Self::scan_free_chunks(&self.tree[offset..], update_index * 2, 2, current_order);

            current_order += 1;
            offset -= self.num_roots << (self.order - current_order);

            self.tree[offset + update_index] = free_order;
        }
    }

    fn find_allocatable_chunk(
        &self,
        tree: &[i8],
        start: usize,
        limit: usize,
        current_order: usize,
        target_order: usize,
        address_bits: usize,
    ) -> Option<usize> {
        let index = (0..limit).find(|i| tree[start + i] >= target_order as i8)?;

        if address_bits != usize::BITS as usize {
            let chunk_size = 1 << (current_order + Self::PAGE_SHIFT);
            let address = self.base_address + ((start + index) * chunk_size) as u64;
            let address_limit = 1_u64 << address_bits;

            if (current_order == target_order && address + (chunk_size as u64) > address_limit)
                || (current_order > target_order && address >= address_limit)
            {
                return None;
            }
        }

        Some(start + index)
    }

    fn scan_free_chunks(tree: &[i8], start: usize, limit: usize, order: usize) -> i8 {
        let mut free_order: i8 = -1;
        let mut all_equal_order = true;

        for i in 0..limit {
            if tree[start + i] >= free_order {
                free_order = tree[start + i];
            }

            if tree[start + i] != order as i8 {
                all_equal_order = false;
            }
        }

        if all_equal_order {
            order as i8 + 1
        } else {
            free_order
        }
    }
}

struct AllocatorRegion {
    physical_address: u64,
    region_size: usize,
    buddy: BuddyAllocator,
}

struct StateInner {
    regions: [Option<AllocatorRegion>; Self::MAX_REGIONS],
    num_regions: usize,
}

impl StateInner {
    const MAX_REGIONS: usize = 8;

    const fn new() -> Self {
        Self {
            regions: [const { None }; Self::MAX_REGIONS],
            num_regions: 0,
        }
    }

    fn regions(&self) -> impl Iterator<Item = &AllocatorRegion> {
        self.regions[0..self.num_regions]
            .iter()
            .filter_map(|region| region.as_ref())
    }

    fn regions_mut(&mut self) -> impl Iterator<Item = &mut AllocatorRegion> {
        self.regions[0..self.num_regions]
            .iter_mut()
            .filter_map(|region| region.as_mut())
    }
}

struct State {
    inner: Mutex<StateInner>,
    total_pages: AtomicUsize,
    free_pages: AtomicUsize,
    used_pages: AtomicUsize,
}

static STATE: Lazy<State> = Lazy::new(|| {
    let memory_layout = super::MEMORY_LAYOUT_NOTE.get();
    let eir_info = unsafe {
        NonNull::new(memory_layout.eir_info() as *mut EirInfo)
            .unwrap()
            .as_ref()
    };

    let mut state_inner = StateInner::new();
    let mut total_pages = 0;

    assert!(eir_info.num_regions as usize <= StateInner::MAX_REGIONS);

    for (i, region) in eir_info.regions().iter().enumerate() {
        state_inner.num_regions += 1;
        state_inner.regions[i] = Some(AllocatorRegion {
            physical_address: region.address as u64,
            region_size: region.length as usize,
            buddy: BuddyAllocator::new(region),
        });

        total_pages += (region.num_roots << region.order) as usize;
    }

    State {
        inner: Mutex::new(state_inner),
        total_pages: AtomicUsize::new(total_pages),
        free_pages: AtomicUsize::new(total_pages),
        used_pages: AtomicUsize::new(0),
    }
});

pub fn allocate_restricted(size: usize, address_bits: usize) -> Option<u64> {
    let order = ((size + PAGE_SIZE - 1) >> PAGE_SHIFT)
        .next_power_of_two()
        .trailing_zeros() as usize;

    let mut inner = STATE.inner.lock();

    for region in inner.regions_mut() {
        if order > region.buddy.order {
            continue;
        }

        if let Some(physical) = region.buddy.allocate(order, address_bits) {
            STATE.free_pages.fetch_sub(1 << order, Ordering::Relaxed);
            STATE.used_pages.fetch_add(1 << order, Ordering::Relaxed);

            return Some(physical);
        }
    }

    None
}

pub fn allocate(size: usize) -> Option<u64> {
    allocate_restricted(size, usize::BITS as _)
}

pub fn free(address: u64, size: usize) {
    let order = ((size + PAGE_SIZE - 1) >> PAGE_SHIFT)
        .next_power_of_two()
        .trailing_zeros() as usize;

    let mut inner = STATE.inner.lock();

    for region in inner.regions_mut() {
        if address < region.physical_address
            || (address + size as u64) - region.physical_address > region.region_size as u64
        {
            continue;
        }

        region.buddy.free(address, order);
    }

    panic!(
        "Physical page {:#x} is not part of any memory region",
        address
    );
}

pub fn total_pages() -> usize {
    STATE.total_pages.load(Ordering::Relaxed)
}

pub fn free_pages() -> usize {
    STATE.free_pages.load(Ordering::Relaxed)
}

pub fn used_pages() -> usize {
    STATE.used_pages.load(Ordering::Relaxed)
}
