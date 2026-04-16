mod constants;
mod io;
mod state;
mod types;
mod usb;

use btstack_sys::{
    hci_transport_config_type_t_HCI_TRANSPORT_CONFIG_USB, hci_transport_t, HCI_ACL_DATA_PACKET,
    HCI_COMMAND_DATA_PACKET, HCI_EVENT_PACKET, HCI_ISO_DATA_PACKET, HCI_SCO_DATA_PACKET,
};
use io::{
    emit_usb_info, spawn_acl_reader, spawn_event_reader, spawn_writer, try_reserve_send_slot,
};
use state::STATE;
use std::ffi::c_void;
use std::os::raw::{c_char, c_int};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{sync_channel, TrySendError};
use std::sync::Arc;
use types::{hci_transport_config_nusb_t, ActiveTransport, OutgoingPacket, UsbSelection};
use usb::{detect_endpoints, endpoint_sco_state, sco_alt_setting, select_device, usb_path};

use crate::constants::OUTGOING_QUEUE_DEPTH;

static TRANSPORT_NAME: &[u8] = b"H2 nusb\0";

#[allow(non_upper_case_globals)]
static mut HCI_TRANSPORT_NUSB: hci_transport_t = hci_transport_t {
    name: TRANSPORT_NAME.as_ptr() as *const c_char,
    init: Some(transport_init),
    open: Some(transport_open),
    close: Some(transport_close),
    register_packet_handler: Some(transport_register_packet_handler),
    can_send_packet_now: Some(transport_can_send_packet_now),
    send_packet: Some(transport_send_packet),
    set_baudrate: None,
    reset_link: None,
    set_sco_config: Some(transport_set_sco_config),
};

pub fn hci_transport_nusb_instance() -> *const hci_transport_t {
    std::ptr::addr_of!(HCI_TRANSPORT_NUSB)
}

pub fn default_config(vendor_id: u16, product_id: u16) -> hci_transport_config_nusb_t {
    hci_transport_config_nusb_t {
        type_: hci_transport_config_type_t_HCI_TRANSPORT_CONFIG_USB,
        vendor_id,
        product_id,
        bus_number: 0,
        path_len: 0,
        path: [0; constants::USB_MAX_PATH_LEN],
    }
}

unsafe extern "C" fn transport_init(config: *const c_void) {
    let mut state = STATE.lock().expect("state lock poisoned");
    state.selected = UsbSelection::default();

    if config.is_null() {
        return;
    }

    let config = &*(config as *const hci_transport_config_nusb_t);
    if config.vendor_id != 0 {
        state.selected.vendor_id = Some(config.vendor_id);
    }
    if config.product_id != 0 {
        state.selected.product_id = Some(config.product_id);
    }
    if config.bus_number != 0 {
        state.selected.bus_number = Some(config.bus_number);
    }
    if config.path_len as usize <= config.path.len() && config.path_len > 0 {
        state.selected.path = Some(config.path[..config.path_len as usize].to_vec());
    }
}

unsafe extern "C" fn transport_open() -> c_int {
    let selection = {
        let state = STATE.lock().expect("state lock poisoned");
        if state.active.is_some() {
            return 0;
        }
        state.selected.clone()
    };

    let device_info = match select_device(selection) {
        Some(device) => device,
        None => return -1,
    };

    let device = match device_info.open() {
        Ok(device) => device,
        Err(_) => return -1,
    };

    let interface = match device.claim_interface(0) {
        Ok(interface) => interface,
        Err(_) => return -1,
    };

    let sco_interface = device.claim_interface(1).ok();
    let endpoints = detect_endpoints(&interface, sco_interface.as_ref());
    let path = usb_path(&device_info).unwrap_or_default();
    emit_usb_info(&device_info, &path);
    let stop = Arc::new(AtomicBool::new(false));
    let outgoing_pending = Arc::new(AtomicUsize::new(0));
    let (outgoing_sender, outgoing_receiver) = sync_channel(OUTGOING_QUEUE_DEPTH);
    let reader_threads = vec![
        spawn_event_reader(interface.clone(), endpoints.event_in, stop.clone()),
        spawn_acl_reader(interface.clone(), endpoints.acl_in, stop.clone()),
    ];
    let writer_thread = spawn_writer(
        interface.clone(),
        endpoints.acl_out,
        stop.clone(),
        outgoing_pending.clone(),
        outgoing_receiver,
    );

    let mut state = STATE.lock().expect("state lock poisoned");
    state.active = Some(ActiveTransport {
        stop,
        outgoing_pending,
        outgoing_sender,
        reader_threads,
        writer_thread,
        outgoing_queue_depth: OUTGOING_QUEUE_DEPTH,
        sco: endpoint_sco_state(sco_interface.as_ref(), &endpoints),
    });

    0
}

