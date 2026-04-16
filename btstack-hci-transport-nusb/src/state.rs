use crate::types::{TransportState, UsbSelection};
use std::sync::Mutex;

pub(crate) static STATE: Mutex<TransportState> = Mutex::new(TransportState {
    packet_handler: None,
    selected: UsbSelection {
        vendor_id: None,
        product_id: None,
        bus_number: None,
        path: None,
    },
    active: None,
});
