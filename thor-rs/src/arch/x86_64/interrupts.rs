use core::mem::offset_of;

use crate::{
    arch::{gdt::Gdt, idt::Idt},
    memory::PageAccess,
};

pub fn setup_idt(idt: &mut Idt) {
    seq_macro::seq! {
        N in 0..256 {
            idt.set_handler(N, kernel_interrupt_stub_~N, Gdt::KERNEL_CODE64_SELECTOR, 0, 0x8E);
        }
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct IretFrame {
    // Pushed onto the stack by the interrupt handler stubs.
    pub int: u64,
    // Pushed onto the stack by the CPU if the interrupt has an error code.
    pub error: u64,
    // The rest is pushed onto the stack by the CPU during an interrupt.
    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
    pub rsp: u64,
    pub ss: u64,
}

#[repr(C)]
#[derive(Debug)]
pub struct ArchInterruptFrame {
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
    pub iret: IretFrame,
}

const _: () = {
    // Make sure the interrupt frame is 16-byte aligned.
    assert!(size_of::<ArchInterruptFrame>() % 16 == 0);
};

seq_macro::seq! {
    N in 0..256 {
        #[unsafe(naked)]
        extern "C" fn kernel_interrupt_stub_~N() {
            core::arch::naked_asm!(
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
    core::arch::naked_asm!(
        // If we are not coming from kernel mode, swap the GS base.
        "cmp qword ptr [rsp + {cs}], {kernel_cs}",
        "je 2f",
        "swapgs",
        "2:",
        // Save general registers.
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
        kernel_interrupt_stub_return = sym kernel_interrupt_stub_return,

        cs = const offset_of!(IretFrame, cs),
        kernel_cs = const Gdt::KERNEL_CODE64_SELECTOR as u16,
    );
}

#[unsafe(naked)]
extern "C" fn kernel_interrupt_stub_return() {
    core::arch::naked_asm!(
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
        // If we are not returning to kernel mode, swap the GS base.
        "cmp qword ptr [rsp + {cs}], {kernel_cs}",
        "je 2f",
        "swapgs",
        "2:",
        // Skip `error` and `interrupt_number` fields.
        "add rsp, 0x10",
        "iretq",

        cs = const offset_of!(IretFrame, cs),
        kernel_cs = const Gdt::KERNEL_CODE64_SELECTOR as u16,
    );
}

extern "C" fn kernel_interrupt_handler(frame: &mut ArchInterruptFrame) {
    if frame.iret.int == 14 && frame.iret.cs & 0x3 == 3 {
        let mut cr2: u64;

        unsafe {
            core::arch::asm!("mov {}, cr2", out(reg) cr2);
        }

        let fault_access = if frame.iret.error & (1 << 1) != 0 {
            PageAccess::WRITE
        } else if frame.iret.error & (1 << 4) != 0 {
            PageAccess::EXECUTE
        } else {
            PageAccess::READ
        };

        if crate::handle_page_fault(cr2, fault_access) {
            return;
        }
    }

    crate::println!("Exception: {}", frame.iret.int);
    crate::println!("Register state:");
    crate::println!(
        "  RAX: {:#018x}  RBX: {:#018x}  RCX: {:#018x}",
        frame.rax,
        frame.rbx,
        frame.rcx
    );
    crate::println!(
        "  RDX: {:#018x}  RDI: {:#018x}  RSI: {:#018x}",
        frame.rdx,
        frame.rdi,
        frame.rsi
    );
    crate::println!(
        "  R8:  {:#018x}  R9:  {:#018x}  R10: {:#018x}",
        frame.r8,
        frame.r9,
        frame.r10
    );
    crate::println!(
        "  R11: {:#018x}  R12: {:#018x}  R13: {:#018x}",
        frame.r11,
        frame.r12,
        frame.r13
    );
    crate::println!("  R14: {:#018x}  R15: {:#018x}", frame.r14, frame.r15);
    crate::println!("  Error code: {:#x}", frame.iret.error);
    crate::println!("  RIP: {:#x}", frame.iret.rip);

    if frame.iret.int == 14 {
        let mut cr2: u64;

        unsafe {
            core::arch::asm!("mov {}, cr2", out(reg) cr2);
        }

        crate::println!("  Faulting address: {:#x}", cr2);
    }

    loop {
        unsafe {
            core::arch::asm!("hlt");
        }
    }
}
