use core::sync::atomic::{AtomicU64, Ordering};

use alloc::{boxed::Box, sync::Arc};
use spin::Mutex;

use crate::{
    arch::{self, executor::ArchExecutor},
    memory::stack::KernelStack,
    per_cpu::CPU_DATA,
    scheduler::{
        BlockToken, Blockable, Executor, LOCAL_SCHEDULER, ScheduleEntity, ScheduleEntityType,
    },
};

struct FiberInner {
    executor: ArchExecutor,
}

pub struct Fiber {
    block_token: AtomicU64,
    inner: Mutex<FiberInner>,
    stack: KernelStack,
}

extern "C" fn fiber_entry<F: FnMut()>(arg0: usize, _arg1: usize) -> ! {
    let raw = arg0 as *mut F;
    let func = unsafe { &mut *raw };

    (func)();

    unsafe {
        drop(Box::from_raw(raw));
    }

    crate::println!("TODO: Exit fibers");

    LOCAL_SCHEDULER.get().force_reschedule();
    LOCAL_SCHEDULER.get().commit_reschedule();
}

impl Fiber {
    pub fn run<F: FnMut()>(func: F) -> Arc<Self> {
        let func = Box::into_raw(Box::new(func));
        let stack = KernelStack::new();

        let mut executor = ArchExecutor::new();

        *executor.ip() = fiber_entry::<F> as usize;
        *executor.sp() = stack.top() as usize;
        *executor.arg0() = func as usize;

        Arc::new(Self {
            block_token: AtomicU64::new(0),
            inner: Mutex::new(FiberInner { executor }),
            stack,
        })
    }
}

impl ScheduleEntity for Fiber {
    fn as_fiber(self: Arc<Self>) -> Option<Arc<Fiber>> {
        Some(self)
    }

    fn entity_type(&self) -> ScheduleEntityType {
        ScheduleEntityType::Fiber
    }

    fn invoke(&self) -> ! {
        let inner = self.inner.lock();
        let current = &raw const inner.executor;

        drop(inner);

        unsafe {
            (*current).restore();
        }
    }
}

const UNBLOCKED_BIT: u64 = 1 << 0;
const NEXT_BLOCK_TOKEN: u64 = 1 << 1;

impl Blockable for Fiber {
    fn next_block_token(&self) -> BlockToken {
        let token = self.block_token.load(Ordering::Relaxed) & !UNBLOCKED_BIT;

        self.block_token.store(
            token.checked_add(NEXT_BLOCK_TOKEN).unwrap(),
            Ordering::Release,
        );

        BlockToken(token + NEXT_BLOCK_TOKEN)
    }

    fn block(self: &Arc<Self>, _token: BlockToken) {
        let fiber = self.clone();

        LOCAL_SCHEDULER.get().force_reschedule();

        arch::executor::fork_executor(move |frame| {
            arch::executor::run_on_stack(CPU_DATA.get().detached_stack(), |_sp| {
                fiber.inner.lock().executor.save(frame);
                LOCAL_SCHEDULER.get().commit_reschedule();
            });
        });
    }

    fn unblock(self: &Arc<Self>, token: BlockToken) {
        if self
            .block_token
            .compare_exchange(
                token.0,
                token.0 | UNBLOCKED_BIT,
                Ordering::AcqRel,
                Ordering::Relaxed,
            )
            .is_ok()
        {
            LOCAL_SCHEDULER.get().schedule(self.clone());
        }
    }
}
