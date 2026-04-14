//! Rust mapping for selected declarations in `hci_transport.h`.

use core::ffi::{c_char, c_int, c_void};
use core::mem::{offset_of, size_of};

pub type hci_transport_packet_handler_t =
    Option<unsafe extern "C" fn(packet_type: u8, packet: *mut u8, size: u16)>;

pub type hci_transport_init_t = Option<unsafe extern "C" fn(transport_config: *const c_void)>;
pub type hci_transport_open_t = Option<unsafe extern "C" fn() -> c_int>;
pub type hci_transport_close_t = Option<unsafe extern "C" fn() -> c_int>;
pub type hci_transport_register_packet_handler_t =
    Option<unsafe extern "C" fn(handler: hci_transport_packet_handler_t)>;
pub type hci_transport_can_send_packet_now_t =
    Option<unsafe extern "C" fn(packet_type: u8) -> c_int>;
pub type hci_transport_send_packet_t =
    Option<unsafe extern "C" fn(packet_type: u8, packet: *mut u8, size: c_int) -> c_int>;
pub type hci_transport_set_baudrate_t = Option<unsafe extern "C" fn(baudrate: u32) -> c_int>;
pub type hci_transport_reset_link_t = Option<unsafe extern "C" fn()>;
pub type hci_transport_set_sco_config_t =
    Option<unsafe extern "C" fn(voice_setting: u16, num_connections: c_int)>;

/// Matches `hci_transport_t` in BTstack `src/hci_transport.h`
/// at commit `5bc5cbdbeec33be1fdbd0d50e04c0f6deab99d2d`.
///
/// Keep this in sync with upstream when upgrading the BTstack submodule.
#[repr(C)]
pub struct hci_transport_t {
    pub name: *const c_char,
    pub init: hci_transport_init_t,
    pub open: hci_transport_open_t,
    pub close: hci_transport_close_t,
    pub register_packet_handler: hci_transport_register_packet_handler_t,
    pub can_send_packet_now: hci_transport_can_send_packet_now_t,
    pub send_packet: hci_transport_send_packet_t,
    pub set_baudrate: hci_transport_set_baudrate_t,
    pub reset_link: hci_transport_reset_link_t,
    pub set_sco_config: hci_transport_set_sco_config_t,
}

const _: [(); size_of::<*const c_void>()] = [(); size_of::<*const c_char>()];
const _: [(); size_of::<*const c_void>()] = [(); size_of::<hci_transport_init_t>()];
const _: [(); size_of::<*const c_void>()] = [(); size_of::<hci_transport_open_t>()];
const _: [(); size_of::<*const c_void>()] = [(); size_of::<hci_transport_close_t>()];
const _: [(); size_of::<*const c_void>()] =
    [(); size_of::<hci_transport_register_packet_handler_t>()];
const _: [(); size_of::<*const c_void>()] = [(); size_of::<hci_transport_can_send_packet_now_t>()];
const _: [(); size_of::<*const c_void>()] = [(); size_of::<hci_transport_send_packet_t>()];
const _: [(); size_of::<*const c_void>()] = [(); size_of::<hci_transport_set_baudrate_t>()];
const _: [(); size_of::<*const c_void>()] = [(); size_of::<hci_transport_reset_link_t>()];
const _: [(); size_of::<*const c_void>()] = [(); size_of::<hci_transport_set_sco_config_t>()];

const _: [(); 0] = [(); offset_of!(hci_transport_t, name)];
const _: [(); size_of::<*const c_void>() * 1] = [(); offset_of!(hci_transport_t, init)];
const _: [(); size_of::<*const c_void>() * 2] = [(); offset_of!(hci_transport_t, open)];
const _: [(); size_of::<*const c_void>() * 3] = [(); offset_of!(hci_transport_t, close)];
const _: [(); size_of::<*const c_void>() * 4] =
    [(); offset_of!(hci_transport_t, register_packet_handler)];
const _: [(); size_of::<*const c_void>() * 5] =
    [(); offset_of!(hci_transport_t, can_send_packet_now)];
const _: [(); size_of::<*const c_void>() * 6] = [(); offset_of!(hci_transport_t, send_packet)];
const _: [(); size_of::<*const c_void>() * 7] = [(); offset_of!(hci_transport_t, set_baudrate)];
const _: [(); size_of::<*const c_void>() * 8] = [(); offset_of!(hci_transport_t, reset_link)];
const _: [(); size_of::<*const c_void>() * 9] = [(); offset_of!(hci_transport_t, set_sco_config)];
const _: [(); size_of::<*const c_void>() * 10] = [(); size_of::<hci_transport_t>()];
