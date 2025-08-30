#![no_main]
#![no_std]

extern crate alloc;

use core::{fmt::Write, panic::PanicInfo};

#[unsafe(no_mangle)]
extern "C" fn _start() -> ! {
    let mut w = hel::Writer::info();

    writeln!(&mut w, "Hello, world!");

    let vec = alloc::vec![1, 2, 3, 4, 5];

    writeln!(&mut w, "Vector: {:?}", vec);

    let executor = hel::executor::Executor::new().expect("Failed to create executor");

    executor.block_on(async { loop {} }).unwrap();

    loop {}
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    let mut w = hel::Writer::error();

    writeln!(&mut w, "Oops: {}", info);

    loop {}
}
