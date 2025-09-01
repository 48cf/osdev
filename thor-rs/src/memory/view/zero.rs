use alloc::{boxed::Box, sync::Arc};
use async_trait::async_trait;

use crate::{
    KernelResult,
    memory::{
        PageAccess,
        view::{MemoryView, MemoryViewBase},
    },
};

pub struct ZeroMemory;

impl ZeroMemory {
    pub fn new() -> Arc<Self> {
        Arc::new(Self)
    }
}

#[async_trait]
impl MemoryView for ZeroMemory {
    fn base(&self) -> &MemoryViewBase {
        unreachable!("ZeroMemory::base should not be called");
    }

    async fn fault_in(&self, _offset: usize, _access: PageAccess) -> KernelResult<()> {
        unreachable!("ZeroMemory::fault_in should not be called");
    }

    async fn copy_to(&self, _offset: usize, _buffer: &[u8]) -> KernelResult<()> {
        unreachable!("ZeroMemory::copy_to should not be called");
    }

    async fn copy_from(&self, _offset: usize, buffer: &mut [u8]) -> KernelResult<()> {
        buffer.fill(0);
        Ok(())
    }
}
