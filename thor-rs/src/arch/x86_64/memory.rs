use crate::memory::{
    self, CachingMode, PageAccess, PageStatus, accessor::PageAccessor, cursor::CursorPolicy,
};

pub const PAGE_SIZE: usize = 0x1000;
pub const PAGE_SHIFT: usize = 12;

pub struct ArchCursorPolicy<const KERNEL: bool>;

impl<const KERNEL: bool> ArchCursorPolicy<KERNEL> {
    const PTE_PRESENT: u64 = 1 << 0;
    const PTE_WRITE: u64 = 1 << 1;
    const PTE_USER: u64 = 1 << 2;
    const PTE_PWT: u64 = 1 << 3;
    const PTE_PCD: u64 = 1 << 4;
    const PTE_DIRTY: u64 = 1 << 6;
    const PTE_PAT: u64 = 1 << 7;
    const PTE_GLOBAL: u64 = 1 << 8;
    const PTE_XD: u64 = 1 << 63;
    const PTE_ADDRESS: u64 = 0x000F_FFFF_FFFF_F000;
}

impl<const KERNEL: bool> CursorPolicy for ArchCursorPolicy<KERNEL> {
    const MAX_LEVELS: usize = 4;
    const BITS_PER_LEVEL: usize = 9;

    fn num_levels() -> usize {
        4
    }

    fn pte_write_barrier() {}

    fn pte_page_present(pte: u64) -> bool {
        pte & Self::PTE_PRESENT != 0
    }

    fn pte_page_address(pte: u64) -> u64 {
        pte & Self::PTE_ADDRESS
    }

    fn pte_page_status(pte: u64) -> PageStatus {
        if pte & Self::PTE_PRESENT == 0 {
            PageStatus::empty()
        } else {
            let mut status = PageStatus::PRESENT;

            if pte & Self::PTE_DIRTY != 0 {
                status |= PageStatus::DIRTY;
            }

            status
        }
    }

    fn pte_page_can_access(pte: u64, flags: PageAccess) -> bool {
        if pte & Self::PTE_PRESENT == 0 {
            return false;
        }

        if flags.contains(PageAccess::EXECUTE) && pte & Self::PTE_XD != 0 {
            return false;
        }

        if flags.contains(PageAccess::WRITE) && pte & Self::PTE_WRITE == 0 {
            return false;
        }

        true
    }

    fn pte_page_clean(pte: u64) -> u64 {
        pte & !Self::PTE_DIRTY
    }

    fn pte_page_new(
        address: u64,
        access: crate::memory::PageAccess,
        caching: crate::memory::CachingMode,
    ) -> u64 {
        let mut pte = Self::PTE_PRESENT | (address & Self::PTE_ADDRESS);

        if KERNEL {
            pte |= Self::PTE_GLOBAL;
        } else {
            pte |= Self::PTE_USER;
        }

        if access.contains(PageAccess::WRITE) {
            pte |= Self::PTE_WRITE;
        }

        if !access.contains(PageAccess::EXECUTE) {
            pte |= Self::PTE_XD;
        }

        let caching_bits = match caching {
            CachingMode::Null | CachingMode::WriteBack => 0,
            CachingMode::Uncached | CachingMode::Mmio | CachingMode::MmioNonPosted => Self::PTE_PCD,
            CachingMode::WriteCombine => Self::PTE_PAT | Self::PTE_PWT,
            CachingMode::WriteThrough => Self::PTE_PWT,
        };

        pte | caching_bits
    }

    fn pte_table_present(pte: u64) -> bool {
        pte & Self::PTE_PRESENT != 0
    }

    fn pte_table_address(pte: u64) -> u64 {
        pte & Self::PTE_ADDRESS
    }

    fn pte_table_new() -> u64 {
        let physical_page = memory::page::allocate(PAGE_SIZE).expect("Out of physical memory");
        let accessor = PageAccessor::new(physical_page);

        unsafe {
            accessor.as_mut::<u8>().write_bytes(0, PAGE_SIZE);
        }

        Self::PTE_PRESENT | Self::PTE_WRITE | Self::PTE_USER | physical_page
    }
}

pub type KernelCursorPolicy = ArchCursorPolicy<true>;
pub type UserCursorPolicy = ArchCursorPolicy<false>;
