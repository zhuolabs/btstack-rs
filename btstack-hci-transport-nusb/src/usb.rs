use crate::constants::USB_MAX_PATH_LEN;
use crate::types::{EndpointConfig, UsbSelection};
use nusb::transfer::{Direction, EndpointType};
use nusb::Interface;

/// Corresponds to `scan_for_bt_device`/`try_open_device` filtering logic in
/// `platform/libusb/hci_transport_h2_libusb.c`.
/// Difference: this Rust implementation uses `nusb::list_devices()` iterator APIs.
pub(crate) fn select_device(selection: UsbSelection) -> Option<nusb::DeviceInfo> {
    let devices = nusb::list_devices().ok()?;
    devices
        .into_iter()
        .find(|device| is_candidate_device(device, &selection))
}

/// Corresponds to USB selector checks in `usb_open` and known-device scanning in
/// `platform/libusb/hci_transport_h2_libusb.c`.
/// Difference: matches optional `vendor_id`/`product_id`/`bus_number`/`path` from Rust config.
fn is_candidate_device(device: &nusb::DeviceInfo, selection: &UsbSelection) -> bool {
    if let Some(vendor_id) = selection.vendor_id {
        if device.vendor_id() != vendor_id {
            return false;
        }
    }

    if let Some(product_id) = selection.product_id {
        if device.product_id() != product_id {
            return false;
        }
    }
    if let Some(bus_number) = selection.bus_number {
        if device.bus_number() != bus_number {
            return false;
        }
    }
    if let Some(path) = selection.path.as_deref() {
        if usb_path(device).as_deref() != Some(path) {
            return false;
        }
    }

    selection.vendor_id.is_some()
        || selection.product_id.is_some()
        || selection.bus_number.is_some()
        || selection.path.is_some()
        || is_bluetooth_device(device)
}

/// Corresponds to `is_known_bt_device` fallback behavior in
/// `platform/libusb/hci_transport_h2_libusb.c`.
/// Difference: detects by Bluetooth interface class/subclass/protocol instead of static VID/PID table.
fn is_bluetooth_device(device: &nusb::DeviceInfo) -> bool {
    device.interfaces().any(|interface| {
        interface.class() == 0xE0 && interface.subclass() == 0x01 && interface.protocol() == 0x01
    })
}

/// Corresponds to endpoint discovery in `scan_for_bt_endpoints` in
/// `platform/libusb/hci_transport_h2_libusb.c`.
/// Difference: only event/ACL endpoints are discovered; SCO endpoints are intentionally ignored.
pub(crate) fn detect_endpoints(interface: &Interface) -> EndpointConfig {
    let mut config = EndpointConfig::default();
    let mut found_event = false;
    let mut found_acl_in = false;
    let mut found_acl_out = false;

    for alt in interface
        .descriptors()
        .filter(|alt| alt.interface_number() == interface.interface_number())
        .filter(|alt| alt.alternate_setting() == 0)
    {
        for endpoint in alt.endpoints() {
            match (endpoint.transfer_type(), endpoint.direction()) {
                (EndpointType::Interrupt, Direction::In) if !found_event => {
                    config.event_in = endpoint.address();
                    found_event = true;
                }
                (EndpointType::Bulk, Direction::In) if !found_acl_in => {
                    config.acl_in = endpoint.address();
                    found_acl_in = true;
                }
                (EndpointType::Bulk, Direction::Out) if !found_acl_out => {
                    config.acl_out = endpoint.address();
                    found_acl_out = true;
                }
                _ => {}
            }
        }
    }

    config
}

/// Corresponds to the `libusb_get_port_numbers` + USB path reporting behavior in
/// `prepare_device` and `hci_transport_h2_libusb_emit_usb_info`.
/// Difference: Linux/Android parse from `sysfs_path()` name via `nusb`.
pub(crate) fn usb_path(device: &nusb::DeviceInfo) -> Option<Vec<u8>> {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        let name = device.sysfs_path().file_name()?.to_str()?;
        parse_usb_path(name)
    }
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    {
        let _ = device;
        None
    }
}

/// Rust helper for `usb_path`.
/// Corresponds conceptually to how `libusb_get_port_numbers` yields hierarchical USB ports.
fn parse_usb_path(sysfs_name: &str) -> Option<Vec<u8>> {
    let (_, chain) = sysfs_name.split_once('-')?;
    let mut path = Vec::new();
    for elem in chain.split('.') {
        let port = elem.parse::<u8>().ok()?;
        path.push(port);
    }
    if path.is_empty() || path.len() > USB_MAX_PATH_LEN {
        return None;
    }
    Some(path)
}
