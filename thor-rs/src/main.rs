#![allow(incomplete_features)]
#![feature(associated_type_defaults)]
#![feature(generic_const_exprs)]
#![feature(never_type)]
#![no_main]
#![no_std]

extern crate alloc;

mod arch;
mod boot;
mod memory;
mod per_cpu;
mod scheduler;

use core::{num::NonZeroU64, panic::PanicInfo, ptr::NonNull};

use alloc::sync::Arc;

use crate::{
    arch::{executor::ArchExecutor, memory::PAGE_SIZE},
    boot::eir::{EirInfo, EirModule},
    memory::{
        CachingMode, MEMORY_LAYOUT_NOTE, PageAccess,
        client::{ClientPageSpace, MapFlags},
        kernel::KERNEL_PAGE_SPACE,
        view::{ImmediateMemory, MemorySlice, MemoryView},
    },
    scheduler::{Fiber, LOCAL_SCHEDULER, Thread},
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
    let this_fiber = scheduler.current().and_then(|e| e.as_fiber()).unwrap();

    let freya_bytes = cpio_reader::iter_files(initrd)
        .find(|entry| entry.name() == "freya")
        .map(|entry| entry.file())
        .expect("No freya in initrd");

    let space = ClientPageSpace::new();

    let (ip, sp) = scheduler::async_block(&this_fiber, async {
        let memory = Arc::new(ImmediateMemory::new(freya_bytes.len()));
        let stack_memory = Arc::new(ImmediateMemory::new(0x10000));

        memory.copy_to(0, freya_bytes).await.unwrap();

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

            let view = MemorySlice::new(memory.clone(), offset, length, CachingMode::Null);
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
                .await
                .unwrap();
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
            .await
            .unwrap();

        (freya_elf.entry, sp + 0x10000)
    });

    scheduler.schedule(Thread::new(
        ArchExecutor::new_user_context(ip as usize, sp as usize, 0, 0),
        space,
    ));
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("Oops: {}", info);

    loop {
        unsafe {
            core::arch::asm!("hlt");
        }
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
pub enum Error {
    IllegalArgs = 1,
    IllegalObject,
    IllegalState,
    OutOfBounds,
    Cancelled,
    FutexRace,
    BufferTooSmall,
    ThreadExited,
    TransmissionMismatch,
    LaneShutdown,
    EndOfLane,
    Dismissed,
    Fault,
    RemoteFault,
    NoMemory,
    NoHardwareSupport,
    HardwareBroken,
    ProtocolViolation,
    SpuriousOperation,
    AlreadyExists,
}

pub type Result<T> = core::result::Result<T, Error>;
