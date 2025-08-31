use core::{
    cell::RefCell,
    mem::offset_of,
    sync::atomic::{AtomicPtr, AtomicUsize, Ordering},
};

use spin::Lazy;

use crate::{
    KernelError,
    arch::{asm, gdt::Gdt, idt::Idt},
    memory::stack::KernelStack,
    per_cpu::CpuData,
};

static IDT: Lazy<Idt> = Lazy::new(|| {
    let mut idt = Idt::new();

    crate::arch::interrupts::setup_idt(&mut idt);

    idt
});

#[repr(C)]
pub struct ArchCpuData {
    self_ptr: AtomicPtr<ArchCpuData>,
    syscall_stack_ptr: AtomicUsize,

    // TODO: Not make those public
    pub gdt: RefCell<Gdt>,
    pub kernel_stack: KernelStack,
}

unsafe impl Send for ArchCpuData {}
unsafe impl Sync for ArchCpuData {}

impl ArchCpuData {
    pub fn new() -> Self {
        let kernel_stack = KernelStack::new();

        Self {
            self_ptr: AtomicPtr::new(core::ptr::null_mut()),
            syscall_stack_ptr: AtomicUsize::new(0),

            gdt: RefCell::new(Gdt::new()),
            kernel_stack,
        }
    }

    pub fn set_syscall_stack(&self, stack: &KernelStack) {
        self.syscall_stack_ptr
            .store(stack.top() as usize, Ordering::Relaxed);
    }
}

pub fn init_early() {
    let cpu_data = get_cpu_data();
    let gs_base = asm::rdmsr(asm::Msr::Ia32GsBase);

    unsafe {
        cpu_data.arch_data().gdt.borrow().load();
        IDT.load();
    }

    asm::wrmsr(asm::Msr::Ia32GsBase, gs_base);
}

pub fn init_cpu_features() {
    let efer = asm::rdmsr(asm::Msr::Ia32Efer);
    asm::wrmsr(asm::Msr::Ia32Efer, efer | (1 << 0)); // IA32_EFER.SCE

    asm::wrmsr(asm::Msr::Ia32Lstar, syscall_stub as *const () as u64);
    asm::wrmsr(asm::Msr::Ia32Fmask, 0x200); // Disable interrupts on syscall entry

    let mut star = 0_u64;
    star |= (Gdt::KERNEL_CODE64_SELECTOR as u64) << 32;
    star |= (Gdt::USER_DATA64_SELECTOR as u64 - 8) << 48;

    asm::wrmsr(asm::Msr::Ia32Star, star);
}

pub fn setup_cpu_context(cpu_data: *const ArchCpuData) {
    unsafe {
        let arch_data = &*cpu_data;

        arch_data
            .self_ptr
            .store(cpu_data as *mut _, Ordering::Relaxed);
    }

    asm::wrmsr(asm::Msr::Ia32GsBase, cpu_data as usize as u64);
}

pub fn get_cpu_data() -> &'static CpuData {
    let mut arch_data: *const ArchCpuData;

    unsafe {
        core::arch::asm!(
            "mov {}, gs:[{self_ptr}]",
            out(reg) arch_data,
            self_ptr = const offset_of!(ArchCpuData, self_ptr)
        );
    }

    unsafe { CpuData::from_arch_data(arch_data) }
}

#[repr(C)]
#[derive(Debug)]
struct SyscallRegisters {
    rax: usize,
    rdx: usize,
    rsi: usize,
    rdi: usize,
    rbp: usize,
    r8: usize,
    r9: usize,
    r10: usize,
    r12: usize,
    r13: usize,
    r14: usize,
    r15: usize,
    rip: usize,
    rflags: usize,
    rsp: usize,
}

extern "C" fn syscall_entry(frame: *mut SyscallRegisters) {
    let frame = unsafe { &mut *frame };
    let result = crate::handle_syscall(
        frame.rdi, frame.rsi, frame.rdx, frame.rax, frame.r8, frame.r9, frame.r10, frame.r12,
        frame.r13, frame.r14,
    );

    match result {
        Ok((a, b)) => {
            frame.rdi = hel_sys::kHelErrNone as usize;
            frame.rsi = a;
            frame.rdx = b;
        }
        Err(err) => {
            frame.rdi = match err {
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
            } as usize;
        }
    }
}

#[unsafe(naked)]
extern "C" fn syscall_stub() {
    core::arch::naked_asm!(
        "swapgs",

        // Save and switch to the syscall stack, userspace should preserve RBX.
        "mov rbx, rsp",
        "mov rsp, gs:[{syscall_stack_ptr}]",

        // Push stack pointer
        "push rbx",
        // Push RFLAGS (R11) and RIP (RCX).
        "push r11",
        "push rcx",
        // Push general registers.
        "push r15",
        "push r14",
        "push r13",
        "push r12",
        "push r10",
        "push r9",
        "push r8",
        "push rbp",
        "push rdi",
        "push rsi",
        "push rdx",
        "push rax",

        // Call the syscall entry point.
        "xor ebp, ebp",
        "mov rdi, rsp",
        "call {syscall_entry}",

        // Restore registers.
        "pop rax",
        "pop rdx",
        "pop rsi",
        "pop rdi",
        "pop rbp",
        "pop r8",
        "pop r9",
        "pop r10",
        "pop r12",
        "pop r13",
        "pop r14",
        "pop r15",
        // Restore RIP (RCX) and RFLAGS (R11).
        "pop rcx",
        "pop r11",
        // Restore stack pointer.
        "pop rbx",
        "mov rsp, rbx",

        // Return to userspace.
        "swapgs",
        "sysretq",

        syscall_stack_ptr = const offset_of!(ArchCpuData, syscall_stack_ptr),
        syscall_entry = sym syscall_entry,
    );
}
