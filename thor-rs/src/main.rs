#![allow(incomplete_features)]
#![feature(associated_type_defaults)]
#![feature(generic_const_exprs)]
#![feature(never_type)]
#![feature(sized_hierarchy)]
#![feature(str_from_raw_parts)]
#![no_main]
#![no_std]

extern crate alloc;

mod arch;
mod boot;
mod memory;
mod per_cpu;
mod scheduler;
mod syscalls;
mod universe;

use core::{num::NonZeroU64, panic::PanicInfo, ptr::NonNull};

use crate::{
    arch::{
        executor::ArchExecutor,
        image::{
            FaultErrorCode, FaultKind, FaultRegisterImage, ImageDomain, IrqRegisterImage,
            SyscallRegisterImage,
        },
        memory::PAGE_SIZE,
    },
    boot::eir::{EirInfo, EirModule},
    memory::{
        CachingMode, MEMORY_LAYOUT_NOTE, PageAccess,
        client::{ClientPageSpace, MapFlags},
        kernel::KERNEL_PAGE_SPACE,
        view::{AllocatedMemory, ImmediateMemory, MemorySlice, MemoryView},
    },
    per_cpu::CPU_DATA,
    scheduler::{Executor, Fiber, LOCAL_SCHEDULER, Thread},
};

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ({
        let _ = core::fmt::write(&mut $crate::E9Writer, format_args!($($arg)*));
    });
}

