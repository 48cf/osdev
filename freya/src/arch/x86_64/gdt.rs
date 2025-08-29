use core::mem::offset_of;

#[repr(C, packed)]
struct GdtPointer {
    limit: u16,
    base: u64,
}

#[repr(C)]
pub struct Gdt {
    null: GdtEntry,
    kernel_code64: GdtEntry,
    kernel_data64: GdtEntry,
}

impl Gdt {
    pub const KERNEL_CODE64_SELECTOR: u16 = offset_of!(Self, kernel_code64) as u16;
    pub const KERNEL_DATA64_SELECTOR: u16 = offset_of!(Self, kernel_data64) as u16;

    pub const fn new() -> Self {
        Self {
            null: GdtEntry::null(),
            kernel_code64: GdtEntry::KERNEL_CODE64,
            kernel_data64: GdtEntry::KERNEL_DATA64,
        }
    }

    pub unsafe fn load(&self) {
        let gdt_ptr = GdtPointer {
            limit: (core::mem::size_of::<Gdt>() - 1) as u16,
            base: self as *const _ as u64,
        };

        unsafe {
            core::arch::asm!("lgdt [{}]", in(reg) &gdt_ptr);
            core::arch::asm!(
                "push {code_seg}",
                "lea rax, [rip + 2f]",
                "push rax",
                "retfq",
                "2:",
                "mov ax, {data_seg}",
                "mov ds, ax",
                "mov es, ax",
                "mov fs, ax",
                "mov gs, ax",
                "mov ss, ax",
                code_seg = const Self::KERNEL_CODE64_SELECTOR,
                data_seg = const Self::KERNEL_DATA64_SELECTOR,
                lateout("rax") _ // rax was modified
            )
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct GdtEntry {
    limit_low: u16,
    base_low: u16,
    base_mid: u8,
    access: u8,
    limit_flags: u8,
    base_high: u8,
}

impl GdtEntry {
    const ACCESS_P: u8 = 1 << 7;
    const ACCESS_DPL3: u8 = 0b11 << 5;
    const ACCESS_S: u8 = 1 << 4;
    const ACCESS_E: u8 = 1 << 3;
    const ACCESS_DC: u8 = 1 << 2;
    const ACCESS_RW: u8 = 1 << 1;
    const ACCESS_A: u8 = 1 << 0;

    const FLAGS_G: u8 = 1 << 7;
    const FLAGS_DB: u8 = 1 << 6;
    const FLAGS_L: u8 = 1 << 5;

    pub const KERNEL_CODE64: Self = Self::new(
        0,
        0xFFFFF,
        Self::ACCESS_P | Self::ACCESS_S | Self::ACCESS_E | Self::ACCESS_RW | Self::ACCESS_A,
        Self::FLAGS_G | Self::FLAGS_L,
    );

    pub const KERNEL_DATA64: Self = Self::new(
        0,
        0xFFFFF,
        Self::ACCESS_P | Self::ACCESS_S | Self::ACCESS_RW | Self::ACCESS_A,
        Self::FLAGS_G | Self::FLAGS_L,
    );

    const fn null() -> Self {
        Self {
            limit_low: 0,
            base_low: 0,
            base_mid: 0,
            access: 0,
            limit_flags: 0,
            base_high: 0,
        }
    }

    const fn new(base: u32, limit: u32, access: u8, flags: u8) -> Self {
        Self {
            limit_low: limit as u16,
            base_low: base as u16,
            base_mid: (base >> 16) as u8,
            access,
            limit_flags: (limit >> 16) as u8 & 0x0F | flags,
            base_high: (base >> 24) as u8,
        }
    }
}
