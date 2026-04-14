//! Rust mapping for selected declarations in `gap.h`.

use core::ffi::c_int;

unsafe extern "C" {
    pub(crate) fn gap_advertisements_set_params(
        adv_int_min: u16,
        adv_int_max: u16,
        adv_type: u8,
        direct_address_typ: u8,
        direct_address: *const u8,
        channel_map: u8,
        filter_policy: u8,
    );
    pub(crate) fn gap_advertisements_set_data(advertising_data_length: u8, advertising_data: *mut u8);
    pub(crate) fn gap_advertisements_enable(enabled: c_int);
}

pub fn advertisements_set_params(
    adv_int_min: u16,
    adv_int_max: u16,
    adv_type: u8,
    direct_address_typ: u8,
    direct_address: *const u8,
    channel_map: u8,
    filter_policy: u8,
) {
    // SAFETY: Arguments are plain values/pointers validated by BTstack.
    unsafe {
        gap_advertisements_set_params(
            adv_int_min,
            adv_int_max,
            adv_type,
            direct_address_typ,
            direct_address,
            channel_map,
            filter_policy,
        )
    }
}

/// # Safety
///
/// `advertising_data` must point to `advertising_data_length` bytes.
pub unsafe fn advertisements_set_data(advertising_data_length: u8, advertising_data: *mut u8) {
    unsafe { gap_advertisements_set_data(advertising_data_length, advertising_data) }
}

pub fn advertisements_enable(enabled: bool) {
    // SAFETY: C API accepts integer flag.
    unsafe { gap_advertisements_enable(if enabled { 1 } else { 0 }) }
}
