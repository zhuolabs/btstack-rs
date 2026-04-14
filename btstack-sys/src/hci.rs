//! Rust mapping for selected declarations in `hci.h`.

use core::ffi::{c_int, c_void};

use crate::btstack_defines::btstack_packet_callback_registration_t;
use crate::hci_cmd::HCI_POWER_MODE;
use crate::hci_transport::{custom_usb_config_t, hci_transport_t};

unsafe extern "C" {
    pub(crate) fn hci_init(transport: *const hci_transport_t, config: *const c_void);
    pub(crate) fn hci_add_event_handler(
        callback_handler: *mut btstack_packet_callback_registration_t,
    );
    pub(crate) fn hci_power_control(power_mode: HCI_POWER_MODE) -> c_int;
}

/// # Safety
///
/// - `transport` must point to a valid `hci_transport_t` vtable for the entire
///   lifetime of the initialized controller.
/// - `config` must be a valid transport-specific configuration pointer (or null
///   only when the selected transport explicitly allows null config).
/// - The call must happen on the same thread/run-loop that owns BTstack's HCI
///   state, and subsequent transport callbacks must remain thread-affine to
///   that same execution context unless the transport implementation documents
///   stronger synchronization.
/// - Any callback context reachable from the selected transport vtable/callback
///   registrations remains owned by the caller; it must outlive BTstack usage
///   and must not be freed, moved, or mutably aliased while C code may still
///   dereference it.
pub unsafe fn init(transport: *const hci_transport_t, config: *const c_void) {
    unsafe { hci_init(transport, config) }
}

/// # Safety
///
/// - All safety requirements from [`init`] apply.
/// - `config` must point to a valid [`custom_usb_config_t`] whose
///   `config.base.type_` equals `HCI_TRANSPORT_CONFIG_USB`.
pub unsafe fn init_with_custom_usb(
    transport: *const hci_transport_t,
    config: *const custom_usb_config_t,
) {
    unsafe { init(transport, config.cast::<c_void>()) }
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
