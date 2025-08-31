mod fiber;
mod idle;
mod thread;

pub use fiber::Fiber;
pub use thread::Thread;

use core::{
    sync::atomic::AtomicU64,
    task::{Context, Poll, Waker},
};

use alloc::{collections::vec_deque::VecDeque, sync::Arc, task::Wake};
use spin::Mutex;

use crate::{
    KernelResult, arch::interrupts::ArchInterruptFrame, memory::client::UserAccessRegion,
    scheduler::idle::GLOBAL_IDLE_TASK,
};

static THREAD_ID_ALLOCATOR: AtomicU64 = AtomicU64::new(1);

crate::define_percpu! {
    pub static LOCAL_SCHEDULER: Scheduler = Scheduler::new();
}

struct SchedulerInner {
    current: Arc<dyn ScheduleEntity>,
    queue: VecDeque<Arc<dyn ScheduleEntity>>,
}

impl SchedulerInner {
    fn set_current(&mut self, mut entity: Arc<dyn ScheduleEntity>) -> Arc<dyn ScheduleEntity> {
        core::mem::swap(&mut self.current, &mut entity);

        // Drop the old reference to the previous task.
        //
        // This is the equivalent [`Arc::from_raw`] to the [`Arc::into_raw`]
        // in [`Scheduler::commit_reschedule`].
        unsafe {
            let ptr = Arc::into_raw(entity);
            let _ = Arc::from_raw(ptr);

            Arc::from_raw(ptr)
        }
    }
}

pub struct Scheduler {
    inner: Mutex<SchedulerInner>,
}

impl Scheduler {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(SchedulerInner {
                current: GLOBAL_IDLE_TASK.clone(),
                queue: VecDeque::with_capacity(128),
            }),
        }
    }

    pub fn current(&self) -> Arc<dyn ScheduleEntity> {
        self.inner.lock().current.clone()
    }

    pub fn schedule(&self, entity: Arc<dyn ScheduleEntity>) {
        let mut inner = self.inner.lock();
        inner.queue.push_back(entity);
    }

    pub fn reschedule(&self) -> bool {
        let mut inner = self.inner.lock();

        if inner.queue.is_empty() {
            return false;
        }

        // TODO: Implement logic for picking the next entity to run.

        let next = inner.queue.pop_front().unwrap_or_else(|| {
            // If the queue is empty, return the idle task.
            GLOBAL_IDLE_TASK.clone()
        });

        let current = inner.set_current(next);

        if current.entity_type() != ScheduleEntityType::Idle {
            inner.queue.push_back(current);
        }

        true

        // let mut inner = self.inner.lock();

        // let entity = inner.queue.pop_front().expect("No entity to schedule");
        // let current = inner
        //     .current
        //     .take()
        //     .expect("No current entity to reschedule");

        // inner.current = Some(entity);
    }

    pub fn force_reschedule(&self) {
        let mut inner = self.inner.lock();

        let next = inner.queue.pop_front().unwrap_or_else(|| {
            // If the queue is empty, return the idle task.
            GLOBAL_IDLE_TASK.clone()
        });

        let _ = inner.set_current(next);
    }

    pub fn commit_reschedule(&self) -> ! {
        let inner = self.inner.lock();
        let current = Arc::into_raw(inner.current.clone());

        drop(inner);

        unsafe {
            (*current).invoke();
        }
    }
}

pub trait Executor {
    fn user_access_region(&self) -> Option<&UserAccessRegion>;

    fn save(&mut self, frame: &ArchInterruptFrame);
    fn restore(&self) -> !;

    fn ip(&mut self) -> &mut usize;
    fn sp(&mut self) -> &mut usize;

    fn arg0(&mut self) -> &mut usize;
    fn arg1(&mut self) -> &mut usize;

    fn result0(&mut self) -> &mut usize;
    fn result1(&mut self) -> &mut usize;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScheduleEntityType {
    Idle,
    Thread,
    Fiber,
}

pub trait ScheduleEntity: Sync + Send {
    fn as_thread(self: Arc<Self>) -> Option<Arc<Thread>> {
        None
    }

    fn as_fiber(self: Arc<Self>) -> Option<Arc<Fiber>> {
        None
    }

    fn entity_type(&self) -> ScheduleEntityType;
    fn invoke(&self) -> !;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockToken(u64);

pub trait Blockable: ScheduleEntity + Sync + Send {
    fn next_block_token(&self) -> BlockToken;
    fn block(self: &Arc<Self>, token: BlockToken);
    fn unblock(self: &Arc<Self>, token: BlockToken);
}

pub fn async_block<B: Blockable + 'static, F: Future<Output = KernelResult<T>>, T>(
    entity: &Arc<B>,
    future: F,
) -> KernelResult<T> {
    let mut future = core::pin::pin!(future);

    struct ThreadWaker<B: Blockable> {
        thread: Arc<B>,
        token: BlockToken,
    }

    impl<B: Blockable> Wake for ThreadWaker<B> {
        fn wake(self: Arc<Self>) {
            self.thread.unblock(self.token);
        }
    }

    loop {
        let block_token = entity.next_block_token();
        let waker = Arc::new(ThreadWaker {
            thread: entity.clone(),
            token: block_token,
        });

        let waker = Waker::from(waker.clone());
        let mut ctx = Context::from_waker(&waker);

        match future.as_mut().poll(&mut ctx) {
            Poll::Pending => {
                // if wq.check() {
                //     wq.run();
                //     continue;
                // }

                entity.block(block_token);
            }
            Poll::Ready(val) => return val,
        }
    }
}
