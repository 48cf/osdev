use crate::arch::memory::PAGE_SIZE;

#[derive(Clone, Copy)]
pub struct PageAccessor(u64);

impl PageAccessor {
    pub const fn null() -> Self {
        Self(0)
    }

    pub fn new(physical_address: u64) -> Self {
        assert!(physical_address & (PAGE_SIZE as u64 - 1) == 0);
        assert!(physical_address < 0x4000_0000_0000);

        Self(super::MEMORY_LAYOUT_NOTE.get().direct_physical() + physical_address)
    }

    pub fn is_null(&self) -> bool {
        self.0 == 0
    }

    pub unsafe fn as_mut<T>(&self) -> *mut T {
        assert!(!self.is_null());

        self.0 as usize as *mut T
    }
}
