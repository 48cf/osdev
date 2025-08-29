pub mod accessor;
pub mod cursor;
pub mod heap;
pub mod page;
pub mod space;
pub mod stack;

use core::{
    alloc::{GlobalAlloc, Layout},
    mem::MaybeUninit,
    ptr::NonNull,
};

use bitflags::bitflags;
use rlsf::Tlsf;
use spin::{Lazy, Mutex};

use crate::{
    arch::memory::PAGE_SIZE, boot::elf_note::MemoryLayout, elf_note, memory::space::KernelPageSpace,
};

elf_note! {
    pub static MEMORY_LAYOUT_NOTE: MemoryLayout = MemoryLayout::new();
}

pub static KERNEL_PAGE_SPACE: Lazy<KernelPageSpace> = Lazy::new(|| KernelPageSpace::new());

#[global_allocator]
static ALLOCATOR: KernelAllocator = KernelAllocator {
    state: Mutex::new(KernelAllocatorState { tlsf: Tlsf::new() }),
};

struct KernelAllocatorState {
    tlsf: Tlsf<'static, u16, u16, 12, 16>,
}

impl KernelAllocatorState {
    const HEAP_BLOCK_SIZE: usize = 2 << 21; // 2MiB

    fn alloc(&mut self, layout: Layout) -> *mut u8 {
        if let Some(ptr) = self.tlsf.allocate(layout) {
            return ptr.as_ptr();
        }

        let virtual_addr =
            heap::allocate_virtual_memory(Self::HEAP_BLOCK_SIZE).expect("Out of virtual memory");

        // Map in the entire block
        let mut cursor = KERNEL_PAGE_SPACE.cursor(virtual_addr);

        for _ in (0..Self::HEAP_BLOCK_SIZE).step_by(PAGE_SIZE) {
            let physical_page = page::allocate(PAGE_SIZE).expect("Out of physical memory");

            cursor.map_page(
                physical_page,
                PageAccess::READ | PageAccess::WRITE,
                CachingMode::Null,
            );
            cursor.advance_page();
        }

        self.tlsf.insert_free_block(unsafe {
            core::slice::from_raw_parts_mut(
                virtual_addr as *mut MaybeUninit<u8>,
                Self::HEAP_BLOCK_SIZE,
            )
        });

        self.tlsf
            .allocate(layout)
            .map_or(core::ptr::null_mut(), |ptr| ptr.as_ptr())
    }

    fn dealloc(&mut self, ptr: *mut u8, layout: Layout) {
        unsafe {
            self.tlsf
                .deallocate(NonNull::new(ptr).unwrap(), layout.align())
        };
    }
}

struct KernelAllocator {
    state: Mutex<KernelAllocatorState>,
}

unsafe impl GlobalAlloc for KernelAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.state.lock().alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.state.lock().dealloc(ptr, layout);
    }
}

bitflags! {
    pub struct PageAccess: u32 {
        const READ = 1 << 0;
        const WRITE = 1 << 1;
        const EXECUTE = 1 << 2;
    }

    pub struct PageStatus: u8 {
        const PRESENT = 1 << 0;
        const DIRTY = 1 << 1;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CachingMode {
    Null,
    Uncached,
    WriteCombine,
    WriteThrough,
    WriteBack,
    Mmio,
    MmioNonPosted,
}
