use core::{
    arch::naked_asm,
    marker::PointeeSized,
    mem::{MaybeUninit, offset_of},
    sync::atomic::AtomicPtr,
};

use crate::{
    arch::{cpu::ArchCpuData, gdt::Gdt, interrupts::ArchInterruptFrame},
    memory::{client::UserAccessRegion, stack::KernelStack},
    scheduler::Executor,
};

#[derive(Debug)]
pub struct GeneralRegisters {
    rax: usize,
    rbx: usize,
    rcx: usize,
    rdx: usize,
    rsi: usize,
    rdi: usize,
    rbp: usize,
    r8: usize,
    r9: usize,
    r10: usize,
    r11: usize,
    r12: usize,
    r13: usize,
    r14: usize,
    r15: usize,
    rip: usize,
    cs: usize,
    rflags: usize,
    rsp: usize,
    ss: usize,
}

#[repr(C)]
#[derive(Debug)]
pub struct ArchExecutor {
    general: GeneralRegisters,
    // fp_state: *mut u8,
    pub(super) user_access_region: AtomicPtr<UserAccessRegion>,
}

impl ArchExecutor {
    pub const fn new() -> Self {
        Self {
            general: GeneralRegisters {
                rax: 0,
                rbx: 0,
                rcx: 0,
                rdx: 0,
                rsi: 0,
                rdi: 0,
                rbp: 0,
                r8: 0,
                r9: 0,
                r10: 0,
                r11: 0,
                r12: 0,
                r13: 0,
                r14: 0,
                r15: 0,
                rip: 0,
                cs: Gdt::KERNEL_CODE64_SELECTOR as usize,
                rflags: 0x202, // Enable interrupts
                rsp: 0,
                ss: Gdt::KERNEL_DATA64_SELECTOR as usize,
            },
            user_access_region: AtomicPtr::new(core::ptr::null_mut()),
            // fp_state: core::ptr::null_mut(),
        }
    }

    pub fn new_user_context(ip: usize, sp: usize, arg0: usize, arg1: usize) -> Self {
        let mut executor = Self::new();

        executor.general.rip = ip;
        executor.general.rsp = sp;
        executor.general.rdi = arg0;
        executor.general.rsi = arg1;
        executor.general.rflags = 0x202; // Enable interrupts and set the reserved bit
        executor.general.cs = Gdt::USER_CODE64_SELECTOR as usize;
        executor.general.ss = Gdt::USER_DATA64_SELECTOR as usize;

        executor
    }
}

impl Executor for ArchExecutor {
    fn user_access_region(&self) -> Option<&UserAccessRegion> {
        let ptr = self
            .user_access_region
            .load(core::sync::atomic::Ordering::Relaxed);

        if !ptr.is_null() {
            Some(unsafe { &*ptr })
        } else {
            None
        }
    }

    fn save(&mut self, frame: &ArchInterruptFrame) {
        self.general.rax = frame.rax as usize;
        self.general.rbx = frame.rbx as usize;
        self.general.rcx = frame.rcx as usize;
        self.general.rdx = frame.rdx as usize;
        self.general.rsi = frame.rsi as usize;
        self.general.rdi = frame.rdi as usize;
        self.general.rbp = frame.rbp as usize;
        self.general.r8 = frame.r8 as usize;
        self.general.r9 = frame.r9 as usize;
        self.general.r10 = frame.r10 as usize;
        self.general.r11 = frame.r11 as usize;
        self.general.r12 = frame.r12 as usize;
        self.general.r13 = frame.r13 as usize;
        self.general.r14 = frame.r14 as usize;
        self.general.r15 = frame.r15 as usize;
        self.general.rip = frame.iret.rip as usize;
        self.general.cs = frame.iret.cs as usize;
        self.general.rflags = frame.iret.rflags as usize;
        self.general.rsp = frame.iret.rsp as usize;
        self.general.ss = frame.iret.ss as usize;
    }

    fn restore(&self) -> ! {
        load_executor(self);
    }

    fn ip(&mut self) -> &mut usize {
        &mut self.general.rip
    }

    fn sp(&mut self) -> &mut usize {
        &mut self.general.rsp
    }

    fn arg0(&mut self) -> &mut usize {
        &mut self.general.rdi
    }

    fn arg1(&mut self) -> &mut usize {
        &mut self.general.rsi
    }

    fn result0(&mut self) -> &mut usize {
        &mut self.general.rax
    }

