use crate::arch::asm::Msr;

pub fn get_clock_nanos() -> u64 {
    let mut tsc_low: u32;
    let mut tsc_high: u32;

    unsafe {
        core::arch::asm!(
            "rdtsc",
            out("eax") tsc_low,
            out("edx") tsc_high
        );
    }

    (tsc_high as u64) << 32 | tsc_low as u64
}

pub fn set_deadline(deadline: u64) {
    Msr::IA32_TSC_DEADLINE.write(deadline);
}

pub fn clear_deadline() {
    Msr::IA32_TSC_DEADLINE.write(0);
}
