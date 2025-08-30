use core::mem::offset_of;

use crate::{arch::cpu::ArchCpuData, memory::stack::KernelStack, per_cpu::PerCpu};

#[unsafe(link_section = ".percpu.head")]
pub static CPU_DATA: PerCpu<CpuData> = PerCpu::new();

pub struct CpuData {
    arch_data: ArchCpuData,
    cpu_id: u32,
    idle_stack: KernelStack,
    detached_stack: KernelStack,
}

impl CpuData {
    pub fn new(cpu_id: u32) -> Self {
        Self {
            arch_data: ArchCpuData::new(),
            cpu_id,
            idle_stack: KernelStack::new(),
            detached_stack: KernelStack::new(),
        }
    }

    pub unsafe fn from_arch_data(arch_data: *const ArchCpuData) -> &'static Self {
        unsafe { &*((arch_data as usize - offset_of!(Self, arch_data)) as *const Self) }
    }

    pub fn arch_data(&self) -> &ArchCpuData {
        &self.arch_data
    }

    pub fn cpu_id(&self) -> u32 {
        self.cpu_id
    }

    pub fn idle_stack(&self) -> &KernelStack {
        &self.idle_stack
    }

    pub fn detached_stack(&self) -> &KernelStack {
        &self.detached_stack
    }
}
