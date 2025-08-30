use alloc::{boxed::Box, sync::Arc};
use async_trait::async_trait;
use spin::Mutex;

use crate::{
    arch::memory::PAGE_SIZE,
    memory::{
        self, CachingMode, PageAccess,
        accessor::PageAccessor,
        cursor::{CursorPolicy, PageCursor},
        kernel::KERNEL_PAGE_SPACE,
        view::MemoryView,
    },
};

struct PageSpaceInner;

pub struct PageSpace {
    root_table: u64,
    inner: Mutex<PageSpaceInner>,
}

#[async_trait]
pub trait VirtualSpace {
    async fn map_present_pages(
        &self,
        memory_view: &Arc<impl MemoryView>,
        virtual_address: u64,
        offset: usize,
        length: usize,
        access: PageAccess,
        caching: CachingMode,
    );
    async fn fault_page(
        &self,
        memory_view: &Arc<impl MemoryView>,
        virtual_address: u64,
        offset: usize,
        access: PageAccess,
        caching: CachingMode,
    );
}

impl PageSpace {
    pub unsafe fn new_from_root_physical(root_table: u64) -> Self {
        Self {
            root_table,
            inner: Mutex::new(PageSpaceInner),
        }
    }

    pub fn new_user() -> Self {
        let root_table = memory::page::allocate(PAGE_SIZE).expect("Out of physical memory");

        let pt_accessor = PageAccessor::new(root_table);
        let kernel_pt_accessor = PageAccessor::new(KERNEL_PAGE_SPACE.space().root_table());

        // Zero out the lower half
        unsafe {
            for i in 256..512 {
                pt_accessor.as_mut::<u64>().add(i).write(0);
            }
        }

        // Copy over higher half mappings
        unsafe {
            let higher_half = pt_accessor.as_mut::<u64>().add(256);
            let kernel_higher_half = kernel_pt_accessor.as_mut::<u64>().add(256);

            higher_half.copy_from_nonoverlapping(kernel_higher_half, 256);
        }

        Self {
            root_table,
            inner: Mutex::new(PageSpaceInner),
        }
    }

    pub fn root_table(&self) -> u64 {
        self.root_table
    }

    pub fn activate(&self) {
        unsafe {
            core::arch::asm!("mov cr3, {}", in(reg) self.root_table());
        }
    }
}

pub async fn map_present_pages_with_cursor<P: CursorPolicy>(
    space: &PageSpace,
    memory_view: &Arc<impl MemoryView>,
    virtual_address: u64,
    offset: usize,
    length: usize,
    access: PageAccess,
    caching: CachingMode,
) where
    [PageAccessor; P::MAX_LEVELS]: Sized,
{
    assert!(virtual_address & (PAGE_SIZE as u64 - 1) == 0);
    assert!(offset & (PAGE_SIZE - 1) == 0);
    assert!(length & (PAGE_SIZE - 1) == 0);

    let contents = memory_view.base().lock().await;
    let mut cursor = PageCursor::<P>::new(space, virtual_address);

    while cursor.address() < virtual_address + length as u64 {
        let offset = cursor.address() - virtual_address + offset as u64;

        if let Some((physical_address, caching_mode)) =
            contents.get(&(offset as usize / PAGE_SIZE)).copied()
        {
            cursor.map_page(
                physical_address,
                access,
                caching_mode.override_with(caching),
            );
        }

        cursor.advance_page();
    }
}
