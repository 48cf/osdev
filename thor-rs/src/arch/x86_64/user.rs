use crate::{
    arch::{gdt::Gdt, tss::Tss},
    memory::stack::KernelStack,
    per_cpu::{CPU_DATA, CpuData},
};

pub struct ArchUserContext {
    tss: Tss,
    syscall_stack: KernelStack,
}

impl ArchUserContext {
    pub fn new(cpu_data: &CpuData) -> Self {
        Self {
            tss: Tss::new(&cpu_data.arch_data().kernel_stack),
            syscall_stack: KernelStack::new(),
        }
    }

    pub fn activate(&self) {
        let arch_data = CPU_DATA.get().arch_data();

        arch_data.set_syscall_stack(&self.syscall_stack);
        arch_data.gdt.borrow_mut().set_tss(&self.tss);

        // Load the TSS
        unsafe {
            core::arch::asm!("ltr ax", in("ax") Gdt::TSS_SELECTOR);
        }
    }
}
