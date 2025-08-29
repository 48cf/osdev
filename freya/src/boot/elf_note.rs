#[repr(C, align(8))]
struct Aligned<T: Sized>(T);

#[repr(C)]
pub struct ElfNote<T: Sized, const N: usize> {
    namesz: u32,
    descsz: u32,
    kind: u32,
    name: [u8; N],
    data: Aligned<T>,
}

impl<T: Sized, const N: usize> ElfNote<T, N> {
    pub const fn new(name: &str, data: T, kind: u32) -> Self {
        let mut buf = [0u8; N];

        let mut i = 0;
        while i < N - 1 {
            buf[i] = name.as_bytes()[i];
            i += 1;
        }

        Self {
            namesz: N as u32 - 1,
            descsz: core::mem::size_of::<T>() as u32,
            kind,
            name: buf,
            data: Aligned(data),
        }
    }

    pub fn get(&self) -> T {
        let ptr = &raw const self.data.0;

        unsafe { ptr.read_volatile() }
    }
}

#[macro_export]
macro_rules! elf_note {
    ($($vis:vis static $name:ident : $ty:ty = $value:expr;)*) => {
        $(
            #[used]
            #[unsafe(link_section = ".note.kernel")]
            $vis static $name: $crate::boot::elf_note::ElfNote<$ty, { "Managarm".len() + 1 }> =
                $crate::boot::elf_note::ElfNote::new("Managarm", $value, <$ty>::NOTE_KIND);
        )*
    };
}

#[repr(C)]
#[derive(Debug)]
pub struct MemoryLayout {
    direct_physical: u64,
    kernel_virtual: u64,
    kernel_virtual_size: u64,
    alloc_log: u64,
    alloc_log_size: u64,
    eir_info: u64,
}

#[repr(C)]
pub struct PerCpuRegion {
    start: *const u8,
    end: *const u8,
}

unsafe impl Sync for PerCpuRegion {}
unsafe impl Send for PerCpuRegion {}

impl MemoryLayout {
    pub const NOTE_KIND: u32 = 0x1000_0000;

    pub const fn new() -> Self {
        Self {
            direct_physical: 0,
            kernel_virtual: 0,
            kernel_virtual_size: 0,
            alloc_log: 0,
            alloc_log_size: 0,
            eir_info: 0,
        }
    }

    pub fn direct_physical(&self) -> u64 {
        self.direct_physical
    }

    pub fn kernel_virtual(&self) -> u64 {
        self.kernel_virtual
    }

    pub fn kernel_virtual_size(&self) -> usize {
        self.kernel_virtual_size as usize
    }

    pub fn alloc_log(&self) -> u64 {
        self.alloc_log
    }

    pub fn alloc_log_size(&self) -> usize {
        self.alloc_log_size as usize
    }

    pub fn eir_info(&self) -> u64 {
        self.eir_info
    }
}

impl PerCpuRegion {
    pub const NOTE_KIND: u32 = 0x1000_0001;

    pub const fn new(start: *const u8, end: *const u8) -> Self {
        Self { start, end }
    }
}
