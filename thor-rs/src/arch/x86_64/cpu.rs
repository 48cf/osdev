use core::{
    cell::RefCell,
    mem::offset_of,
    sync::atomic::{AtomicPtr, Ordering},
};

use spin::Lazy;

use crate::{
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
            gdt: RefCell::new(Gdt::new()),
            kernel_stack,
        }
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
