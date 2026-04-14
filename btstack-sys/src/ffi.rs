use crate::btstack_run_loop::btstack_run_loop_t;

unsafe extern "C" {
    pub(crate) fn btstack_memory_init();
    pub(crate) fn btstack_run_loop_init(run_loop: *const btstack_run_loop_t);
}
