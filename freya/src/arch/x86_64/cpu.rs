use core::{
    mem::offset_of,
    sync::atomic::{AtomicPtr, Ordering},
};

use crate::{arch::asm, per_cpu::CpuData};

#[repr(C)]
pub struct ArchCpuData {
    self_ptr: AtomicPtr<ArchCpuData>,
}

unsafe impl Send for ArchCpuData {}
unsafe impl Sync for ArchCpuData {}

impl ArchCpuData {
    pub fn new() -> Self {
        Self {
            self_ptr: AtomicPtr::new(core::ptr::null_mut()),
        }
    }
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
