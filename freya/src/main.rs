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

use core::{arch::naked_asm, panic};

use alloc::vec::Vec;

use arch::{gdt::Gdt, idt::Idt};

use crate::{
    arch::executor::ArchExecutor,
    scheduler::{Blockable, Executor, Fiber, LOCAL_SCHEDULER, Thread},
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

seq_macro::seq! {
    N in 0..256 {
        #[unsafe(naked)]
        extern "C" fn kernel_interrupt_stub_~N() {
            naked_asm!(
                // These codes push an error on the stack, do nothing.
                ".if ({i} == 8 || ({i} >= 10 && {i} <= 14) || {i} == 17 || {i} == 21 || {i} == 29 || {i} == 30)",
                // All other ones don't, so we need to push something ourselves.
                ".else",
                "push 0",
                ".endif",

                // Push the interrupt number.
                "push {i}",
                "jmp {kernel_interrupt_stub_common}",

                i = const N,
                kernel_interrupt_stub_common = sym kernel_interrupt_stub_common
            );
        }
    }
}

#[unsafe(naked)]
extern "C" fn kernel_interrupt_stub_common() {
    naked_asm!(
        "push rax",
        "push rbx",
        "push rcx",
        "push rdx",
        "push rbp",
        "push rdi",
        "push rsi",
        "push r8",
        "push r9",
        "push r10",
        "push r11",
        "push r12",
        "push r13",
        "push r14",
        "push r15",
        "cld",
        // Zero out the base pointer since we can't trust it.
        "xor rbp, rbp",
        // Load the frame as first argument.
        "mov rdi, rsp",
        "call {kernel_interrupt_handler}",
        "jmp {kernel_interrupt_stub_return}",

        kernel_interrupt_handler = sym kernel_interrupt_handler,
        kernel_interrupt_stub_return = sym kernel_interrupt_stub_return
    );
}

#[unsafe(naked)]
extern "C" fn kernel_interrupt_stub_return() {
    naked_asm!(
        "pop r15",
        "pop r14",
        "pop r13",
        "pop r12",
        "pop r11",
        "pop r10",
        "pop r9",
        "pop r8",
        "pop rsi",
        "pop rdi",
        "pop rbp",
        "pop rdx",
        "pop rcx",
        "pop rbx",
        "pop rax",
        // Skip `error` and `interrupt_number` fields.
        "add rsp, 0x10",
        "iretq",
    );
}

#[repr(C)]
#[derive(Debug)]
pub struct InterruptFrame {
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub r11: u64,
    pub r10: u64,
    pub r9: u64,
    pub r8: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub rbp: u64,
    pub rdx: u64,
    pub rcx: u64,
    pub rbx: u64,
    pub rax: u64,
    // Pushed onto the stack by the interrupt handler stubs.
    pub interrupt_number: u64,
    // Pushed onto the stack by the CPU if the interrupt has an error code.
    pub error: u64,
    // The rest is pushed onto the stack by the CPU during an interrupt.
    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
    pub rsp: u64,
    pub ss: u64,
}

const _: () = {
    // Make sure the interrupt frame is 16-byte aligned.
    assert!(size_of::<InterruptFrame>() % 16 == 0);
};

extern "C" fn kernel_interrupt_handler(frame: &mut InterruptFrame) {
    println!("Exception: {}", frame.interrupt_number);

    println!("Register state:");
    println!(
        "  RAX: {:#018x}  RBX: {:#018x}  RCX: {:#018x}",
        frame.rax, frame.rbx, frame.rcx
    );
    println!(
        "  RDX: {:#018x}  RDI: {:#018x}  RSI: {:#018x}",
        frame.rdx, frame.rdi, frame.rsi
    );
    println!(
        "  R8:  {:#018x}  R9:  {:#018x}  R10: {:#018x}",
        frame.r8, frame.r9, frame.r10
    );
    println!(
        "  R11: {:#018x}  R12: {:#018x}  R13: {:#018x}",
        frame.r11, frame.r12, frame.r13
    );
    println!("  R14: {:#018x}  R15: {:#018x}", frame.r14, frame.r15);
    println!("  Error code: {:#x}", frame.error);
    println!("  RIP: {:#x}", frame.rip);

    if frame.interrupt_number == 14 {
        let mut cr2: u64;

        unsafe {
            core::arch::asm!("mov {}, cr2", out(reg) cr2);
        }

        println!("  Faulting address: {:#x}", cr2);
    }

    loop {
        unsafe {
            core::arch::asm!("hlt");
        }
    }
}

#[unsafe(no_mangle)]
extern "C" fn kernel_main() -> ! {
    println!("Kernel main reached");

    let gdt = Gdt::new();
    let mut idt = Idt::new();

    unsafe {
        gdt.load();
    }

    per_cpu::init_for_boot_processor();

    seq_macro::seq! {
        N in 0..256 {
            idt.set_handler(N, kernel_interrupt_stub_~N, Gdt::KERNEL_CODE64_SELECTOR, 0, 0x8E);
        }
    }

    unsafe {
        idt.load();
    }

    {
        let mut vec = Vec::with_capacity(128);

        vec.push(1);
        vec.push(2);
        vec.push(3);

        println!("Vector contents: {:?}", vec);
    }

    // memory::heap::dump_virtual_tree();

    // let memory_layout = MEMORY_LAYOUT_NOTE.data();
    // let eir_info = unsafe { &*(memory_layout.eir_info as *const boot::eir::Info) };

    // let regions = unsafe {
    //     core::slice::from_raw_parts(
    //         eir_info.region_info as *const boot::eir::Region,
    //         eir_info.num_regions as usize,
    //     )
    // };

    // println!("Framebuffer address: {:#x}", eir_info.framebuffer.address);
    // println!(
    //     "Framebuffer address from PTE: {:#x}",
    //     KernelCursorPolicy::pte_page_address(
    //         KERNEL_PAGE_SPACE
    //             .cursor(eir_info.framebuffer.early_window)
    //             .read_pte()
    //     )
    // );

    // unsafe {
    //     core::arch::asm!("ud2");
    // }

    // for region in regions {
    //     let mut allocator = BuddyAllocator::new(region);

    //     let page1 = allocator.allocate(0, 64);
    //     let page2 = allocator.allocate(0, 64);

    //     println!(
    //         "Allocated pages: {:x}, {:x}",
    //         page1.unwrap_or(0),
    //         page2.unwrap_or(0)
    //     );

    //     allocator.free(page1.unwrap(), 0);

    //     let page3 = allocator.allocate(0, 64);

    //     println!("Allocated page: {:x}", page3.unwrap_or(0));

    //     allocator.free(page2.unwrap(), 0);

    //     let page4 = allocator.allocate(0, 64);

    //     println!("Allocated page: {:x}", page4.unwrap_or(0));

    //     allocator.free(page3.unwrap(), 0);
    //     allocator.free(page4.unwrap(), 0);
    // }

    // for i in 0..100 {
    //     let pixel_offset = i * eir_info.frame_buffer.fb_pitch + i * 4;
    //     let address = eir_info.frame_buffer.fb_early_window + pixel_offset;

    //     unsafe {
    //         (address as *mut u32).write_volatile(0xFFFFFFFF);
    //     }
    // }

    // let x = memory::heap::allocate_virtual_memory(0x1000);
    // let y = memory::heap::allocate_virtual_memory(0x1000);
    // let z = memory::heap::allocate_virtual_memory(0x1000);
    // let w = memory::heap::allocate_virtual_memory(0x1000);

    // println!(
    //     "Allocated virtual memory: {:#x}, {:#x}, {:#x}, {:#x}",
    //     x.unwrap(),
    //     y.unwrap(),
    //     z.unwrap(),
    //     w.unwrap()
    // );

    // memory::heap::dump_virtual_tree();

    // arch::executor::fork_executor(|frame| {
    //     let mut executor = ArchExecutor::new();
    //     executor.save(frame);
    //     println!("Executor: {:#x?}", executor);
    //     executor.restore();
    // });

    let scheduler = LOCAL_SCHEDULER.get();

    scheduler.schedule(Thread::new(thread_main, 0, 42));
    scheduler.schedule(Fiber::run(|| {
        println!("Hello from fiber!");
    }));

    if scheduler.reschedule() {
        scheduler.commit_reschedule();
    }

    panic!("Nothing to run");
}

extern "C" fn thread_main(_arg0: usize, _arg1: usize) -> ! {
    println!("Thread started with args: {}, {}", _arg0, _arg1);

    let thread = LOCAL_SCHEDULER
        .get()
        .current()
        .and_then(|entity| entity.as_thread())
        .expect("No current thread");

    println!("About to block on an async operation");

    thread.block(thread.next_block_token());

    scheduler::async_block(&thread, async {
        println!("Thread is running asynchronously");
    });

    println!("Thread finished, halting");

    loop {
        unsafe {
            core::arch::asm!("hlt");
        }
    }
}

#[panic_handler]
fn panic(info: &panic::PanicInfo) -> ! {
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
