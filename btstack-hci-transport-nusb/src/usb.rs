use crate::constants::{ALT_SETTING_16_BIT, ALT_SETTING_8_BIT, USB_MAX_PATH_LEN};
use crate::types::{EndpointConfig, ScoState, UsbSelection};
use nusb::transfer::{Direction, EndpointType};
use nusb::Interface;
use std::os::raw::c_int;
use std::sync::{Arc, Mutex};

pub(crate) fn select_device(selection: UsbSelection) -> Option<nusb::DeviceInfo> {
    let devices = nusb::list_devices().ok()?;
    devices
        .into_iter()
        .find(|device| is_candidate_device(device, &selection))
}

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

fn is_bluetooth_device(device: &nusb::DeviceInfo) -> bool {
    device.interfaces().any(|interface| {
        interface.class() == 0xE0 && interface.subclass() == 0x01 && interface.protocol() == 0x01
    })
}

pub(crate) fn detect_endpoints(
    interface: &Interface,
    sco_interface: Option<&Interface>,
) -> EndpointConfig {
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

    if let Some(sco_interface) = sco_interface {
        for alt in sco_interface.descriptors() {
            for endpoint in alt.endpoints() {
                match (endpoint.transfer_type(), endpoint.direction()) {
                    (EndpointType::Isochronous, Direction::In) if config.sco_in.is_none() => {
                        config.sco_in = Some(endpoint.address());
                    }
                    (EndpointType::Isochronous, Direction::Out) if config.sco_out.is_none() => {
                        config.sco_out = Some(endpoint.address());
                    }
                    _ => {}
                }
            }
        }
    }

    config
}

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

pub(crate) fn endpoint_sco_state(
    sco_interface: Option<&Interface>,
    endpoints: &EndpointConfig,
) -> Option<Arc<Mutex<ScoState>>> {
    let Some(interface) = sco_interface else {
        return None;
    };
    if endpoints.sco_in.is_none() && endpoints.sco_out.is_none() {
        return None;
    }
    Some(Arc::new(Mutex::new(ScoState {
        interface: interface.clone(),
        voice_setting: 0,
        num_connections: 0,
        sco_in_endpoint: endpoints.sco_in,
        sco_out_endpoint: endpoints.sco_out,
    })))
}

pub(crate) fn sco_alt_setting(voice_setting: u16, num_connections: c_int) -> u8 {
    if num_connections <= 0 {
        return 0;
    }
    let index = (num_connections as usize).saturating_sub(1);
    if index >= ALT_SETTING_8_BIT.len() {
        return 0;
    }
    let is_16_bit = (voice_setting & 0x0020) != 0;
    if is_16_bit {
        ALT_SETTING_16_BIT[index]
    } else {
        ALT_SETTING_8_BIT[index]
    }
}
