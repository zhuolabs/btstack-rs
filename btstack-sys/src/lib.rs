//! Minimal low-level FFI prototype for BTstack integration.

unsafe extern "C" {
    fn btstack_rs_init();
    fn btstack_rs_tick();
    fn btstack_rs_counter() -> i32;
}

/// Initializes the underlying C runtime state.
pub fn init() {
    // SAFETY: Calls into a local C shim with no preconditions.
    unsafe { btstack_rs_init() }
}

/// Advances internal state by one step.
pub fn tick() {
    // SAFETY: Calls into a local C shim with no preconditions.
    unsafe { btstack_rs_tick() }
}

/// Returns a diagnostic counter value from the C layer.
pub fn counter() -> i32 {
    // SAFETY: Reads an integer from a local C shim with no preconditions.
    unsafe { btstack_rs_counter() }
}
