//! Rust mapping for selected declarations in `ble/att_db.h`.

use core::ffi::c_int;

use crate::bluetooth::hci_con_handle_t;

pub type att_read_callback_t =
    Option<extern "C" fn(hci_con_handle_t, u16, u16, *mut u8, u16) -> u16>;
pub type att_write_callback_t =
    Option<extern "C" fn(hci_con_handle_t, u16, u16, u16, *mut u8, u16) -> c_int>;
