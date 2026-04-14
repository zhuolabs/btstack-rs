//! Rust mapping for selected declarations in `ble/att_server.h`.

use crate::att_db::{att_read_callback_t, att_write_callback_t};
use crate::bluetooth::hci_con_handle_t;
use crate::btstack_defines::btstack_packet_handler_t;

unsafe extern "C" {
    pub(crate) fn att_server_init(
        db: *const u8,
        read_callback: att_read_callback_t,
        write_callback: att_write_callback_t,
    );
    pub(crate) fn att_server_register_packet_handler(handler: btstack_packet_handler_t);
    pub(crate) fn att_server_request_can_send_now_event(con_handle: hci_con_handle_t);
    pub(crate) fn att_server_notify(
        con_handle: hci_con_handle_t,
        attribute_handle: u16,
        value: *const u8,
        value_len: u16,
    ) -> u8;
}

/// # Safety
///
/// `db` and callbacks must follow BTstack ABI and lifetime requirements.
pub unsafe fn init(db: *const u8, read_callback: att_read_callback_t, write_callback: att_write_callback_t) {
    unsafe { att_server_init(db, read_callback, write_callback) }
}

pub fn register_packet_handler(handler: btstack_packet_handler_t) {
    // SAFETY: Callback signature matches BTstack packet handler ABI.
    unsafe { att_server_register_packet_handler(handler) }
}

pub fn request_can_send_now_event(con_handle: hci_con_handle_t) {
    // SAFETY: BTstack validates connection handle.
    unsafe { att_server_request_can_send_now_event(con_handle) }
}

/// # Safety
///
/// `value` must point to `value_len` readable bytes.
pub unsafe fn notify(
    con_handle: hci_con_handle_t,
    attribute_handle: u16,
    value: *const u8,
    value_len: u16,
) -> u8 {
    unsafe { att_server_notify(con_handle, attribute_handle, value, value_len) }
}
