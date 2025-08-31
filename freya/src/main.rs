#![no_main]
#![no_std]

mod vfs;

extern crate alloc;

use core::{fmt::Write, panic::PanicInfo};

#[unsafe(no_mangle)]
extern "C" fn _start(initrd_address: usize, initrd_size: usize) -> ! {
    // let mut w = hel::Writer::info();

    let initrd_bytes =
        unsafe { core::slice::from_raw_parts(initrd_address as *const u8, initrd_size) };

    vfs::unpack_initrd(initrd_bytes);

    let _mbus = vfs::lookup("/usr/bin/mbus").expect("freya: /usr/bin/mbus not found");
    let executor = hel::executor::Executor::new().expect("freya: Failed to create executor");

    executor.block_on(async { loop {} }).unwrap();

    loop {}
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    let mut w = hel::Writer::error();

    writeln!(&mut w, "freya: Oops: {}", info);

    loop {}
}
