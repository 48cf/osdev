#![no_main]
#![no_std]

use core::{fmt::Write, panic::PanicInfo};

#[unsafe(no_mangle)]
extern "C" fn _start() -> ! {
    let mut w = hel::Writer::info();

    writeln!(&mut w, "Hello, world!");

    let executor = hel::executor::Executor::new().expect("Failed to create executor");

    executor.block_on(async { loop {} }).unwrap();

    loop {}
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
