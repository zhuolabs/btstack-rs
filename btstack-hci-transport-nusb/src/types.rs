use crate::constants::{DEFAULT_ACL_IN_EP, DEFAULT_ACL_OUT_EP, DEFAULT_EVENT_EP, USB_MAX_PATH_LEN};
use std::sync::atomic::{AtomicBool, AtomicUsize};
use std::sync::mpsc::SyncSender;
use std::sync::Arc;
use std::thread::JoinHandle;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct hci_transport_config_nusb_t {
    pub type_: u32,
    pub vendor_id: u16,
    pub product_id: u16,
    pub bus_number: u8,
    pub path_len: u8,
    pub path: [u8; USB_MAX_PATH_LEN],
}

#[derive(Debug, Default, Clone)]
pub(crate) struct UsbSelection {
    pub(crate) vendor_id: Option<u16>,
    pub(crate) product_id: Option<u16>,
    pub(crate) bus_number: Option<u8>,
    pub(crate) path: Option<Vec<u8>>,
}

#[derive(Debug, Copy, Clone)]
pub(crate) struct EndpointConfig {
    pub(crate) event_in: u8,
    pub(crate) acl_in: u8,
    pub(crate) acl_out: u8,
}

impl Default for EndpointConfig {
    /// Corresponds to default endpoint addresses initialized in `usb_open` and
    /// endpoint scan fallback behavior in `scan_for_bt_endpoints` within
    /// `platform/libusb/hci_transport_h2_libusb.c`.
    /// Difference: SCO defaults were removed because SCO is unsupported in this transport.
    fn default() -> Self {
        Self {
            event_in: DEFAULT_EVENT_EP,
            acl_in: DEFAULT_ACL_IN_EP,
            acl_out: DEFAULT_ACL_OUT_EP,
        }
    }
}

pub(crate) struct ActiveTransport {
    pub(crate) stop: Arc<AtomicBool>,
    pub(crate) outgoing_pending: Arc<AtomicUsize>,
    pub(crate) outgoing_sender: SyncSender<OutgoingPacket>,
    pub(crate) reader_threads: Vec<JoinHandle<()>>,
    pub(crate) writer_thread: JoinHandle<()>,
    pub(crate) outgoing_queue_depth: usize,
}

pub(crate) enum OutgoingPacket {
    Command(Vec<u8>),
    Acl(Vec<u8>),
    Iso(Vec<u8>),
}

#[derive(Default)]
pub(crate) struct TransportState {
    pub(crate) packet_handler:
        Option<unsafe extern "C" fn(packet_type: u8, packet: *mut u8, size: u16)>,
    pub(crate) selected: UsbSelection,
    pub(crate) active: Option<ActiveTransport>,
}
