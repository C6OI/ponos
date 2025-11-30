#![warn(clippy::all, clippy::nursery, clippy::pedantic)]
#![allow(clippy::used_underscore_items, reason = "Internal low-level print to VGA")]

#![no_std]
#![no_main]

#![feature(custom_test_frameworks)]
#![test_runner(ponos::test_runner)]
#![reexport_test_harness_main = "test_main"]

use core::panic::PanicInfo;
use ponos::{hlt_loop, println};

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    println!("Welcome to ponos!");

    ponos::init();

    // invoke a breakpoint exception
    x86_64::instructions::interrupts::int3();

    #[cfg(test)]
    test_main();

    hlt_loop();
}

/// This function is called on panic.
#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    hlt_loop();
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    ponos::test_panic_handler(info)
}

#[test_case]
fn trivial_assertion() {
    assert_eq!(1, 1);
}
