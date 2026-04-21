//! Runtime entry point equivalent to BTstack's `port/libusb/main.c` for the
//! `gatt_counter` example.

use std::ffi::{c_int, c_void};

use btstack_hci_transport_nusb::hci_transport_nusb_instance;
use btstack_sys::{
    btstack_memory_init, btstack_run_loop_execute, btstack_run_loop_init,
    btstack_run_loop_posix_get_instance, hci_init,
};

#[cfg(target_os = "windows")]
unsafe extern "C" {
    fn btstack_run_loop_windows_get_instance() -> *const btstack_sys::btstack_run_loop_t;
}

use crate::gatt_counter;

#[cfg(target_os = "windows")]
unsafe fn run_loop_instance() -> *const btstack_sys::btstack_run_loop_t {
    btstack_run_loop_windows_get_instance()
}

#[cfg(target_os = "linux")]
unsafe fn run_loop_instance() -> *const btstack_sys::btstack_run_loop_t {
    btstack_run_loop_posix_get_instance()
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
unsafe fn run_loop_instance() -> *const btstack_sys::btstack_run_loop_t {
    btstack_run_loop_posix_get_instance()
}

/// Equivalent startup flow to BTstack's `port/libusb/main.c` adapted to
/// `gatt_counter` and the Rust `nusb` HCI transport implementation.
pub fn gatt_counter_main() -> c_int {
    unsafe {
        btstack_memory_init();
        btstack_run_loop_init(run_loop_instance());
        hci_init(hci_transport_nusb_instance(), std::ptr::null::<c_void>());
        gatt_counter::btstack_main(0, std::ptr::null());
        btstack_run_loop_execute();
    }

    0
}
