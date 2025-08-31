use core::num::NonZeroU64;

use alloc::sync::Arc;

use crate::{
    KernelError, KernelResult,
    arch::{image::SyscallRegisterImage, memory::PAGE_SIZE},
    memory::{
        CachingMode, PageAccess,
        client::MapFlags,
        user::copy_from_user,
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

pub fn hel_log(image: &impl SyscallRegisterImage) -> SyscallResult {
    let severity = image.arg0();
    let ptr = image.arg1();
    let length = image.arg2();

    let _severity = LogSeverity::try_from(severity).map_err(|_| KernelError::IllegalArgs)?;
    let message = unsafe { core::str::from_raw_parts(ptr as *const u8, length) };

    crate::print!("{}", message);

    Ok(Default::default())
}

pub fn hel_allocate_memory(image: &impl SyscallRegisterImage) -> SyscallResult {
    let length = image.arg0();
    let flags = image.arg1();
    let restrictions_addr = image.arg2();

    ensure!(
        length > 0 && length.is_multiple_of(PAGE_SIZE),
        KernelError::IllegalArgs
    );

    let address_bits = if restrictions_addr != 0 {
        let mut restrictions: hel_sys::HelAllocRestrictions = unsafe { core::mem::zeroed() };

        copy_from_user(restrictions_addr, unsafe {
            core::slice::from_raw_parts_mut(
                &raw mut restrictions as *mut u8,
                size_of::<hel_sys::HelAllocRestrictions>(),
            )
        })?;

        restrictions.addressBits as usize
    } else {
        64
    };

    let thread = LOCAL_SCHEDULER
        .get()
        .current()
        .as_thread()
        .expect("No current thread");

    let memory: Arc<dyn MemoryView> = if flags & hel_sys::kHelAllocContinuous as usize != 0 {
        assert!(flags & hel_sys::kHelAllocOnDemand as usize != 0);

        AllocatedMemory::new_contiguous(length, address_bits)
    } else if flags & hel_sys::kHelAllocOnDemand as usize != 0 {
        AllocatedMemory::new(length, address_bits)
    } else {
        ImmediateMemory::new(length)
    };

    let handle = thread
        .universe()
        .attach_descriptor(Descriptor::MemoryView(memory));

    Ok((handle.id(), 0))
}

pub fn hel_map_memory(image: &impl SyscallRegisterImage) -> SyscallResult {
    let memory_handle = image.arg0();
    let space_handle = image.arg1();
    let address = image.arg2();
    let offset = image.arg3();
    let length = image.arg4();
    let _flags = image.arg5();

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
