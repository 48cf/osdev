use alloc::boxed::Box;
use async_trait::async_trait;

use crate::{
    KernelResult,
    memory::{
        PageAccess,
        view::{MemoryView, MemoryViewBase},
    },
};

pub struct CopyOnWriteView;

#[async_trait]
impl MemoryView for CopyOnWriteView {
    fn base(&self) -> &MemoryViewBase {
        todo!()
    }

    async fn fault_in(&self, _offset: usize, _access: PageAccess) -> KernelResult<()> {
        todo!()
    }
}
