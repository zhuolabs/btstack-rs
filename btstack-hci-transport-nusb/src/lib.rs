use btstack_sys::{
    hci_transport_config_type_t_HCI_TRANSPORT_CONFIG_USB, hci_transport_t, HCI_ACL_DATA_PACKET,
    HCI_COMMAND_DATA_PACKET, HCI_EVENT_PACKET, HCI_EVENT_TRANSPORT_PACKET_SENT,
    HCI_EVENT_TRANSPORT_USB_INFO, HCI_ISO_DATA_PACKET, HCI_SCO_DATA_PACKET,
};
use futures_lite::future::{block_on, poll_once};
use nusb::transfer::{
    ControlOut, ControlType, Direction, EndpointType, Queue, Recipient, RequestBuffer,
    TransferError,
};
use nusb::Interface;
use std::ffi::c_void;
use std::os::raw::{c_char, c_int};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

const DEFAULT_EVENT_EP: u8 = 0x81;
const DEFAULT_ACL_IN_EP: u8 = 0x82;
const DEFAULT_ACL_OUT_EP: u8 = 0x02;
const EVENT_TRANSFER_SIZE: usize = 260;
const ACL_TRANSFER_SIZE: usize = 2048;
const POLL_INTERVAL: Duration = Duration::from_millis(1);
const OUTGOING_QUEUE_DEPTH: usize = 4;
const USB_MAX_PATH_LEN: usize = 7;
const EVENT_IN_FLIGHT: usize = 3;
const ACL_IN_FLIGHT: usize = 3;
const ACL_OUT_IN_FLIGHT: usize = 4;

const ALT_SETTING_8_BIT: [u8; 3] = [1, 2, 3];
const ALT_SETTING_16_BIT: [u8; 3] = [2, 4, 5];

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
struct UsbSelection {
    vendor_id: Option<u16>,
    product_id: Option<u16>,
    bus_number: Option<u8>,
    path: Option<Vec<u8>>,
}

#[derive(Debug, Copy, Clone)]
struct EndpointConfig {
    event_in: u8,
    acl_in: u8,
    acl_out: u8,
    sco_in: Option<u8>,
    sco_out: Option<u8>,
}

struct ActiveTransport {
    stop: Arc<AtomicBool>,
    outgoing_pending: Arc<std::sync::atomic::AtomicUsize>,
    outgoing_sender: SyncSender<OutgoingPacket>,
    reader_threads: Vec<JoinHandle<()>>,
    writer_thread: JoinHandle<()>,
    outgoing_queue_depth: usize,
    sco: Option<Arc<Mutex<ScoState>>>,
}

enum OutgoingPacket {
    Command(Vec<u8>),
    Acl(Vec<u8>),
    Iso(Vec<u8>),
}

struct ScoState {
    interface: Interface,
    voice_setting: u16,
    num_connections: c_int,
    sco_in_endpoint: Option<u8>,
    sco_out_endpoint: Option<u8>,
}

#[derive(Default)]
struct TransportState {
    packet_handler: Option<unsafe extern "C" fn(packet_type: u8, packet: *mut u8, size: u16)>,
    selected: UsbSelection,
    active: Option<ActiveTransport>,
}

static STATE: Mutex<TransportState> = Mutex::new(TransportState {
    packet_handler: None,
    selected: UsbSelection {
        vendor_id: None,
        product_id: None,
        bus_number: None,
        path: None,
    },
    active: None,
});

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
        path: [0; USB_MAX_PATH_LEN],
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
    emit_usb_info(&device_info);
    let stop = Arc::new(AtomicBool::new(false));
    let outgoing_pending = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let (outgoing_sender, outgoing_receiver) = sync_channel(OUTGOING_QUEUE_DEPTH);
    let mut reader_threads = Vec::with_capacity(2);
    reader_threads.push(spawn_event_reader(
        interface.clone(),
        endpoints.event_in,
        stop.clone(),
    ));
    reader_threads.push(spawn_acl_reader(
        interface.clone(),
        endpoints.acl_in,
        stop.clone(),
    ));
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

