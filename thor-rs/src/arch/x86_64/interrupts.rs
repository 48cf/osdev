use core::mem::offset_of;

use crate::{
    arch::{
        gdt::Gdt,
        idt::{Idt, IdtEntry},
        image::{
            FaultErrorCode, FaultKind, FaultRegisterImage, ImageDomain, IrqRegisterImage,
            RegisterImage,
        },
    },
    memory::PageAccess,
};

pub fn setup_idt(idt: &mut Idt) {
    seq_macro::seq! {
        N in 0..256 {
            let flags = if N == 3 {
                IdtEntry::PRESENT | IdtEntry::DPL3 | IdtEntry::TRAP_GATE
            } else {
                IdtEntry::PRESENT | IdtEntry::INTERRUPT_GATE
            };

            idt.set_handler(N, kernel_interrupt_stub_~N, Gdt::KERNEL_CODE64_SELECTOR, 0, flags);
        }
    }
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy)]
pub struct ErrorCode(pub usize);

impl FaultErrorCode for ErrorCode {
    fn is_user(&self) -> bool {
        self.0 & (1 << 2) != 0
    }

    fn into_page_access(self) -> PageAccess {
        if self.0 & (1 << 1) != 0 {
            PageAccess::WRITE
        } else if self.0 & (1 << 4) != 0 {
            PageAccess::EXECUTE
        } else {
            PageAccess::READ
        }
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct IretFrame {
    // Pushed onto the stack by the interrupt handler stubs.
    pub int: usize,
    // Pushed onto the stack by the CPU if the interrupt has an error code.
    pub error: ErrorCode,
    // The rest is pushed onto the stack by the CPU during an interrupt.
    pub rip: usize,
    pub cs: usize,
    pub rflags: usize,
    pub rsp: usize,
    pub ss: usize,
}

#[repr(C)]
#[derive(Debug)]
pub struct ArchInterruptFrame {
    pub cr2: usize,
    pub r15: usize,
    pub r14: usize,
    pub r13: usize,
    pub r12: usize,
    pub r11: usize,
    pub r10: usize,
    pub r9: usize,
    pub r8: usize,
    pub rsi: usize,
    pub rdi: usize,
    pub rbp: usize,
    pub rdx: usize,
    pub rcx: usize,
    pub rbx: usize,
    pub rax: usize,
    pub iret: IretFrame,
}

impl RegisterImage for ArchInterruptFrame {
    fn dump_registers(&self) {
        crate::println!("Register state:");
        crate::println!(
            "  RAX: {:#018x}  RBX: {:#018x}  RCX: {:#018x}",
            self.rax,
            self.rbx,
            self.rcx
        );
        crate::println!(
            "  RDX: {:#018x}  RDI: {:#018x}  RSI: {:#018x}",
            self.rdx,
            self.rdi,
            self.rsi
        );
        crate::println!(
            "  R8:  {:#018x}  R9:  {:#018x}  R10: {:#018x}",
            self.r8,
            self.r9,
            self.r10
        );
        crate::println!(
            "  R11: {:#018x}  R12: {:#018x}  R13: {:#018x}",
            self.r11,
            self.r12,
            self.r13
        );
        crate::println!("  R14: {:#018x}  R15: {:#018x}", self.r14, self.r15);
        crate::println!("  Error code: {:#x}", self.iret.error.0);
        crate::println!("  RIP: {:#x}", self.iret.rip);
    }

    fn domain(&self) -> ImageDomain {
        if self.iret.cs & 0b11 == 3 {
            ImageDomain::User
        } else {
            ImageDomain::Kernel
        }
    }

    fn ip(&self) -> usize {
        self.iret.rip
    }

    fn sp(&self) -> usize {
        self.iret.rsp
    }

    fn flags(&self) -> usize {
        self.iret.rflags
    }

    fn set_ip(&mut self, value: usize) {
        self.iret.rip = value;
    }

    fn set_sp(&mut self, value: usize) {
        self.iret.rsp = value;
    }

    fn set_flags(&mut self, value: usize) {
        self.iret.rflags = value;
    }
}

impl IrqRegisterImage for ArchInterruptFrame {
    fn irq_number(&self) -> usize {
        self.iret.int
    }
}

impl FaultRegisterImage for ArchInterruptFrame {
    fn fault_kind(&self) -> FaultKind {
        match self.iret.int {
            3 => FaultKind::Breakpoint,
            6 => FaultKind::InvalidOpcode,
            14 => FaultKind::PageFault,
            other => FaultKind::Other(other),
        }
    }

    fn error_code(&self) -> impl FaultErrorCode {
        self.iret.error
    }

    fn fault_address(&self) -> usize {
        self.cr2
    }
}

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
        // Save fault address.
        "mov rax, cr2",
        "push rax",
        // Clear direction flag.
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
        // Skip the fault address.
        "add rsp, 8",
        // Pop the general registers.
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

extern "C" fn kernel_interrupt_handler(frame: *mut ArchInterruptFrame) {
    let frame = unsafe { &mut *frame };

    if frame.irq_number() < 32 {
        crate::handle_fault(frame);
    } else {
        crate::handle_interrupt(frame);
    }
}
