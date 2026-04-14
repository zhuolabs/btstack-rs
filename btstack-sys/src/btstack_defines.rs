//! Rust mapping for selected declarations in `btstack_defines.h`.

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct btstack_linked_item_t {
    pub next: *mut btstack_linked_item_t,
}

pub type btstack_packet_handler_t = Option<extern "C" fn(u8, u16, *mut u8, u16)>;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct btstack_packet_callback_registration_t {
    pub item: btstack_linked_item_t,
    pub callback: btstack_packet_handler_t,
}
