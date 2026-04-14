#[repr(C)]
pub struct btstack_run_loop_t {
    _private: [u8; 0],
    _marker: core::marker::PhantomData<(*mut u8, core::marker::PhantomPinned)>,
}

/// Opaque BTstack run-loop handle.
pub type BtstackRunLoop = btstack_run_loop_t;

/// Initializes the global BTstack run loop state with a concrete run-loop implementation.
///
/// # Safety
///
/// The caller must provide a valid pointer returned by a BTstack run-loop provider
/// (for example, a platform-specific `*_get_instance()` function) and ensure it
/// outlives BTstack usage.
pub unsafe fn run_loop_init(run_loop: *const BtstackRunLoop) {
    unsafe { crate::ffi::btstack_run_loop_init(run_loop) }
}
