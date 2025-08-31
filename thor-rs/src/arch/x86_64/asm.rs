#[derive(Clone, Copy)]
pub struct Msr(u32);

impl Msr {
    pub const IA32_APIC_BASE: Self = Self(0x1b);
    pub const IA32_TSC_DEADLINE: Self = Self(0x6e0);
    pub const IA32_EFER: Self = Self(0xc0000080);
    pub const IA32_STAR: Self = Self(0xc0000081);
    pub const IA32_LSTAR: Self = Self(0xc0000082);
    pub const IA32_FMASK: Self = Self(0xc0000084);
    pub const IA32_FS_BASE: Self = Self(0xc0000100);
    pub const IA32_GS_BASE: Self = Self(0xc0000101);
    pub const IA32_KERNEL_GS_BASE: Self = Self(0xc0000102);

    pub fn read(self) -> u64 {
        let mut low: u32;
        let mut high: u32;

        unsafe {
            core::arch::asm!(
                "rdmsr",
                out("eax") low,
                out("edx") high,
                in("ecx") self.0,
            );
        }

        (high as u64) << 32 | low as u64
    }

    pub fn write(self, value: u64) {
        let low = value as u32;
        let high = (value >> 32) as u32;

        unsafe {
            core::arch::asm!(
                "wrmsr",
                in("eax") low,
                in("edx") high,
                in("ecx") self.0,
            );
        }
    }
}

pub fn halt() {
    unsafe {
        core::arch::asm!("hlt");
    }
}

pub fn enable_interrupts() {
    unsafe {
        core::arch::asm!("sti");
    }
}

pub fn disable_interrupts() {
    unsafe {
        core::arch::asm!("cli");
    }
}