fn select_device(selection: UsbSelection) -> Option<nusb::DeviceInfo> {
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

fn detect_endpoints(interface: &Interface, sco_interface: Option<&Interface>) -> EndpointConfig {
    let mut config = EndpointConfig {
        event_in: DEFAULT_EVENT_EP,
        acl_in: DEFAULT_ACL_IN_EP,
        acl_out: DEFAULT_ACL_OUT_EP,
        sco_in: None,
        sco_out: None,
    };
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

fn spawn_event_reader(interface: Interface, endpoint: u8, stop: Arc<AtomicBool>) -> JoinHandle<()> {
    thread::spawn(move || {
        let mut queue = interface.interrupt_in_queue(endpoint);
        for _ in 0..EVENT_IN_FLIGHT {
            queue.submit(RequestBuffer::new(EVENT_TRANSFER_SIZE));
        }

        while queue.pending() > 0 {
            if let Some(completion) = block_on(poll_once(queue.next_complete())) {
                let status = completion.status;
                let data = completion.data;

                if status.is_ok() && !data.is_empty() {
                    emit_packet(HCI_EVENT_PACKET as u8, data.as_slice());
                }
                if matches!(status, Err(TransferError::Stall)) {
                    let _ = queue.clear_halt();
                }

                if stop.load(Ordering::Relaxed) {
                    cancel_and_drain_reader_queue(&mut queue);
                    continue;
                }

                queue.submit(RequestBuffer::reuse(data, EVENT_TRANSFER_SIZE));
            } else {
                thread::sleep(POLL_INTERVAL);
            }
        }
    })
}

fn spawn_acl_reader(interface: Interface, endpoint: u8, stop: Arc<AtomicBool>) -> JoinHandle<()> {
    thread::spawn(move || {
        let mut queue = interface.bulk_in_queue(endpoint);
        for _ in 0..ACL_IN_FLIGHT {
            queue.submit(RequestBuffer::new(ACL_TRANSFER_SIZE));
        }

        while queue.pending() > 0 {
            if let Some(completion) = block_on(poll_once(queue.next_complete())) {
                let status = completion.status;
                let data = completion.data;

                if status.is_ok() && !data.is_empty() {
                    emit_packet(HCI_ACL_DATA_PACKET as u8, data.as_slice());
                }
                if matches!(status, Err(TransferError::Stall)) {
                    let _ = queue.clear_halt();
                }

                if stop.load(Ordering::Relaxed) {
                    cancel_and_drain_reader_queue(&mut queue);
                    continue;
                }

                queue.submit(RequestBuffer::reuse(data, ACL_TRANSFER_SIZE));
            } else {
                thread::sleep(POLL_INTERVAL);
            }
        }
    })
}

fn spawn_writer(
    interface: Interface,
    acl_out_endpoint: u8,
    stop: Arc<AtomicBool>,
    pending: Arc<std::sync::atomic::AtomicUsize>,
    receiver: Receiver<OutgoingPacket>,
) -> JoinHandle<()> {
    thread::spawn(move || {
        let mut acl_out_queue = interface.bulk_out_queue(acl_out_endpoint);
        let mut in_flight_out = 0usize;
        let mut cancelled = false;

        loop {
            while in_flight_out < ACL_OUT_IN_FLIGHT && !stop.load(Ordering::Relaxed) {
                let packet = match receiver.recv_timeout(POLL_INTERVAL) {
                    Ok(packet) => packet,
                    Err(RecvTimeoutError::Timeout) => break,
                    Err(RecvTimeoutError::Disconnected) => return,
                };

                match packet {
                    OutgoingPacket::Command(packet) => {
                        let result = send_hci_command(&interface, &packet);
                        pending.fetch_sub(1, Ordering::Relaxed);
                        if result.is_ok() {
                            emit_transport_packet_sent();
                        }
                    }
                    OutgoingPacket::Acl(packet) | OutgoingPacket::Iso(packet) => {
                        acl_out_queue.submit(packet);
                        in_flight_out += 1;
                    }
                }
            }

            if stop.load(Ordering::Relaxed) && !cancelled {
                acl_out_queue.cancel_all();
                cancelled = true;
            }

            if in_flight_out == 0 {
                if stop.load(Ordering::Relaxed) {
                    break;
                }
                continue;
            }

            if let Some(completion) = block_on(poll_once(acl_out_queue.next_complete())) {
                in_flight_out -= 1;
                pending.fetch_sub(1, Ordering::Relaxed);
                match completion.status {
                    Ok(()) => emit_transport_packet_sent(),
                    Err(TransferError::Stall) => {
                        let _ = acl_out_queue.clear_halt();
                    }
                    Err(_) => {}
                }
            }
        }
    })
}

fn send_hci_command(interface: &Interface, packet: &[u8]) -> Result<(), ()> {
    let transfer = interface.control_out(ControlOut {
        control_type: ControlType::Class,
        recipient: Recipient::Device,
        request: 0,
        value: 0,
        index: 0,
        data: packet,
    });

    block_on(transfer).into_result().map(|_| ()).map_err(|_| ())
}

fn emit_transport_packet_sent() {
    let event = [HCI_EVENT_TRANSPORT_PACKET_SENT as u8, 0];
    emit_packet(HCI_EVENT_PACKET as u8, &event);
}

fn emit_usb_info(device: &nusb::DeviceInfo) {
    let path = usb_path(device).unwrap_or_default();
    let mut event = Vec::with_capacity(8 + path.len());
    event.push(HCI_EVENT_TRANSPORT_USB_INFO as u8);
    event.push((6 + path.len()) as u8);
    event.extend_from_slice(&device.vendor_id().to_le_bytes());
    event.extend_from_slice(&device.product_id().to_le_bytes());
    event.push(device.bus_number());
    event.push(path.len() as u8);
    event.extend_from_slice(&path);
    emit_packet(HCI_EVENT_PACKET as u8, &event);
}

fn usb_path(device: &nusb::DeviceInfo) -> Option<Vec<u8>> {
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

fn cancel_and_drain_reader_queue(queue: &mut Queue<RequestBuffer>) {
    queue.cancel_all();
    while queue.pending() > 0 {
        let _ = block_on(queue.next_complete());
    }
}

fn endpoint_sco_state(
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

fn sco_alt_setting(voice_setting: u16, num_connections: c_int) -> u8 {
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

fn emit_packet(packet_type: u8, packet: &[u8]) {
    let handler = {
        let state = STATE.lock().expect("state lock poisoned");
        state.packet_handler
    };

    if let Some(handler) = handler {
        let mut owned = packet.to_vec();
        unsafe { handler(packet_type, owned.as_mut_ptr(), owned.len() as u16) };
    }
}

fn try_reserve_send_slot(pending: &std::sync::atomic::AtomicUsize, queue_depth: usize) -> bool {
    let mut current = pending.load(Ordering::Relaxed);
    loop {
        if current >= queue_depth {
            return false;
        }
        match pending.compare_exchange_weak(
            current,
            current + 1,
            Ordering::Relaxed,
            Ordering::Relaxed,
        ) {
            Ok(_) => return true,
            Err(actual) => current = actual,
        }
    }
}
