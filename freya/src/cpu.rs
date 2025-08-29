pub trait IrqRegisterAccessor {
    fn ip(&self) -> usize;

    fn set_ip(&mut self, value: usize);
}

pub trait FaultRegisterAccessor {
    fn ip(&self) -> usize;
    fn sp(&self) -> usize;
    fn fault_code(&self) -> usize;
    fn fault_address(&self) -> usize;

    fn set_ip(&mut self, value: usize);
    fn set_sp(&mut self, value: usize);
}

pub trait ForkRegisterAccessor {
    fn ip(&self) -> usize;
    fn sp(&self) -> usize;

    fn set_ip(&mut self, value: usize);
    fn set_sp(&mut self, value: usize);
}
