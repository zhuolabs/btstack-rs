use crate::constants::{
    ACL_IN_FLIGHT, ACL_OUT_IN_FLIGHT, ACL_TRANSFER_SIZE, EVENT_IN_FLIGHT, EVENT_TRANSFER_SIZE,
    POLL_INTERVAL,
};
use crate::state::STATE;
use crate::types::OutgoingPacket;
use btstack_sys::{
    HCI_ACL_DATA_PACKET, HCI_EVENT_PACKET, HCI_EVENT_TRANSPORT_PACKET_SENT,
    HCI_EVENT_TRANSPORT_USB_INFO,
};
use futures_lite::future::{block_on, poll_once};
use nusb::transfer::{ControlOut, ControlType, Queue, Recipient, RequestBuffer, TransferError};
use nusb::Interface;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

pub(crate) fn spawn_event_reader(
    interface: Interface,
    endpoint: u8,
    stop: Arc<AtomicBool>,
) -> JoinHandle<()> {
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

pub(crate) fn spawn_acl_reader(
    interface: Interface,
    endpoint: u8,
    stop: Arc<AtomicBool>,
) -> JoinHandle<()> {
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

pub(crate) fn spawn_writer(
    interface: Interface,
    acl_out_endpoint: u8,
    stop: Arc<AtomicBool>,
    pending: Arc<AtomicUsize>,
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

fn cancel_and_drain_reader_queue(queue: &mut Queue<RequestBuffer>) {
    queue.cancel_all();
    while queue.pending() > 0 {
        let _ = block_on(queue.next_complete());
    }
}

pub(crate) fn emit_transport_packet_sent() {
    let event = [HCI_EVENT_TRANSPORT_PACKET_SENT as u8, 0];
    emit_packet(HCI_EVENT_PACKET as u8, &event);
}

pub(crate) fn emit_usb_info(device: &nusb::DeviceInfo, path: &[u8]) {
    let mut event = Vec::with_capacity(8 + path.len());
    event.push(HCI_EVENT_TRANSPORT_USB_INFO as u8);
    event.push((6 + path.len()) as u8);
    event.extend_from_slice(&device.vendor_id().to_le_bytes());
    event.extend_from_slice(&device.product_id().to_le_bytes());
    event.push(device.bus_number());
    event.push(path.len() as u8);
    event.extend_from_slice(path);
    emit_packet(HCI_EVENT_PACKET as u8, &event);
}

pub(crate) fn emit_packet(packet_type: u8, packet: &[u8]) {
    let handler = {
        let state = STATE.lock().expect("state lock poisoned");
        state.packet_handler
    };

    if let Some(handler) = handler {
        let mut owned = packet.to_vec();
        unsafe { handler(packet_type, owned.as_mut_ptr(), owned.len() as u16) };
    }
}

pub(crate) fn try_reserve_send_slot(pending: &AtomicUsize, queue_depth: usize) -> bool {
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
