use core::sync::atomic::{AtomicU64, Ordering};

use alloc::sync::Arc;
use spin::Mutex;

use crate::{
    arch::{self, executor::ArchExecutor, user::ArchUserContext},
    memory::client::ClientPageSpace,
    per_cpu::CPU_DATA,
    scheduler::{
        BlockToken, Blockable, Executor, LOCAL_SCHEDULER, ScheduleEntity, ScheduleEntityType,
        THREAD_ID_ALLOCATOR,
    },
    universe::Universe,
};

struct ThreadInner {
    context: ArchUserContext,
    executor: ArchExecutor,
}

pub struct Thread {
    tid: AtomicU64,
    block_token: AtomicU64,
    space: Arc<ClientPageSpace>,
    universe: Arc<Universe>,
    inner: Mutex<ThreadInner>,
}

impl Thread {
    pub fn new(executor: ArchExecutor, space: Arc<ClientPageSpace>) -> Arc<Self> {
        let tid = THREAD_ID_ALLOCATOR.fetch_add(1, Ordering::Relaxed);

        Arc::new(Self {
            space,
            tid: AtomicU64::new(tid),
            block_token: AtomicU64::new(0),
            universe: Universe::new(),
            inner: Mutex::new(ThreadInner {
                context: ArchUserContext::new(CPU_DATA.get()),
                executor,
            }),
        })
    }

    pub fn space(&self) -> &Arc<ClientPageSpace> {
        &self.space
    }

    pub fn universe(&self) -> &Arc<Universe> {
        &self.universe
    }
}

impl ScheduleEntity for Thread {
    fn as_thread(self: Arc<Self>) -> Option<Arc<Thread>> {
        Some(self)
    }

    fn entity_type(&self) -> ScheduleEntityType {
        ScheduleEntityType::Thread
    }

    fn invoke(&self) -> ! {
        let inner = self.inner.lock();

        inner.context.activate();

        self.space.space().activate();

        let current = &raw const inner.executor;

        drop(inner);

        unsafe {
            (*current).restore();
        }
    }
}

const UNBLOCKED_BIT: u64 = 1 << 0;
const NEXT_BLOCK_TOKEN: u64 = 1 << 1;

impl Blockable for Thread {
    fn next_block_token(&self) -> BlockToken {
        let token = self.block_token.load(Ordering::Relaxed) & !UNBLOCKED_BIT;

        self.block_token.store(
            token.checked_add(NEXT_BLOCK_TOKEN).unwrap(),
            Ordering::Release,
        );

        BlockToken(token + NEXT_BLOCK_TOKEN)
    }

    fn block(self: &Arc<Self>, _token: BlockToken) {
        let thread = self.clone();

        LOCAL_SCHEDULER.get().reschedule();

        arch::executor::fork_executor(move |frame| {
            arch::executor::run_on_stack(CPU_DATA.get().detached_stack(), |_sp| {
                thread.inner.lock().executor.save(frame);
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
