mod cpu_data;

pub use cpu_data::{CPU_DATA, CpuData};

use core::mem::MaybeUninit;

use crate::{arch, boot::elf_note::PerCpuRegion, elf_note};

#[macro_export]
macro_rules! define_percpu {
    ($vis:vis static $name:ident : $ty:ty = $value:expr;) => {
        #[unsafe(link_section = ".percpu.tail")]
        $vis static $name: $crate::per_cpu::PerCpu<$ty> =
            $crate::per_cpu::PerCpu::new();

        const _: () = {
            #[used]
            #[doc(hidden)]
            #[unsafe(link_section = ".percpu.init")]
            static __INIT: fn(&$crate::per_cpu::CpuData) = |cpu_data| {
                unsafe {
                    let ptr = $name.as_mut_ptr(cpu_data);
                    (*ptr).write($value);
                }
            };
        };
    };
}

unsafe extern "C" {
    static LD_PERCPU_INIT_FNS_START: u8;
    static LD_PERCPU_INIT_FNS_END: u8;

    static LD_PERCPU_START: u8;
    static LD_PERCPU_END: u8;
}

elf_note! {
    pub static PER_CPU_REGION_NOTE: PerCpuRegion =
        PerCpuRegion::new(unsafe { &LD_PERCPU_START }, unsafe { &LD_PERCPU_END });
}

#[repr(transparent)]
pub struct PerCpu<T: Sized> {
    storage: MaybeUninit<T>,
}

impl<T: Sized> PerCpu<T> {
    pub const fn new() -> Self {
        Self {
            storage: MaybeUninit::zeroed(),
        }
    }

    pub fn get(&self) -> &'static T {
        self.get_for_cpu(arch::cpu::get_cpu_data())
    }

    pub fn get_for_cpu(&self, cpu_data: &CpuData) -> &'static T {
        unsafe {
            let ptr = self.as_ptr(cpu_data);
            (*ptr).assume_init_ref()
        }
    }

    pub fn get_for_cpu_by_id(&self, cpu_id: usize) -> &'static T {
        let size = &raw const LD_PERCPU_END as usize - &raw const LD_PERCPU_START as usize;
        let ptr = (&raw const LD_PERCPU_START as usize + self.offset() + cpu_id * size)
            as *const MaybeUninit<T>;

        unsafe { (*ptr).assume_init_ref() }
    }

    pub unsafe fn as_ptr(&self, cpu_data: &CpuData) -> *const MaybeUninit<T> {
        (cpu_data as *const _ as usize + self.offset()) as *const _
    }

    pub unsafe fn as_mut_ptr(&self, cpu_data: &CpuData) -> *mut MaybeUninit<T> {
        (cpu_data as *const _ as usize + self.offset()) as *mut _
    }

    fn offset(&self) -> usize {
        &raw const self.storage as usize - &raw const LD_PERCPU_START as usize
    }
}

fn init_for_cpu(cpu_data: &CpuData) {
    type PerCpuInitializer = fn(&CpuData);

    let init_fns_start = &raw const LD_PERCPU_INIT_FNS_START as *const PerCpuInitializer;
    let init_fns_end = &raw const LD_PERCPU_INIT_FNS_END as *const PerCpuInitializer;
    let init_fns = unsafe {
        core::slice::from_raw_parts(
            init_fns_start,
            init_fns_end.offset_from_unsigned(init_fns_start),
        )
    };

    for func in init_fns {
        func(cpu_data);
    }
}

pub fn init_for_boot_processor() {
    let ptr = &raw const CPU_DATA as *mut MaybeUninit<CpuData>;
    let cpu_data = unsafe { (*ptr).write(CpuData::new(0)) };

    arch::cpu::setup_cpu_context(cpu_data.arch_data());
    arch::cpu::init_early();

    init_for_cpu(cpu_data);
}
