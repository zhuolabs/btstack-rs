use std::time::Duration;

pub(crate) const DEFAULT_EVENT_EP: u8 = 0x81;
pub(crate) const DEFAULT_ACL_IN_EP: u8 = 0x82;
pub(crate) const DEFAULT_ACL_OUT_EP: u8 = 0x02;
pub(crate) const EVENT_TRANSFER_SIZE: usize = 260;
pub(crate) const ACL_TRANSFER_SIZE: usize = 2048;
pub(crate) const POLL_INTERVAL: Duration = Duration::from_millis(1);
pub(crate) const OUTGOING_QUEUE_DEPTH: usize = 4;
pub(crate) const USB_MAX_PATH_LEN: usize = 7;
pub(crate) const EVENT_IN_FLIGHT: usize = 3;
pub(crate) const ACL_IN_FLIGHT: usize = 3;
pub(crate) const ACL_OUT_IN_FLIGHT: usize = 4;
