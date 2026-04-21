//! Runtime entry point equivalent to BTstack's `port/libusb/main.c` for the
//! `gatt_counter` example.

use std::ffi::c_int;

use crate::{gatt_counter, runtime::BtstackRuntime};

/// Equivalent startup flow to BTstack's `port/libusb/main.c` adapted to
/// `gatt_counter` and the Rust `nusb` HCI transport implementation.
pub fn gatt_counter_main() -> c_int {
    let mut runtime = match BtstackRuntime::start() {
        Ok(runtime) => runtime,
        Err(_) => return -1,
    };

    gatt_counter::btstack_main(0, std::ptr::null());

    match runtime.join() {
        Ok(()) => 0,
        Err(_) => -1,
    }
}