unsafe extern "C" fn transport_close() -> c_int {
    let active = {
        let mut state = STATE.lock().expect("state lock poisoned");
        state.active.take()
    };

    if let Some(mut active) = active {
        active.stop.store(true, Ordering::Relaxed);
        for handle in active.reader_threads.drain(..) {
            let _ = handle.join();
        }
        let _ = active.writer_thread.join();
    }

    0
}

unsafe extern "C" fn transport_register_packet_handler(
    handler: Option<unsafe extern "C" fn(packet_type: u8, packet: *mut u8, size: u16)>,
) {
    let mut state = STATE.lock().expect("state lock poisoned");
    state.packet_handler = handler;
}

unsafe extern "C" fn transport_can_send_packet_now(packet_type: u8) -> c_int {
    if packet_type != HCI_COMMAND_DATA_PACKET as u8
        && packet_type != HCI_ACL_DATA_PACKET as u8
        && packet_type != HCI_ISO_DATA_PACKET as u8
    {
        return 0;
    }

    let state = STATE.lock().expect("state lock poisoned");
    let Some(active) = state.active.as_ref() else {
        return 0;
    };
    if active.outgoing_pending.load(Ordering::Relaxed) < active.outgoing_queue_depth {
        1
    } else {
        0
    }
}

unsafe extern "C" fn transport_send_packet(packet_type: u8, packet: *mut u8, size: c_int) -> c_int {
    if packet.is_null() || size < 0 {
        return -1;
    }

    let (sender, pending, queue_depth) = {
        let state = STATE.lock().expect("state lock poisoned");
        let Some(active) = state.active.as_ref() else {
            return -1;
        };
        (
            active.outgoing_sender.clone(),
            active.outgoing_pending.clone(),
            active.outgoing_queue_depth,
        )
    };

    if !try_reserve_send_slot(&pending, queue_depth) {
        return -1;
    }

    let bytes = std::slice::from_raw_parts(packet, size as usize).to_vec();
    let outgoing = match packet_type {
        t if t == HCI_COMMAND_DATA_PACKET as u8 => OutgoingPacket::Command(bytes),
        t if t == HCI_ACL_DATA_PACKET as u8 => OutgoingPacket::Acl(bytes),
        t if t == HCI_ISO_DATA_PACKET as u8 => OutgoingPacket::Iso(bytes),
        t if t == HCI_SCO_DATA_PACKET as u8 => {
            pending.fetch_sub(1, Ordering::Relaxed);
            return -1;
        }
        _ => {
            pending.fetch_sub(1, Ordering::Relaxed);
            return -1;
        }
    };

    match sender.try_send(outgoing) {
        Ok(()) => 0,
        Err(TrySendError::Full(_)) | Err(TrySendError::Disconnected(_)) => {
            pending.fetch_sub(1, Ordering::Relaxed);
            -1
        }
    }
}

unsafe extern "C" fn transport_set_sco_config(voice_setting: u16, num_connections: c_int) {
    let sco = {
        let state = STATE.lock().expect("state lock poisoned");
        state.active.as_ref().and_then(|active| active.sco.clone())
    };
    let Some(sco) = sco else {
        return;
    };

    let mut sco = sco.lock().expect("sco lock poisoned");
    let alt_setting = sco_alt_setting(voice_setting, num_connections);
    if alt_setting == 0 {
        let _ = sco.interface.set_alt_setting(0);
    } else if sco.sco_in_endpoint.is_some() && sco.sco_out_endpoint.is_some() {
        let _ = sco.interface.set_alt_setting(alt_setting);
    }
    sco.voice_setting = voice_setting;
    sco.num_connections = num_connections;
}
