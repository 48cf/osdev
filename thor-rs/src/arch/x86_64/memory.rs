use core::{
    arch::{asm, naked_asm},
    mem::offset_of,
};

use crate::{
    arch::{cpu::ArchCpuData, executor::ArchExecutor},
    memory::{
        self, CachingMode, PageAccess, PageStatus,
        accessor::PageAccessor,
        client::{UserAccessFlags, UserAccessRegion},
        cursor::CursorPolicy,
    },
    scheduler::Executor,
};

pub const PAGE_SIZE: usize = 0x1000;
pub const PAGE_SHIFT: usize = 12;

pub fn activate_page_table(root_table: u64) {
    unsafe {
        asm!("mov cr3, {}", in(reg) root_table);
    }
}

pub fn copy_from_user(address: usize, buffer: &mut [u8]) -> bool {
    do_copy_from_user(buffer.as_mut_ptr(), address as *const u8, buffer.len())
}

pub fn copy_to_user(address: usize, buffer: &[u8]) -> bool {
    do_copy_to_user(address as *mut u8, buffer.as_ptr(), buffer.len())
}

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

#[unsafe(naked)]
extern "C" fn do_copy_from_user(dest: *mut u8, src: *const u8, len: usize) -> bool {
    naked_asm!(
        "mov rcx, rdx",
        "mov r8, gs:[{active_executor}]",
        "lea rax, [rip + 5f]",
        "mov [r8 + {user_access_region}], rax",

        "2:",
        "rep movsb",

        "3:",
        "xor eax, eax",
        "mov [r8 + {user_access_region}], rax",
        "ret",

        "4:",
        "xor eax, eax",
        "mov [r8 + {user_access_region}], rax",
        "mov eax, 1",
        "ret",

        ".align {user_access_region_align}",
        "5:",
        ".quad 2b",
        ".quad 3b",
        ".quad 4b",
        ".long {user_access_read}",

        active_executor = const offset_of!(ArchCpuData, current_executor),
        user_access_region = const offset_of!(ArchExecutor, user_access_region),
        user_access_region_align = const align_of::<UserAccessRegion>(),
        user_access_read = const UserAccessFlags::READ.bits(),
    );
}

#[unsafe(naked)]
extern "C" fn do_copy_to_user(dest: *mut u8, src: *const u8, len: usize) -> bool {
    naked_asm!(
        "mov rcx, rdx",
        "mov r8, gs:[{active_executor}]",
        "lea rax, [rip + 5f]",
        "mov [r8 + {user_access_region}], rax",

        "2:",
        "rep movsb",

        "3:",
        "xor eax, eax",
        "mov [r8 + {user_access_region}], rax",
        "ret",

        "4:",
        "xor eax, eax",
        "mov [r8 + {user_access_region}], rax",
        "mov eax, 1",
        "ret",

        ".align {user_access_region_align}",
        "5:",
        ".quad 2b",
        ".quad 3b",
        ".quad 4b",
        ".long {user_access_write}",

        active_executor = const offset_of!(ArchCpuData, current_executor),
        user_access_region = const offset_of!(ArchExecutor, user_access_region),
        user_access_region_align = const align_of::<UserAccessRegion>(),
        user_access_write = const UserAccessFlags::WRITE.bits(),
    );
}