    fn result1(&mut self) -> &mut usize {
        &mut self.general.rdx
    }
}

pub fn fork_executor<F: FnMut(&ArchInterruptFrame) -> !>(func: F) {
    extern "C" fn fork_executor_entry<F: FnMut(&ArchInterruptFrame) -> ! + Sized + PointeeSized>(
        frame: *const ArchInterruptFrame,
        arg: usize,
    ) -> ! {
        let func: &mut F = unsafe { &mut *(arg as *mut F) };

        func(unsafe { &*frame });
    }

    do_fork_executor(
        fork_executor_entry::<F> as usize,
        &func as *const _ as usize,
    );
}

pub fn run_on_stack<F: FnMut(usize) -> !>(stack: &KernelStack, func: F) -> ! {
    extern "C" fn run_on_stack_entry<F: FnMut(usize) -> ! + Sized + PointeeSized>(
        previous_sp: usize,
        arg: usize,
    ) -> ! {
        let func: &mut F = unsafe { &mut *(arg as *mut F) };

        func(previous_sp)
    }

    let mut top = stack.top() as usize;

    top -= (size_of::<F>() + 0xF) & !0xF;

    let ptr = top as *mut MaybeUninit<F>;

    unsafe {
        (*ptr).write(func);
    }

    do_run_on_stack(top, run_on_stack_entry::<F> as usize, ptr as usize);
}

#[unsafe(naked)]
extern "C" fn save_executor(general: *mut GeneralRegisters) {
    naked_asm!(
        "mov [rdi + {rax}], rax",
        "mov [rdi + {rbx}], rbx",
        "mov [rdi + {rcx}], rcx",
        "mov [rdi + {rdx}], rdx",
        "mov [rdi + {rsi}], rsi",
        "mov [rdi + {rdi}], rdi",
        "mov [rdi + {rbp}], rbp",
        "mov [rdi + {r8}], r8",
        "mov [rdi + {r9}], r9",
        "mov [rdi + {r10}], r10",
        "mov [rdi + {r11}], r11",
        "mov [rdi + {r12}], r12",
        "mov [rdi + {r13}], r13",
        "mov [rdi + {r14}], r14",
        "mov [rdi + {r15}], r15",
        "mov [rdi + {rip}], rip",
        "mov [rdi + {cs}], cs",
        "mov [rdi + {rsp}], rsp",
        "mov [rdi + {ss}], ss",

        "pushfq",
        "pop [rdi + {rflags}]",

        "ret",

        rax = const offset_of!(GeneralRegisters, rax),
        rbx = const offset_of!(GeneralRegisters, rbx),
        rcx = const offset_of!(GeneralRegisters, rcx),
        rdx = const offset_of!(GeneralRegisters, rdx),
        rsi = const offset_of!(GeneralRegisters, rsi),
        rdi = const offset_of!(GeneralRegisters, rdi),
        rbp = const offset_of!(GeneralRegisters, rbp),
        r8 = const offset_of!(GeneralRegisters, r8),
        r9 = const offset_of!(GeneralRegisters, r9),
        r10 = const offset_of!(GeneralRegisters, r10),
        r11 = const offset_of!(GeneralRegisters, r11),
        r12 = const offset_of!(GeneralRegisters, r12),
        r13 = const offset_of!(GeneralRegisters, r13),
        r14 = const offset_of!(GeneralRegisters, r14),
        r15 = const offset_of!(GeneralRegisters, r15),
        rip = const offset_of!(GeneralRegisters, rip),
        cs = const offset_of!(GeneralRegisters, cs),
        rflags = const offset_of!(GeneralRegisters, rflags),
        rsp = const offset_of!(GeneralRegisters, rsp),
        ss = const offset_of!(GeneralRegisters, ss),
    );
}

