pub(crate) mod asm;

mod lapic;

pub mod cpu;
pub mod executor;
pub mod gdt;
pub mod idt;
pub mod interrupts;
pub mod memory;
pub mod tss;
pub mod user;

pub use asm::{disable_interrupts, enable_interrupts, halt};

#[initgraph::task("arch.x86_64.timer-available")]
// #[initgraph::depends()]
#[initgraph::entails(crate::scheduler::SCHEDULING_AVAILABLE)]
static TIMER_AVAILABLE: () = || {};
