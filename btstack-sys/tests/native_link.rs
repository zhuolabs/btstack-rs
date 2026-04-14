use btstack_sys::btstack_memory::memory_init;

#[test]
fn links_and_calls_btstack_memory_init() {
    memory_init();
}
