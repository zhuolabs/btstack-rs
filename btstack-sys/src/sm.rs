//! Rust mapping for selected declarations in `ble/sm.h`.

unsafe extern "C" {
    pub(crate) fn sm_init();
}

pub fn init() {
    // SAFETY: Calls a global BTstack initializer.
    unsafe { sm_init() }
}
