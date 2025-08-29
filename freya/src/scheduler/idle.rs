use alloc::sync::Arc;
use spin::Lazy;

use crate::scheduler::{ScheduleEntity, ScheduleEntityType};

pub static GLOBAL_IDLE_TASK: Lazy<Arc<IdleTask>> = Lazy::new(|| Arc::new(IdleTask));

pub struct IdleTask;

impl ScheduleEntity for IdleTask {
    fn entity_type(&self) -> ScheduleEntityType {
        ScheduleEntityType::Idle
    }

    fn invoke(&self) -> ! {
        loop {
            unsafe {
                core::arch::asm!("hlt");
            }
        }
    }
}
