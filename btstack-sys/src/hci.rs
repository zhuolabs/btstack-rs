//! Rust mapping for selected declarations in `hci.h`.

use core::ffi::{c_int, c_void};

use crate::btstack_defines::btstack_packet_callback_registration_t;
use crate::hci_cmd::HCI_POWER_MODE;
use crate::hci_transport::hci_transport_t;

unsafe extern "C" {
    pub(crate) fn hci_init(transport: *const hci_transport_t, config: *const c_void);
    pub(crate) fn hci_add_event_handler(callback_handler: *mut btstack_packet_callback_registration_t);
    pub(crate) fn hci_power_control(power_mode: HCI_POWER_MODE) -> c_int;
}

/// # Safety
///
/// `transport` and `config` must match the selected transport implementation.
pub unsafe fn init(transport: *const hci_transport_t, config: *const c_void) {
    unsafe { hci_init(transport, config) }
}

/// # Safety
///
/// `callback_handler` must remain valid while registered.
pub unsafe fn add_event_handler(callback_handler: *mut btstack_packet_callback_registration_t) {
    unsafe { hci_add_event_handler(callback_handler) }
}

pub fn power_control(power_mode: HCI_POWER_MODE) -> i32 {
    // SAFETY: Calls a global BTstack API with plain value arguments.
    unsafe { hci_power_control(power_mode) }
}
