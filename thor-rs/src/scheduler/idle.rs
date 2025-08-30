use alloc::sync::Arc;
use spin::Lazy;

use crate::{
    arch,
    per_cpu::CPU_DATA,
    scheduler::{ScheduleEntity, ScheduleEntityType},
};

pub static GLOBAL_IDLE_TASK: Lazy<Arc<IdleTask>> = Lazy::new(|| Arc::new(IdleTask));

pub struct IdleTask;

impl ScheduleEntity for IdleTask {
    fn entity_type(&self) -> ScheduleEntityType {
        ScheduleEntityType::Idle
    }

    fn invoke(&self) -> ! {
        arch::executor::run_on_stack(CPU_DATA.get().idle_stack(), |_sp| {
            crate::println!("System is idle");

            loop {
                arch::halt();
            }
        });
    }
}
