use crate::{KernelError, KernelResult, arch};

pub fn copy_from_user(address: usize, buffer: &mut [u8]) -> KernelResult<()> {
    if address.checked_add(address + buffer.len()).is_none() {
        return Err(KernelError::Fault);
    }

    // TODO: Disable and enable access checks

    if arch::memory::copy_from_user(address, buffer) {
        Err(KernelError::Fault)
    } else {
        Ok(())
    }
}

pub fn copy_to_user(address: usize, buffer: &[u8]) -> KernelResult<()> {
    if address.checked_add(address + buffer.len()).is_none() {
        return Err(KernelError::Fault);
    }

    // TODO: Disable and enable access checks

    if arch::memory::copy_to_user(address, buffer) {
        Err(KernelError::Fault)
    } else {
        Ok(())
    }
}
