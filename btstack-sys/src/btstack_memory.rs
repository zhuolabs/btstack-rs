/// Initializes BTstack's internal memory pools.
pub fn memory_init() {
    // SAFETY: BTstack exposes this initializer as a global process setup step.
    unsafe { crate::ffi::btstack_memory_init() }
}
