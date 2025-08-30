use core::sync::atomic::{AtomicU64, Ordering};

use crate::{
    arch::memory::{PAGE_SHIFT, PAGE_SIZE},
    memory::{CachingMode, PageAccess, PageStatus, accessor::PageAccessor, space::PageSpace},
};

pub trait CursorPolicy {
    /// Maximum possible number of table levels.
    const MAX_LEVELS: usize;
    /// How many bits of the address are used per level.
    const BITS_PER_LEVEL: usize;

    /// Amount of levels currently in use.
    fn num_levels() -> usize;

    /// Synchronize the page table write with the page table walker.
    fn pte_write_barrier();

    /// Check whether the given PTE marks a page as present.
    fn pte_page_present(pte: u64) -> bool;
    /// Get the page address from the given PTE.
    fn pte_page_address(pte: u64) -> u64;
    /// Get the status (present, dirty) from the given PTE.
    fn pte_page_status(pte: u64) -> PageStatus;
    /// Check whether the given PTE can be accessed with the given flags.
    fn pte_page_can_access(pte: u64, flags: PageAccess) -> bool;
    /// Clean the given PTE (remove the dirty status).
    fn pte_page_clean(pte: u64) -> u64;
    /// Construct a new PTE from the given parameters.
    fn pte_page_new(address: u64, access: PageAccess, caching: CachingMode) -> u64;

    /// Check whether the given PTE say the table is present.
    fn pte_table_present(pte: u64) -> bool;
    /// Get the table address from the given PTE.
    fn pte_table_address(pte: u64) -> u64;
    /// Allocate a new page table and construct a PTE for it.
    fn pte_table_new() -> u64;
}

pub struct PageCursor<'a, P: CursorPolicy>
where
    [PageAccessor; P::MAX_LEVELS]: Sized,
{
    space: &'a PageSpace,
    accessors: [PageAccessor; P::MAX_LEVELS],
    initial_level: usize,
    address: u64,
}

impl<'a, P: CursorPolicy> PageCursor<'a, P>
where
    [PageAccessor; P::MAX_LEVELS]: Sized,
{
    const LAST_LEVEL: usize = P::MAX_LEVELS - 1;
    const LEVEL_MASK: u64 = (1 << P::BITS_PER_LEVEL) - 1;

    pub fn new(space: &'a PageSpace, address: u64) -> Self {
        let initial_level = P::MAX_LEVELS - P::num_levels();
        let mut accessors = [const { PageAccessor::null() }; P::MAX_LEVELS];

        accessors[initial_level] = PageAccessor::new(space.root_table());

        Self {
            space,
            accessors,
            initial_level,
            address,
        }
    }

    pub fn address(&self) -> u64 {
        self.address
    }

    pub fn move_to(&mut self, address: u64) {
        for i in self.initial_level + 1..P::MAX_LEVELS {
            if (self.address ^ address) & (Self::LEVEL_MASK << Self::level_shift(i)) != 0 {
                self.accessors[i..P::MAX_LEVELS].fill(PageAccessor::null());
                break;
            }
        }

        self.address = address;

        self.reload_level(Self::LAST_LEVEL);
    }

    pub fn advance_page(&mut self) {
        self.move_to(self.address + PAGE_SIZE as u64);
    }

    pub fn map_page(&mut self, physical_address: u64, access: PageAccess, caching: CachingMode) {
        if self.accessors[Self::LAST_LEVEL].is_null() {
            self.ensure_page_levels();
        }

        self.current_pte().store(
            P::pte_page_new(physical_address, access, caching),
            Ordering::Relaxed,
        );

        P::pte_write_barrier();
    }

    pub fn remap_page(
        &mut self,
        physical_address: u64,
        access: PageAccess,
        caching: CachingMode,
    ) -> PageStatus {
        if self.accessors[Self::LAST_LEVEL].is_null() {
            self.ensure_page_levels();
        }

        let new_pte = P::pte_page_new(physical_address, access, caching);
        let old_pte = self.current_pte().swap(new_pte, Ordering::Relaxed);

        P::pte_write_barrier();
        P::pte_page_status(old_pte)
    }

    pub fn clean_page(&self) {
        if !self.accessors[Self::LAST_LEVEL].is_null() {
            let old_pte = self.current_pte().load(Ordering::Acquire);
            let new_pte = P::pte_page_clean(old_pte);

            self.current_pte().store(new_pte, Ordering::Release);
        }
    }

    pub fn unmap_page(&mut self) -> Option<(u64, PageStatus)> {
        if !self.accessors[Self::LAST_LEVEL].is_null() || self.reload_level(Self::LAST_LEVEL) {
            let pte = self.current_pte().swap(0, Ordering::Relaxed);

            P::pte_write_barrier();

            if P::pte_page_present(pte) {
                Some((P::pte_page_address(pte), P::pte_page_status(pte)))
            } else {
                None
            }
        } else {
            None
        }
    }

    fn current_pte(&self) -> &AtomicU64 {
        unsafe {
            AtomicU64::from_ptr(self.accessors[Self::LAST_LEVEL].as_mut::<u64>().offset(
                ((self.address >> Self::level_shift(Self::LAST_LEVEL)) & Self::LEVEL_MASK) as isize,
            ))
        }
    }

    fn reload_level(&mut self, level: usize) -> bool {
        if self.accessors[level].is_null() {
            assert_ne!(level, self.initial_level);

            if !self.reload_level(level - 1) {
                return false;
            }

            let accessor = &self.accessors[level - 1];
            let pte_ptr = unsafe {
                AtomicU64::from_ptr(accessor.as_mut::<u64>().offset(
                    ((self.address >> Self::level_shift(level - 1)) & Self::LEVEL_MASK) as isize,
                ))
            };

            let pte = pte_ptr.load(Ordering::Acquire);

            if !P::pte_table_present(pte) {
                return false;
            }

            let table_address = P::pte_table_address(pte);

            self.accessors[level] = PageAccessor::new(table_address);
        }

        true
    }

    fn ensure_level(&mut self, level: usize) {
        if self.accessors[level].is_null() {
            assert_ne!(level, self.initial_level);

            self.ensure_level(level - 1);

            let accessor = &self.accessors[level - 1];
            let pte_ptr = unsafe {
                AtomicU64::from_ptr(accessor.as_mut::<u64>().offset(
                    ((self.address >> Self::level_shift(level - 1)) & Self::LEVEL_MASK) as isize,
                ))
            };

            let pte = pte_ptr.load(Ordering::Acquire);
            let table_address = if P::pte_table_present(pte) {
                P::pte_table_address(pte)
            } else {
                let new_pte = P::pte_table_new();
                let table_address = P::pte_table_address(new_pte);

                pte_ptr.store(new_pte, Ordering::Release);

                P::pte_write_barrier();

                table_address
            };

            self.accessors[level] = PageAccessor::new(table_address);
        }
    }

    fn ensure_page_levels(&mut self) {
        self.ensure_level(Self::LAST_LEVEL);
    }

    fn level_shift(level: usize) -> usize {
        (P::MAX_LEVELS - level - 1) * P::BITS_PER_LEVEL + PAGE_SHIFT
    }
}
