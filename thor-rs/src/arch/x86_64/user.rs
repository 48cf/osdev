use crate::{
    arch::{gdt::Gdt, tss::Tss},
    per_cpu::{CPU_DATA, CpuData},
};

pub struct ArchUserContext {
    tss: Tss,
}

impl ArchUserContext {
    pub fn new(cpu_data: &CpuData) -> Self {
        Self {
            tss: Tss::new(&cpu_data.arch_data().kernel_stack),
        }
    }

    pub fn activate(&self) {
        CPU_DATA
            .get()
            .arch_data()
            .gdt
            .borrow_mut()
            .set_tss(&self.tss);

        // Load the TSS
        unsafe {
            core::arch::asm!("ltr ax", in("ax") Gdt::TSS_SELECTOR);
        }
    }
}
