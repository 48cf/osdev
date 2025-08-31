use core::num::NonZeroU64;

use alloc::sync::Arc;

use crate::{
    KernelError, KernelResult,
    arch::memory::PAGE_SIZE,
    memory::{
        CachingMode, PageAccess,
        client::MapFlags,
        view::{AllocatedMemory, ImmediateMemory, MemorySlice, MemoryView},
    },
    scheduler::{self, LOCAL_SCHEDULER},
    universe::{Descriptor, Handle},
};

macro_rules! ensure {
    ($expr:expr, $err:expr) => {
        if !$expr {
            return Err($err);
        }
    };
}

pub type SyscallResult = KernelResult<(usize, usize)>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogSeverity {
    Emergency,
    Alert,
    Critical,
    Error,
    Warning,
    Notice,
    Info,
    Debug,
}

impl TryFrom<usize> for LogSeverity {
    type Error = ();

    fn try_from(value: usize) -> Result<Self, ()> {
        match value {
            0 => Ok(LogSeverity::Emergency),
            1 => Ok(LogSeverity::Alert),
            2 => Ok(LogSeverity::Critical),
            3 => Ok(LogSeverity::Error),
            4 => Ok(LogSeverity::Warning),
            5 => Ok(LogSeverity::Notice),
            6 => Ok(LogSeverity::Info),
            7 => Ok(LogSeverity::Debug),
            _ => Err(()),
        }
    }
}

pub fn hel_log(severity: usize, ptr: usize, length: usize) -> SyscallResult {
    let _severity = LogSeverity::try_from(severity).map_err(|_| KernelError::IllegalArgs)?;
    let message = unsafe { core::str::from_raw_parts(ptr as *const u8, length) };

    crate::print!("{}", message);

    Ok(Default::default())
}

pub fn hel_allocate_memory(length: usize, flags: usize, _restrictions: usize) -> SyscallResult {
    ensure!(
        length > 0 && length.is_multiple_of(PAGE_SIZE),
        KernelError::IllegalArgs
    );

    let thread = LOCAL_SCHEDULER
        .get()
        .current()
        .as_thread()
        .expect("No current thread");

    let memory: Arc<dyn MemoryView> = if flags & hel_sys::kHelAllocContinuous as usize != 0 {
        assert!(flags & hel_sys::kHelAllocOnDemand as usize != 0);

        AllocatedMemory::new_contiguous(length)
    } else if flags & hel_sys::kHelAllocOnDemand as usize != 0 {
        AllocatedMemory::new(length)
    } else {
        ImmediateMemory::new(length)
    };

    let handle = thread
        .universe()
        .attach_descriptor(Descriptor::MemoryView(memory));

    Ok((handle.id(), 0))
}

pub fn hel_map_memory(
    memory_handle: usize,
    space_handle: usize,
    address: usize,
    offset: usize,
    length: usize,
    _flags: usize,
) -> SyscallResult {
    ensure!(
        length > 0 && length.is_multiple_of(PAGE_SIZE),
        KernelError::IllegalArgs
    );
    ensure!(offset.is_multiple_of(PAGE_SIZE), KernelError::IllegalArgs);
    ensure!(length.is_multiple_of(PAGE_SIZE), KernelError::IllegalArgs);

    let memory_handle = Handle::from_id(memory_handle);
    let space_handle = Handle::from_id(space_handle);

    assert!(space_handle == Handle::NULL);

    let thread = LOCAL_SCHEDULER
        .get()
        .current()
        .as_thread()
        .expect("No current thread");

    let descriptor = thread
        .universe()
        .get(memory_handle)
        .ok_or(KernelError::BadDescriptor)?;

    match descriptor {
        Descriptor::MemoryView(view) => {
            let address = scheduler::async_block(
                &thread,
                thread.space().map(
                    MemorySlice::new(view, offset, length, CachingMode::Null),
                    NonZeroU64::new(address as u64),
                    0,
                    length,
                    MapFlags::PREFER_TOP,
                    PageAccess::READ_WRITE,
                ),
            )?;

            Ok((address as usize, 0))
        }
    }
}
