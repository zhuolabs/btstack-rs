//! Rust mapping for selected declarations in `l2cap.h`.

unsafe extern "C" {
    pub(crate) fn l2cap_init();
}

pub fn init() {
    // SAFETY: Calls a global BTstack initializer.
    unsafe { l2cap_init() }
}
