use intrusive_collections::{KeyAdapter, RBTree, RBTreeAtomicLink, intrusive_adapter};
use spin::{Lazy, Mutex};

use crate::{
    arch::memory::PAGE_SIZE,
    memory::{accessor::PageAccessor, page},
};

#[derive(Clone)]
struct KernelVirtualHole {
    hook: RBTreeAtomicLink,
    address: u64,
    size: usize,
}

intrusive_adapter! {
    KernelVirtualHoleAdapter<'a> = &'a KernelVirtualHole: KernelVirtualHole { hook: RBTreeAtomicLink }
}

impl<'a, 'b> KeyAdapter<'a> for KernelVirtualHoleAdapter<'b> {
    type Key = u64;

    fn get_key(&self, hole: &'a KernelVirtualHole) -> Self::Key {
        hole.address
    }
}

type KernelVirtualTree<'a> = RBTree<KernelVirtualHoleAdapter<'a>>;

static KERNEL_VIRTUAL_TREE: Lazy<Mutex<KernelVirtualTree<'static>>> = Lazy::new(|| {
    let memory_layout = super::MEMORY_LAYOUT_NOTE.get();

    // TODO: This should be packaged into a reusable allocator.
    let hole = unsafe {
        let page = page::allocate(PAGE_SIZE).unwrap();
        let accessor = PageAccessor::new(page);
        let hole = accessor.as_mut::<KernelVirtualHole>();

        hole.write(KernelVirtualHole {
            hook: RBTreeAtomicLink::new(),
            address: memory_layout.kernel_virtual(),
            size: memory_layout.kernel_virtual_size(),
        });

        &*hole
    };

    let mut tree = KernelVirtualTree::new(KernelVirtualHoleAdapter::new());

    tree.insert(hole);

    Mutex::new(tree)
});

pub fn allocate_virtual_memory(size: usize) -> Option<u64> {
    let pages = (size + PAGE_SIZE - 1) / PAGE_SIZE;

    let mut tree = KERNEL_VIRTUAL_TREE.lock();
    let mut best_hole = None;
    let mut best_size = usize::MAX;

    for hole in tree.iter() {
        if hole.size >= pages * PAGE_SIZE && hole.size < best_size {
            best_hole = Some(hole);
            best_size = hole.size;
        }
    }

    if let Some(hole) = best_hole {
        let address = hole.address;
        let size = hole.size;
        let hole = hole as *const _;

        unsafe {
            tree.cursor_mut_from_ptr(hole).remove();
        }

        if size > pages * PAGE_SIZE {
            let new_hole = unsafe {
                let page = page::allocate(PAGE_SIZE).unwrap();
                let accessor = PageAccessor::new(page);
                let new_hole = accessor.as_mut::<KernelVirtualHole>();

                new_hole.write(KernelVirtualHole {
                    hook: RBTreeAtomicLink::new(),
                    address: address + (pages * PAGE_SIZE) as u64,
                    size: size - (pages * PAGE_SIZE),
                });

                &*new_hole
            };

            tree.insert(new_hole);
        }

        Some(address)
    } else {
        None
    }
}

pub fn free_virtual_memory(address: u64, size: usize) {
    crate::println!(
        "Freeing virtual memory not implemented yet, address={:#x} size={:#x}",
        address,
        size
    );
}

pub fn dump_virtual_tree() {
    let tree = KERNEL_VIRTUAL_TREE.lock();

    for hole in tree.iter() {
        crate::println!(
            "Virtual hole: {:#x} - {:#x} ({:#x} bytes)",
            hole.address,
            hole.address + hole.size as u64,
            hole.size
        );
    }
}
