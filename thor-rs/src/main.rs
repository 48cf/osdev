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

use core::{panic::PanicInfo, ptr::NonNull};

use alloc::sync::Arc;

use crate::{
    arch::{executor::ArchExecutor, memory::PAGE_SIZE},
    boot::eir::{EirInfo, EirModule},
    memory::{
        CachingMode, MEMORY_LAYOUT_NOTE, PageAccess,
        client::ClientPageSpace,
        kernel::KERNEL_PAGE_SPACE,
        space::VirtualSpace,
        view::{ImmediateMemory, MemoryView},
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

    // scheduler.schedule(Fiber::run(|| {
    //     let fiber = LOCAL_SCHEDULER
    //         .get()
    //         .current()
    //         .and_then(|e| e.as_fiber())
    //         .unwrap();

    //     scheduler::async_block(&fiber, async {
    //         let mut i = 0;
    //         loop {
    //             tx.send(i).await.unwrap();
    //             i += 1;
    //         }
    //     });
    // }));

    // scheduler.schedule(Fiber::run(|| {
    //     let fiber = LOCAL_SCHEDULER
    //         .get()
    //         .current()
    //         .and_then(|e| e.as_fiber())
    //         .unwrap();

    //     scheduler::async_block(&fiber, async {
    //         loop {
    //             let x = rx.recv().await.unwrap();
    //             println!("Fiber received: {}", x);
    //         }
    //     });
    // }));
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

    let freya_bin = cpio_reader::iter_files(initrd)
        .find(|entry| entry.name() == "freya")
        .expect("No freya in initrd");

    let scheduler = LOCAL_SCHEDULER.get();
    let this_fiber = scheduler.current().and_then(|e| e.as_fiber()).unwrap();

    let memory = Arc::new(ImmediateMemory::new(0x1000));

    scheduler::async_block(&this_fiber, async {
        memory.copy_to(0, &[0x0F, 0x0B]).await.unwrap();
    });

    let space = ClientPageSpace::new();

    scheduler::async_block(
        &this_fiber,
        space.map_present_pages(
            &memory,
            0x1000,
            0,
            0x1000,
            PageAccess::READ | PageAccess::EXECUTE,
            CachingMode::Null,
        ),
    );

    let thread = Thread::new(ArchExecutor::new_user_context(0x1000, 0, 0, 0), space);

    scheduler.schedule(thread);
}

// extern "C" fn thread_main(_arg0: usize, _arg1: usize) -> ! {
//     println!("Thread started with args: {}, {}", _arg0, _arg1);

//     let thread = LOCAL_SCHEDULER
//         .get()
//         .current()
//         .and_then(|entity| entity.as_thread())
//         .expect("No current thread");

//     println!("About to block on an async operation");

//     thread.block(thread.next_block_token());

//     scheduler::async_block(&thread, async {
//         println!("Thread is running asynchronously");
//     });

//     println!("Thread finished, halting");

//     loop {
//         unsafe {
//             core::arch::asm!("hlt");
//         }
//     }
// }

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
    // Internal error: the remote has violated the IPC protocol.
    ProtocolViolation,
    SpuriousOperation,
    AlreadyExists,
}

pub type Result<T> = core::result::Result<T, Error>;
