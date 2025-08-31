#[repr(C, packed)]
struct IdtPointer {
    limit: u16,
    base: u64,
}

#[repr(C)]
pub struct Idt {
    entries: [IdtEntry; 256],
}

impl Idt {
    pub const fn new() -> Self {
        Self {
            entries: [IdtEntry::empty(); 256],
        }
    }

    pub unsafe fn load(&self) {
        let gdt_ptr = IdtPointer {
            limit: (core::mem::size_of::<Idt>() - 1) as u16,
            base: self as *const _ as u64,
        };

        unsafe {
            core::arch::asm!("lidt [{}]", in(reg) &gdt_ptr);
        }
    }

    pub fn set_handler(
        &mut self,
        index: usize,
        handler: unsafe extern "C" fn(),
        selector: u16,
        ist: u8,
        type_attr: u8,
    ) {
        let handler_address = handler as u64;

        self.entries[index] = IdtEntry {
            offset_low: handler_address as u16,
            selector,
            ist,
            type_attr,
            offset_mid: (handler_address >> 16) as u16,
            offset_high: (handler_address >> 32) as u32,
            zero: 0,
        };
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct IdtEntry {
    offset_low: u16,
    selector: u16,
    ist: u8,
    type_attr: u8,
    offset_mid: u16,
    offset_high: u32,
    zero: u32,
}

impl IdtEntry {
    pub const PRESENT: u8 = 1 << 7;
    pub const DPL3: u8 = 3 << 5;
    pub const CALL_GATE: u8 = 0b1100;
    pub const INTERRUPT_GATE: u8 = 0b1110;
    pub const TRAP_GATE: u8 = 0b1111;

    const fn empty() -> Self {
        Self {
            offset_low: 0,
            selector: 0,
            ist: 0,
            type_attr: 0,
            offset_mid: 0,
            offset_high: 0,
            zero: 0,
        }
    }
}
