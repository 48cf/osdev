use core::sync::atomic::{AtomicBool, AtomicPtr, AtomicU64, Ordering};

use raw_cpuid::CpuId;

use crate::{
    arch::asm::Msr,
    memory::{self, CachingMode, PageAccess, kernel::KERNEL_PAGE_SPACE},
};

crate::define_percpu! {
    pub(super) static LAPIC_CONTEXT: LocalApicContext = LocalApicContext::new();
}

pub struct FrequencyFraction {
    f: u64,
    s: u32,
}

impl FrequencyFraction {
    pub const fn new() -> Self {
        Self { f: 0, s: 0 }
    }

    pub fn compute_fraction(value: u64, denom: u64) -> Self {
        let s = 63 - (u64::BITS - value.leading_zeros());
        let f = (value << s) / denom;

        Self { f, s }
    }
}

impl core::ops::Mul<u64> for FrequencyFraction {
    type Output = u64;

    fn mul(self, rhs: u64) -> Self::Output {
        let product = (self.f as u128 * rhs as u128) >> self.s;

        if product >> u64::BITS != 0 {
            u64::MAX
        } else {
            product as u64
        }
    }
}

pub struct LocalApicContext {
    is_calibrated: bool,
    uses_tsc_deadline: bool,
    timer_frequency: FrequencyFraction,
}

impl LocalApicContext {
    const fn new() -> Self {
        Self {
            is_calibrated: false,
            uses_tsc_deadline: false,
            timer_frequency: FrequencyFraction::new(),
        }
    }
}

static LAPIC_BASE: AtomicPtr<u8> = AtomicPtr::new(core::ptr::null_mut());
static LAPIC_IS_X2APIC: AtomicBool = AtomicBool::new(false);

const IA32_APIC_BASE_EXTD: u64 = 1 << 10;
const IA32_APIC_BASE_EN: u64 = 1 << 11;

#[initgraph::task("arch.x86_64.discover-lapic")]
#[initgraph::entails(crate::arch::x86_64::TIMER_AVAILABLE)]
pub(super) static DISCOVER_LAPIC: () = || {
    let cpuid = CpuId::new();
    let feature_info = cpuid.get_feature_info().expect("No feature info available");
    let x2apic_available = feature_info.has_x2apic();

    let mut msr = Msr::IA32_APIC_BASE.read();
    msr |= IA32_APIC_BASE_EN;

    let lapic_physical = msr & !0xFFF;
    crate::println!("x86_64: Local APIC at {:#x}", lapic_physical);

    if x2apic_available {
        msr |= IA32_APIC_BASE_EXTD;
        LAPIC_IS_X2APIC.store(true, Ordering::Relaxed);

        crate::println!("x86_64: Using x2APIC");
    } else {
        let lapic_virtual =
            memory::heap::allocate_virtual_memory(0x1000).expect("Out of virtual memory");

        KERNEL_PAGE_SPACE.cursor(lapic_virtual).map_page(
            msr & !0xFFF,
            PageAccess::READ_WRITE,
            CachingMode::Uncached,
        );

        LAPIC_BASE.store(lapic_virtual as *mut u8, Ordering::Relaxed);
    }

    Msr::IA32_APIC_BASE.write(msr);
};
