use core::{cell::UnsafeCell, num::NonZeroU64};

use crate::arch::{self, interrupts_enabled};

crate::define_percpu! {
    static DEADLINE_STATE: DeadlineState = DeadlineState::new();
}

struct DeadlineStateInner {
    current: Option<NonZeroU64>,
    timer: Option<NonZeroU64>,
    preempt: Option<NonZeroU64>,
}

// SAFETY: It's not sane to access another CPU's deadline state.
unsafe impl Sync for DeadlineState {}

struct DeadlineState {
    inner: UnsafeCell<DeadlineStateInner>,
}

impl DeadlineState {
    const fn new() -> Self {
        Self {
            inner: UnsafeCell::new(DeadlineStateInner {
                current: None,
                timer: None,
                preempt: None,
            }),
        }
    }

    fn update_deadline(&self) {
        // assert!(!interrupts_enabled());

        let inner = unsafe { &mut *self.inner.get() };
        let mut candidate = None;

        fn update_candidate(candidate: &mut Option<NonZeroU64>, new: Option<NonZeroU64>) {
            match (candidate, new) {
                (candidate @ None, new) => *candidate = new,
                (Some(candidate), Some(new)) => {
                    let current = *candidate;
                    *candidate = current.min(new);
                }
                _ => {}
            }
        }

        update_candidate(&mut candidate, inner.preempt);
        update_candidate(&mut candidate, inner.timer);

        if inner.current != candidate {
            crate::println!("Setting timer deadline to {:?}", candidate);

            if let Some(deadline) = candidate {
                arch::timer::set_deadline(deadline.get());
            } else {
                arch::timer::clear_deadline();
            }

            inner.current = candidate;
        }
    }

    fn set_timer_deadline(&self, deadline: u64) {
        {
            let inner = unsafe { &mut *self.inner.get() };
            inner.timer = NonZeroU64::new(deadline);
        }
        self.update_deadline();
    }

    fn set_preemption_deadline(&self, deadline: u64) {
        {
            let inner = unsafe { &mut *self.inner.get() };
            inner.preempt = NonZeroU64::new(deadline);
        }
        self.update_deadline();
    }
}

pub fn set_timer_deadline(deadline: u64) {
    DEADLINE_STATE.get().set_timer_deadline(deadline);
}

pub fn set_preemption_deadline(deadline: u64) {
    DEADLINE_STATE.get().set_preemption_deadline(deadline);
}

pub fn get_clock_nanos() -> u64 {
    arch::timer::get_clock_nanos()
}
