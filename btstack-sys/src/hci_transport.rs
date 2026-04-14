//! Rust mapping for selected declarations in `hci_transport.h`.

#[repr(C)]
pub struct hci_transport_t {
    _private: [u8; 0],
    _marker: core::marker::PhantomData<(*mut u8, core::marker::PhantomPinned)>,
}
