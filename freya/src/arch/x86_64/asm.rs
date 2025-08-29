#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Msr {
    Ia32ApicBase = 0x1b,
    Ia32TscDeadline = 0x6e0,
    Ia32Efer = 0xc0000080,
    Ia32Star = 0xc0000081,
    Ia32Lstar = 0xc0000082,
    Ia32Fmask = 0xc0000084,
    Ia32FsBase = 0xc0000100,
    Ia32GsBase = 0xc0000101,
    Ia32KernelGsBase = 0xc0000102,
}

pub fn rdmsr(msr: Msr) -> u64 {
    let mut low: u32;
    let mut high: u32;

    unsafe {
        core::arch::asm!(
            "rdmsr",
            out("eax") low,
            out("edx") high,
            in("ecx") msr as u32,
        );
    }

    (high as u64) << 32 | low as u64
}

pub fn wrmsr(msr: Msr, value: u64) {
    let low = value as u32;
    let high = (value >> 32) as u32;

    unsafe {
        core::arch::asm!(
            "wrmsr",
            in("eax") low,
            in("edx") high,
            in("ecx") msr as u32,
        );
    }
}
