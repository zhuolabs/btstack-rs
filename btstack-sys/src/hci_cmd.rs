//! Rust mapping for selected declarations in `hci_cmd.h`.

#[repr(i32)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum HCI_POWER_MODE {
    HCI_POWER_OFF = 0,
    HCI_POWER_ON = 1,
    HCI_POWER_SLEEP = 2,
}