#[macro_export]
macro_rules! println {
    () => (print!("\n"));
    ($fmt:expr) => ($crate::print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => ($crate::print!(concat!($fmt, "\n"), $($arg)*));
}

#[unsafe(no_mangle)]
extern "C" fn kernel_main() -> ! {
    println!("Kernel main reached");

    per_cpu::init_for_boot_processor();

    let scheduler = LOCAL_SCHEDULER.get();

    scheduler.schedule(Fiber::run(init_fiber));
    scheduler.force_reschedule();
    scheduler.commit_reschedule();
}

fn handle_page_fault(image: &mut impl FaultRegisterImage) {
    // TODO: Check SMAP before continuing.

    let fault_access = image.error_code().into_page_access();
    let thread = LOCAL_SCHEDULER
        .get()
        .current()
        .as_thread()
        .expect("No current thread");

    if let Ok(()) = scheduler::async_block(
        &thread,
        thread.space().handle_page_fault(
            image.fault_address() as u64,
            image.error_code().into_page_access(),
        ),
    ) {
        return;
    }

    if !image.error_code().is_user() {
        if let Some(user_access) = unsafe {
            CPU_DATA
                .get()
                .arch_data()
                .current_executor()
                .and_then(|executor| executor.user_access_region())
        } {
            if user_access.contains_address(image.ip())
                && user_access.flags().to_page_access().contains(fault_access)
            {
                image.set_ip(user_access.fault_handler());
            }
        }
    }
}

pub fn handle_fault(image: &mut impl FaultRegisterImage) {
    if image.fault_kind() == FaultKind::PageFault {
        return handle_page_fault(image);
    }

    if image.fault_kind() == FaultKind::Breakpoint && image.domain() == ImageDomain::User {
        let thread = LOCAL_SCHEDULER
            .get()
            .current()
            .as_thread()
            .expect("No current thread");

        println!("thor: Breakpoint in user thread {}", thread.tid());
        return;
    }

    println!("thor: Unexpected fault: {:?}", image.fault_kind());

    image.dump_registers();

    loop {
        unsafe {
            core::arch::asm!("hlt");
        }
    }
}

pub fn handle_interrupt(image: &impl IrqRegisterImage) {
    image.dump_registers();

    todo!()
}

pub fn handle_syscall(image: &mut impl SyscallRegisterImage) {
    let result = match image.syscall_number() as u32 {
        hel_sys::kHelCallLog => syscalls::hel_log(image),
        hel_sys::kHelCallAllocateMemory => syscalls::hel_allocate_memory(image),
        hel_sys::kHelCallMapMemory => syscalls::hel_map_memory(image),
        _ => {
            println!("thor: Unknown syscall number: {}", image.syscall_number());

            Err(KernelError::IllegalSyscall)
        }
    };

    match result {
        Ok((a, b)) => {
            image.set_error(hel_sys::kHelErrNone as usize);
            image.set_out0(a);
            image.set_out1(b);
        }
        Err(err) => {
            let error = match err {
                KernelError::IllegalSyscall => hel_sys::kHelErrIllegalSyscall,
                KernelError::IllegalArgs => hel_sys::kHelErrIllegalArgs,
                KernelError::IllegalState => hel_sys::kHelErrIllegalState,
                KernelError::UnsupportedOperation => hel_sys::kHelErrUnsupportedOperation,
                KernelError::OutOfBounds => hel_sys::kHelErrOutOfBounds,
                KernelError::QueueTooSmall => hel_sys::kHelErrQueueTooSmall,
                KernelError::Cancelled => hel_sys::kHelErrCancelled,
                KernelError::NoDescriptor => hel_sys::kHelErrNoDescriptor,
                KernelError::BadDescriptor => hel_sys::kHelErrBadDescriptor,
                KernelError::ThreadTerminated => hel_sys::kHelErrThreadTerminated,
                KernelError::TransmissionMismatch => hel_sys::kHelErrTransmissionMismatch,
                KernelError::LaneShutdown => hel_sys::kHelErrLaneShutdown,
                KernelError::EndOfLane => hel_sys::kHelErrEndOfLane,
                KernelError::Dismissed => hel_sys::kHelErrDismissed,
                KernelError::BufferTooSmall => hel_sys::kHelErrBufferTooSmall,
                KernelError::Fault => hel_sys::kHelErrFault,
                KernelError::RemoteFault => hel_sys::kHelErrRemoteFault,
                KernelError::NoHardwareSupport => hel_sys::kHelErrNoHardwareSupport,
                KernelError::NoMemory => hel_sys::kHelErrNoMemory,
                KernelError::AlreadyExists => hel_sys::kHelErrAlreadyExists,
            };

            image.set_error(error as usize);
        }
    }
}

fn init_fiber() {
    let memory_layout = MEMORY_LAYOUT_NOTE.get();
    let eir_info = unsafe {
        NonNull::new(memory_layout.eir_info() as *mut EirInfo)
            .unwrap()
            .as_ref()
    };
    let module_info = unsafe {
        NonNull::new(eir_info.module_info as *mut EirModule)
            .unwrap()
            .as_ref()
    };
    let length = (module_info.length as usize + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);
    let initrd_virtual =
        memory::heap::allocate_virtual_memory(length).expect("Out of virtual memory");

    let mut cursor = KERNEL_PAGE_SPACE.cursor(initrd_virtual);

    for i in (0..module_info.length).step_by(PAGE_SIZE) {
        cursor.map_page(
            module_info.physical_base + i,
            PageAccess::READ,
            CachingMode::Null,
        );
        cursor.advance_page();
    }

    let initrd = unsafe {
        core::slice::from_raw_parts(initrd_virtual as *const u8, module_info.length as usize)
    };

    let scheduler = LOCAL_SCHEDULER.get();
    let this_fiber = scheduler.current().as_fiber().expect("No current fiber");

    let freya_bytes = cpio_reader::iter_files(initrd)
        .find(|entry| entry.name() == "freya")
        .map(|entry| entry.file())
        .expect("No freya in initrd");

    let space = ClientPageSpace::new();

    let (ip, sp, initrd_address) = scheduler::async_block(&this_fiber, async {
        let initrd_len = initrd.len().next_multiple_of(PAGE_SIZE);
        let initrd_memory = ImmediateMemory::new(initrd_len);

        initrd_memory.copy_to(0, initrd).await?;

        let elf_memory = ImmediateMemory::new(freya_bytes.len());
        let stack_memory = AllocatedMemory::new(0x10000, 64);

        elf_memory.copy_to(0, freya_bytes).await?;

        let freya_elf = goblin::elf::Elf::parse(freya_bytes).expect("Failed to parse freya ELF");

        for phdr in freya_elf
            .program_headers
            .iter()
            .filter(|phdr| phdr.p_type == goblin::elf::program_header::PT_LOAD)
        {
            let misalign = phdr.p_vaddr & (PAGE_SIZE as u64 - 1);
            let virtual_address = phdr.p_vaddr - misalign;
            let offset = (phdr.p_offset - misalign) as usize;
            let length =
                (phdr.p_memsz as usize + misalign as usize + (PAGE_SIZE - 1)) & !(PAGE_SIZE - 1);

            let view = MemorySlice::new(elf_memory.clone(), offset, length, CachingMode::Null);
            let mut access = PageAccess::empty();

            if phdr.p_flags & goblin::elf::program_header::PF_R != 0 {
                access |= PageAccess::READ;
            }

            if phdr.p_flags & goblin::elf::program_header::PF_W != 0 {
                access |= PageAccess::WRITE;
            }

            if phdr.p_flags & goblin::elf::program_header::PF_X != 0 {
                access |= PageAccess::EXECUTE;
            }

            space
                .map(
                    view,
                    NonZeroU64::new(virtual_address),
                    0,
                    length,
                    MapFlags::FIXED,
                    access,
                )
                .await?;
        }

        let sp = space
            .map(
                MemorySlice::new(stack_memory, 0, 0x10000, CachingMode::Null),
                None,
                0,
                0x10000,
                MapFlags::PREFER_TOP,
                PageAccess::READ | PageAccess::WRITE,
            )
            .await?;

        let initrd_address = space
            .map(
                MemorySlice::new(initrd_memory, 0, initrd_len, CachingMode::Null),
                None,
                0,
                initrd_len,
                MapFlags::PREFER_TOP,
                PageAccess::READ,
            )
            .await?;

        Ok((freya_elf.entry, sp + 0x10000, initrd_address))
    })
    .expect("Failed to setup freya address space");

    scheduler.schedule(Thread::new(
        ArchExecutor::new_user_context(
            ip as usize,
            sp as usize,
            initrd_address as usize,
            initrd.len(),
        ),
        space,
    ));
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("Oops: {}", info);

    loop {
        arch::halt();
    }
}

pub struct E9Writer;

impl core::fmt::Write for E9Writer {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for byte in s.bytes() {
            unsafe {
                outb(0xe9, byte);
            }
        }
        Ok(())
    }
}

unsafe fn outb(port: u16, value: u8) {
    unsafe {
        core::arch::asm!("out dx, al", in("dx") port, in("al") value, options(nostack, preserves_flags));
    }
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KernelError {
    IllegalSyscall,
    IllegalArgs,
    IllegalState,
    UnsupportedOperation,
    OutOfBounds,
    QueueTooSmall,
    Cancelled,
    NoDescriptor,
    BadDescriptor,
    ThreadTerminated,
    TransmissionMismatch,
    LaneShutdown,
    EndOfLane,
    Dismissed,
    BufferTooSmall,
    Fault,
    RemoteFault,
    NoHardwareSupport,
    NoMemory,
    AlreadyExists,
}

pub type KernelResult<T> = core::result::Result<T, KernelError>;
