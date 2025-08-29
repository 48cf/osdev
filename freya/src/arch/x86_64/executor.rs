use core::{
    arch::naked_asm,
    mem::{MaybeUninit, offset_of},
};

use crate::{InterruptFrame, arch::gdt::Gdt, memory::stack::KernelStack, scheduler::Executor};

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

#[derive(Debug)]
pub struct ArchExecutor {
    general: GeneralRegisters,
    // fp_state: *mut u8,
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
                cs: 0x8,
                rflags: 0x200, // Enable interrupts
                rsp: 0,
                ss: 0x10,
            },
            // fp_state: core::ptr::null_mut(),
        }
    }
}

impl Executor for ArchExecutor {
    fn save(&mut self, frame: &InterruptFrame) {
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
        self.general.rip = frame.rip as usize;
        self.general.cs = frame.cs as usize;
        self.general.rflags = frame.rflags as usize;
        self.general.rsp = frame.rsp as usize;
        self.general.ss = frame.ss as usize;
    }

    fn restore(&self) -> ! {
        load_executor(&self.general);
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

extern "C" fn fork_executor_entry<F: FnMut(&InterruptFrame) -> !>(
    frame: *const InterruptFrame,
    arg: usize,
) -> ! {
    crate::println!("Frame: {:#016x?}", unsafe { &*frame });

    let func: &mut F = unsafe { &mut *(arg as *mut F) };

    func(unsafe { &*frame });
}

extern "C" fn run_on_stack_entry<F: FnMut(usize) -> !>(previous_sp: usize, arg: usize) -> ! {
    let func: &mut F = unsafe { &mut *(arg as *mut F) };

    func(previous_sp)
}

pub fn fork_executor<F: FnMut(&InterruptFrame) -> !>(func: F) {
    do_fork_executor(
        fork_executor_entry::<F> as usize,
        &func as *const _ as usize,
    );
}

pub fn run_on_stack<F: FnMut(usize) -> !>(stack: &KernelStack, func: F) -> ! {
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
extern "C" fn load_executor(general: *const GeneralRegisters) -> ! {
    naked_asm!(
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

        "mov rdi, [rdi + {rdi}]",
        "iretq",

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
extern "C" fn do_fork_executor(func: usize, arg: usize) {
    naked_asm!(
        // Make enough space on the stack to store the fake interrupt frame
        "sub rsp, {interrupt_frame_size}",
        // Save general registers
        "mov [rsp + {r15}], r15",
        "mov [rsp + {r14}], r14",
        "mov [rsp + {r13}], r13",
        "mov [rsp + {r12}], r12",
        "mov [rsp + {r11}], r11",
        "mov [rsp + {r10}], r10",
        "mov [rsp + {r9}], r9",
        "mov [rsp + {r8}], r8",
        "mov [rsp + {rsi}], rsi",
        "mov [rsp + {rdi}], rdi",
        "mov [rsp + {rbp}], rbp",
        "mov [rsp + {rdx}], rdx",
        "mov [rsp + {rcx}], rcx",
        "mov [rsp + {rbx}], rbx",
        "mov [rsp + {rax}], rax",
        // Clear out interrupt number and error code
        "mov qword ptr [rsp + {interrupt_number}], 0",
        "mov qword ptr [rsp + {error}], 0",
        // Load the return address from the stack
        "mov rax, [rsp + {interrupt_frame_size}]",
        "mov [rsp + {rip}], rax",
        // Load the code segment selector
        "mov qword ptr [rsp + {cs}], {kernel_code64}",
        "mov qword ptr [rsp + {rflags}], 0x200",
        // Load the flags
        // "pushfq",
        // "pop [rsp + {rflags} - 0x8]",
        // Load the stack pointer
        "lea rax, [rsp + {interrupt_frame_size}]",
        "mov [rsp + {rsp}], rax",
        // Load the stack segment selector
        "mov qword ptr [rsp + {ss}], {kernel_data64}",
        // Call the provided function
        "mov rcx, rdi",
        "mov rdi, rsp",
        "call rcx",
        "ud2",

        interrupt_frame_size = const size_of::<InterruptFrame>(),

        ss = const offset_of!(InterruptFrame, ss),
        rsp = const offset_of!(InterruptFrame, rsp),
        r15 = const offset_of!(InterruptFrame, r15),
        r14 = const offset_of!(InterruptFrame, r14),
        r13 = const offset_of!(InterruptFrame, r13),
        r12 = const offset_of!(InterruptFrame, r12),
        r11 = const offset_of!(InterruptFrame, r11),
        r10 = const offset_of!(InterruptFrame, r10),
        r9 = const offset_of!(InterruptFrame, r9),
        r8 = const offset_of!(InterruptFrame, r8),
        rsi = const offset_of!(InterruptFrame, rsi),
        rdi = const offset_of!(InterruptFrame, rdi),
        rbp = const offset_of!(InterruptFrame, rbp),
        rdx = const offset_of!(InterruptFrame, rdx),
        rcx = const offset_of!(InterruptFrame, rcx),
        rbx = const offset_of!(InterruptFrame, rbx),
        rax = const offset_of!(InterruptFrame, rax),
        interrupt_number = const offset_of!(InterruptFrame, interrupt_number),
        error = const offset_of!(InterruptFrame, error),
        rip = const offset_of!(InterruptFrame, rip),
        cs = const offset_of!(InterruptFrame, cs),
        rflags = const offset_of!(InterruptFrame, rflags),

        kernel_code64 = const Gdt::KERNEL_CODE64_SELECTOR,
        kernel_data64 = const Gdt::KERNEL_DATA64_SELECTOR,
    );
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
