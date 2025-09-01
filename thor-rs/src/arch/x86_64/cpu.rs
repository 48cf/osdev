use core::{
    cell::RefCell,
    mem::offset_of,
    sync::atomic::{AtomicPtr, AtomicUsize, Ordering},
};

use spin::Lazy;

use crate::{
    arch::{
        asm::Msr,
        executor::ArchExecutor,
        gdt::Gdt,
        idt::Idt,
        image::{ImageDomain, RegisterImage, SyscallRegisterImage},
    },
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

    pub(super) current_executor: AtomicPtr<ArchExecutor>,

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
            current_executor: AtomicPtr::new(core::ptr::null_mut()),
            syscall_stack_ptr: AtomicUsize::new(0),

            gdt: RefCell::new(Gdt::new()),
            kernel_stack,
        }
    }

    pub unsafe fn current_executor(&self) -> Option<&ArchExecutor> {
        let ptr = self.current_executor.load(Ordering::Relaxed);

        if !ptr.is_null() {
            Some(unsafe { &*ptr })
        } else {
            None
        }
    }

    pub fn set_syscall_stack(&self, stack: &KernelStack) {
        self.syscall_stack_ptr
            .store(stack.top() as usize, Ordering::Relaxed);
    }
}

pub fn init_early() {
    let cpu_data = get_cpu_data();
    let gs_base = Msr::IA32_GS_BASE.read();

    unsafe {
        cpu_data.arch_data().gdt.borrow().load();
        IDT.load();
    }

    Msr::IA32_GS_BASE.write(gs_base);
}

pub fn init_cpu_features() {
    let efer = Msr::IA32_EFER.read();
    Msr::IA32_EFER.write(efer | (1 << 0)); // IA32_EFER.SCE

    Msr::IA32_LSTAR.write(syscall_stub as *const () as u64);
    Msr::IA32_FMASK.write(0x200); // Disable interrupts on syscall entry

    let mut star = 0;
    star |= (Gdt::KERNEL_CODE64_SELECTOR as u64) << 32;
    star |= (Gdt::USER_DATA64_SELECTOR as u64 - 8) << 48;

    Msr::IA32_STAR.write(star);
}

pub fn setup_cpu_context(cpu_data: *const ArchCpuData) {
    unsafe {
        let arch_data = &*cpu_data;

        arch_data
            .self_ptr
            .store(cpu_data as *mut _, Ordering::Relaxed);
    }

    Msr::IA32_GS_BASE.write(cpu_data as usize as u64);
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

impl RegisterImage for SyscallRegisters {
    fn dump_registers(&self) {
        todo!("Why would you want to do that?")
    }

    fn domain(&self) -> ImageDomain {
        ImageDomain::User
    }

    fn ip(&self) -> usize {
        self.rip
    }

    fn sp(&self) -> usize {
        self.rsp
    }

    fn flags(&self) -> usize {
        self.rflags
    }

    fn set_ip(&mut self, value: usize) {
        self.rip = value;
    }

    fn set_sp(&mut self, value: usize) {
        self.rsp = value;
    }

    fn set_flags(&mut self, value: usize) {
        self.rflags = value;
    }
}

impl SyscallRegisterImage for SyscallRegisters {
    fn syscall_number(&self) -> usize {
        self.rdi
    }

    fn arg0(&self) -> usize {
        self.rsi
    }

    fn arg1(&self) -> usize {
        self.rdx
    }

    fn arg2(&self) -> usize {
        self.rax
    }

    fn arg3(&self) -> usize {
        self.r8
    }

    fn arg4(&self) -> usize {
        self.r9
    }

    fn arg5(&self) -> usize {
        self.r10
    }

    fn arg6(&self) -> usize {
        self.r12
    }

    fn arg7(&self) -> usize {
        self.r13
    }

    fn arg8(&self) -> usize {
        self.r14
    }

    fn set_error(&mut self, value: usize) {
        self.rdi = value;
    }

    fn set_out0(&mut self, value: usize) {
        self.rsi = value;
    }

    fn set_out1(&mut self, value: usize) {
        self.rdx = value;
    }
}

extern "C" fn syscall_entry(frame: *mut SyscallRegisters) {
    crate::handle_syscall(unsafe { &mut *frame });
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

#[initgraph::task("arch.x86_64.init-bsp")]
#[initgraph::depends(crate::arch::x86_64::lapic::DISCOVER_LAPIC)]
static INIT_BSP: () = || {
    
};