#[unsafe(naked)]
extern "C" fn load_executor(general: *const ArchExecutor) -> ! {
    naked_asm!(
        "mov gs:[{active_executor}], rdi",

        "mov rax, [rdi + {rax}]",
        "mov rbx, [rdi + {rbx}]",
        "mov rcx, [rdi + {rcx}]",
        "mov rdx, [rdi + {rdx}]",
        "mov rsi, [rdi + {rsi}]",
        "mov rbp, [rdi + {rbp}]",
        "mov r8, [rdi + {r8}]",
        "mov r9, [rdi + {r9}]",
        "mov r10, [rdi + {r10}]",
        "mov r11, [rdi + {r11}]",
        "mov r12, [rdi + {r12}]",
        "mov r13, [rdi + {r13}]",
        "mov r14, [rdi + {r14}]",
        "mov r15, [rdi + {r15}]",

        "push [rdi + {ss}]",
        "push [rdi + {rsp}]",
        "push [rdi + {rflags}]",
        "push [rdi + {cs}]",
        "push [rdi + {rip}]",

        "cmp qword ptr [rdi + {cs}], {kernel_cs}",
        "je 2f",
        "swapgs",
        "2:",

        "mov rdi, [rdi + {rdi}]",
        "iretq",

        active_executor = const offset_of!(ArchCpuData, current_executor),

        rax = const offset_of!(ArchExecutor, general.rax),
        rbx = const offset_of!(ArchExecutor, general.rbx),
        rcx = const offset_of!(ArchExecutor, general.rcx),
        rdx = const offset_of!(ArchExecutor, general.rdx),
        rsi = const offset_of!(ArchExecutor, general.rsi),
        rdi = const offset_of!(ArchExecutor, general.rdi),
        rbp = const offset_of!(ArchExecutor, general.rbp),
        r8 = const offset_of!(ArchExecutor, general.r8),
        r9 = const offset_of!(ArchExecutor, general.r9),
        r10 = const offset_of!(ArchExecutor, general.r10),
        r11 = const offset_of!(ArchExecutor, general.r11),
        r12 = const offset_of!(ArchExecutor, general.r12),
        r13 = const offset_of!(ArchExecutor, general.r13),
        r14 = const offset_of!(ArchExecutor, general.r14),
        r15 = const offset_of!(ArchExecutor, general.r15),
        rip = const offset_of!(ArchExecutor, general.rip),
        cs = const offset_of!(ArchExecutor, general.cs),
        rflags = const offset_of!(ArchExecutor, general.rflags),
        rsp = const offset_of!(ArchExecutor, general.rsp),
        ss = const offset_of!(ArchExecutor, general.ss),

        kernel_cs = const Gdt::KERNEL_CODE64_SELECTOR as u16,
    );
}

extern "C" fn do_fork_executor(entry: usize, arg: usize) {
    let mut frame: ArchInterruptFrame = unsafe { MaybeUninit::zeroed().assume_init() };

    frame.iret.cs = Gdt::KERNEL_CODE64_SELECTOR as usize;
    frame.iret.ss = Gdt::KERNEL_DATA64_SELECTOR as usize;

    unsafe {
        core::arch::asm!(
            "mov [{frame} + {rbx}], rbx",
            "mov [{frame} + {rbp}], rbp",
            "mov [{frame} + {r12}], r12",
            "mov [{frame} + {r13}], r13",
            "mov [{frame} + {r14}], r14",
            "mov [{frame} + {r15}], r15",
            "mov [{frame} + {rsp}], rsp",

            "lea rax, [rip + 2f]",
            "mov [{frame} + {rip}], rax",

            "pushfq",
            "pop [{frame} + {rflags}]",

            "mov rdi, {frame}",
            "mov rsi, {arg}",
            "call {entry}",
            "ud2",

            "2:",

            out("rax") _,
            out("rdi") _,
            out("rsi") _,
            out("rdx") _,
            out("rcx") _,
            out("r8") _,
            out("r9") _,
            out("r10") _,
            out("r11") _,

            frame = in(reg) &raw mut frame,
            arg = in(reg)  arg,
            entry = in(reg) entry,

            rbx = const offset_of!(ArchInterruptFrame, rbx),
            rbp = const offset_of!(ArchInterruptFrame, rbp),
            r12 = const offset_of!(ArchInterruptFrame, r12),
            r13 = const offset_of!(ArchInterruptFrame, r13),
            r14 = const offset_of!(ArchInterruptFrame, r14),
            r15 = const offset_of!(ArchInterruptFrame, r15),
            rflags = const offset_of!(ArchInterruptFrame, iret.rflags),
            rsp = const offset_of!(ArchInterruptFrame, iret.rsp),
            rip = const offset_of!(ArchInterruptFrame, iret.rip),
        );
    }
}

#[unsafe(naked)]
extern "C" fn do_run_on_stack(rsp: usize, func: usize, arg: usize) -> ! {
    naked_asm!(
        "xor ebp, ebp",
        "mov rcx, rsp",
        "mov rsp, rdi",
        "mov rdi, rcx",
        "mov rcx, rsi",
        "mov rsi, rdx",
        "call rcx",
        "ud2",
    );
}
