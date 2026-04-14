use crate::btstack_defines::btstack_linked_item_t;

#[repr(C)]
pub struct btstack_run_loop_t {
    _private: [u8; 0],
    _marker: core::marker::PhantomData<(*mut u8, core::marker::PhantomPinned)>,
}

pub type btstack_time_t = u32;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct btstack_timer_source_t {
    pub item: btstack_linked_item_t,
    pub timeout: btstack_time_t,
    pub process: Option<extern "C" fn(*mut btstack_timer_source_t)>,
    pub context: *mut core::ffi::c_void,
}

/// Opaque BTstack run-loop handle.
pub type BtstackRunLoop = btstack_run_loop_t;

unsafe extern "C" {
    pub(crate) fn btstack_run_loop_set_timer(timer: *mut btstack_timer_source_t, timeout_in_ms: u32);
    pub(crate) fn btstack_run_loop_add_timer(timer: *mut btstack_timer_source_t);
    pub(crate) fn btstack_run_loop_execute();
}

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

/// Arms a one-shot timer.
///
/// # Safety
///
/// `timer` must be initialized and valid for BTstack to access.
pub unsafe fn set_timer(timer: *mut btstack_timer_source_t, timeout_in_ms: u32) {
    unsafe { btstack_run_loop_set_timer(timer, timeout_in_ms) }
}

/// Adds a timer source to the run loop.
///
/// # Safety
///
/// `timer` must stay valid while it is registered.
pub unsafe fn add_timer(timer: *mut btstack_timer_source_t) {
    unsafe { btstack_run_loop_add_timer(timer) }
}

/// Enter run loop execution.
pub fn execute() {
    // SAFETY: Global event loop entry point.
    unsafe { btstack_run_loop_execute() }
}
