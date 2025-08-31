use crate::memory::PageAccess;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ImageDomain {
    User,
    Kernel,
}

pub trait RegisterImage {
    fn dump_registers(&self);
    fn domain(&self) -> ImageDomain;

    fn ip(&self) -> usize;
    fn sp(&self) -> usize;
    fn flags(&self) -> usize;

    fn set_ip(&mut self, value: usize);
    fn set_sp(&mut self, value: usize);
    fn set_flags(&mut self, value: usize);
}

pub trait IrqRegisterImage: RegisterImage {
    fn irq_number(&self) -> usize;
}

pub trait FaultErrorCode: Copy {
    fn into_page_access(self) -> PageAccess;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FaultKind {
    Breakpoint,
    InvalidOpcode,
    PageFault,
    Other(usize),
}

pub trait FaultRegisterImage: RegisterImage {
    fn fault_kind(&self) -> FaultKind;
    fn error_code(&self) -> impl FaultErrorCode;
    fn fault_address(&self) -> usize;
}

pub trait SyscallRegisterImage: RegisterImage {
    fn syscall_number(&self) -> usize;
    fn arg0(&self) -> usize;
    fn arg1(&self) -> usize;
    fn arg2(&self) -> usize;
    fn arg3(&self) -> usize;
    fn arg4(&self) -> usize;
    fn arg5(&self) -> usize;
    fn arg6(&self) -> usize;
    fn arg7(&self) -> usize;
    fn arg8(&self) -> usize;

    fn set_error(&mut self, value: usize);
    fn set_out0(&mut self, value: usize);
    fn set_out1(&mut self, value: usize);
}
