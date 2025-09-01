use core::mem::offset_of;

use alloc::sync::Arc;
use async_channel::Sender;
use async_task::Runnable;

use crate::{
    arch::cpu::ArchCpuData,
    memory::stack::KernelStack,
    per_cpu::PerCpu,
    scheduler::{self, Fiber},
};

#[unsafe(link_section = ".percpu.head")]
pub static CPU_DATA: PerCpu<CpuData> = PerCpu::new();

pub struct WorkQueue {
    fiber: Arc<Fiber>,
    queue: Sender<Runnable>,
}

impl WorkQueue {
    pub fn new() -> Self {
        let (tx, rx) = async_channel::unbounded::<Runnable>();
        let fiber = Fiber::run(|fiber| {
            let _ = scheduler::async_block::<_, _, !>(&fiber, async {
                while let Ok(runnable) = rx.recv().await {
                    runnable.run();
                }

                panic!("WorkQueue fiber exited unexpectedly");
            });
        });

        Self { fiber, queue: tx }
    }

    pub fn submit(&self, runnable: Runnable) {
        self.queue
            .try_send(runnable)
            .expect("WorkQueue channel is full");
    }
}

pub struct CpuData {
    arch_data: ArchCpuData,
    cpu_id: u32,
    idle_stack: KernelStack,
    detached_stack: KernelStack,
    work_queue: WorkQueue,
}

impl CpuData {
    pub fn new(cpu_id: u32) -> Self {
        Self {
            arch_data: ArchCpuData::new(),
            cpu_id,
            idle_stack: KernelStack::new(),
            detached_stack: KernelStack::new(),
            work_queue: WorkQueue::new(),
        }
    }

    pub unsafe fn from_arch_data(arch_data: *const ArchCpuData) -> &'static Self {
        unsafe { &*((arch_data as usize - offset_of!(Self, arch_data)) as *const Self) }
    }

    pub fn arch_data(&self) -> &ArchCpuData {
        &self.arch_data
    }

    pub fn cpu_id(&self) -> u32 {
        self.cpu_id
    }

    pub fn idle_stack(&self) -> &KernelStack {
        &self.idle_stack
    }

    pub fn detached_stack(&self) -> &KernelStack {
        &self.detached_stack
    }

    pub fn work_queue(&self) -> &WorkQueue {
        &self.work_queue
    }
}
