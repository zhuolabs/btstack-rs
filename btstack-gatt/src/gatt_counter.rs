//! Rust translation of BTstack's `example/gatt_counter.c`.
//!
//! The module intentionally focuses on the BLE path only (no GATT-over-Classic code).
//! It provides setup and callback wiring, but no application `main` function.

use std::ffi::c_int;

use btstack_sys::{
    att_read_callback_handle_blob, att_server_init, att_server_notify,
    att_server_register_packet_handler, att_server_request_can_send_now_event,
    btstack_packet_callback_registration_t, btstack_run_loop_add_timer, btstack_run_loop_set_timer,
    btstack_snprintf_assert_complete, btstack_timer_source_t, gap_advertisements_enable,
    gap_advertisements_set_data, gap_advertisements_set_params, hci_add_event_handler,
    hci_con_handle_t, hci_power_control, l2cap_init, sm_init, ATT_EVENT_CAN_SEND_NOW,
    HCI_EVENT_DISCONNECTION_COMPLETE, HCI_EVENT_PACKET, HCI_POWER_MODE_HCI_POWER_ON,
};

const HEARTBEAT_PERIOD_MS: u32 = 1_000;
const APP_AD_FLAGS: u8 = 0x06;
const ATT_COUNTER_VALUE_HANDLE: u16 = 0x000b;
const ATT_COUNTER_CLIENT_CONFIGURATION_HANDLE: u16 = 0x000c;
const GATT_CLIENT_CHARACTERISTICS_CONFIGURATION_NOTIFICATION: u16 = 0x0001;
const ADV_DATA: [u8; 19] = [
    0x02,
    0x01,
    APP_AD_FLAGS,
    0x0b,
    0x09,
    b'L',
    b'E',
    b' ',
    b'C',
    b'o',
    b'u',
    b'n',
    b't',
    b'e',
    b'r',
    0x03,
    0x02,
    0x10,
    0xff,
];

// Generated from btstack/example/gatt_counter.gatt.
const PROFILE_DATA: [u8; 199] = [
    1, 10, 0, 2, 0, 1, 0, 0, 40, 0, 24, 13, 0, 2, 0, 2, 0, 3, 40, 2, 3, 0, 0, 42, 20, 0, 2, 0, 3,
    0, 0, 42, 71, 65, 84, 84, 32, 67, 111, 117, 110, 116, 101, 114, 13, 0, 2, 0, 4, 0, 3, 40, 2, 5,
    0, 1, 42, 10, 0, 2, 0, 5, 0, 1, 42, 131, 0, 10, 0, 2, 0, 6, 0, 0, 40, 1, 24, 13, 0, 2, 0, 7, 0,
    3, 40, 2, 8, 0, 42, 43, 24, 0, 2, 0, 8, 0, 42, 43, 144, 46, 56, 175, 191, 20, 18, 1, 110, 186,
    244, 184, 221, 87, 181, 172, 24, 0, 2, 0, 9, 0, 0, 40, 251, 52, 155, 95, 128, 0, 0, 128, 0, 16,
    0, 0, 16, 255, 0, 0, 27, 0, 2, 0, 10, 0, 3, 40, 26, 11, 0, 251, 52, 155, 95, 128, 0, 0, 128, 0,
    16, 0, 0, 17, 255, 0, 0, 22, 0, 10, 3, 11, 0, 251, 52, 155, 95, 128, 0, 0, 128, 0, 16, 0, 0,
    17, 255, 0, 0, 10, 0, 14, 1, 12, 0, 2, 41, 0, 0, 0, 0,
];

static mut NOTIFICATION_ENABLED: bool = false;
static mut CONNECTION_HANDLE: hci_con_handle_t = 0;
static mut COUNTER: u32 = 0;
const COUNTER_BUFFER_CAPACITY: usize = 30;
static mut COUNTER_BUFFER: [u8; COUNTER_BUFFER_CAPACITY] = [0; COUNTER_BUFFER_CAPACITY];
static mut COUNTER_LEN: u16 = 0;
static mut HEARTBEAT_TIMER: btstack_timer_source_t = btstack_timer_source_t {
    item: btstack_sys::btstack_linked_item_t {
        next: std::ptr::null_mut(),
    },
    timeout: 0,
    process: None,
    context: std::ptr::null_mut(),
};
static mut HCI_EVENT_CB: btstack_packet_callback_registration_t =
    btstack_packet_callback_registration_t {
        item: btstack_sys::btstack_linked_item_t {
            next: std::ptr::null_mut(),
        },
        callback: None,
    };

/// Sets up BLE GATT counter state and registers BTstack callbacks.
///
/// This function only wires the example logic. A host application still has to
/// initialize the BTstack run loop and power on the controller.
pub fn setup() {
    unsafe {
        l2cap_init();
        sm_init();

        att_server_init(
            PROFILE_DATA.as_ptr(),
            Some(att_read_callback),
            Some(att_write_callback),
        );

        let mut null_addr = [0u8; 6];
        gap_advertisements_set_params(0x0030, 0x0030, 0, 0, null_addr.as_mut_ptr(), 0x07, 0x00);
        gap_advertisements_set_data(ADV_DATA.len() as u8, ADV_DATA.as_ptr() as *mut u8);
        gap_advertisements_enable(1);

        HCI_EVENT_CB.callback = Some(packet_handler);
        hci_add_event_handler(std::ptr::addr_of_mut!(HCI_EVENT_CB));
        att_server_register_packet_handler(Some(packet_handler));

        HEARTBEAT_TIMER.process = Some(heartbeat_handler);
        btstack_run_loop_set_timer(std::ptr::addr_of_mut!(HEARTBEAT_TIMER), HEARTBEAT_PERIOD_MS);
        btstack_run_loop_add_timer(std::ptr::addr_of_mut!(HEARTBEAT_TIMER));

        beat();
    }
}

/// Entry point equivalent to BTstack C examples' `btstack_main`.
///
/// It configures the example and powers on the controller.
#[no_mangle]
pub extern "C" fn btstack_main(_argc: c_int, _argv: *const *const i8) -> c_int {
    setup();
    unsafe {
        hci_power_control(HCI_POWER_MODE_HCI_POWER_ON);
    }
    0
}

unsafe extern "C" fn heartbeat_handler(ts: *mut btstack_timer_source_t) {
    if NOTIFICATION_ENABLED {
        beat();
        att_server_request_can_send_now_event(CONNECTION_HANDLE);
    }

    btstack_run_loop_set_timer(ts, HEARTBEAT_PERIOD_MS);
    btstack_run_loop_add_timer(ts);
}

unsafe extern "C" fn packet_handler(packet_type: u8, _channel: u16, packet: *mut u8, _size: u16) {
    if packet_type != HCI_EVENT_PACKET as u8 || packet.is_null() {
        return;
    }

    match *packet {
        x if x == HCI_EVENT_DISCONNECTION_COMPLETE as u8 => {
            NOTIFICATION_ENABLED = false;
        }
        x if x == ATT_EVENT_CAN_SEND_NOW as u8 => {
            let _ = att_server_notify(
                CONNECTION_HANDLE,
                ATT_COUNTER_VALUE_HANDLE,
                std::ptr::addr_of!(COUNTER_BUFFER[0]),
                COUNTER_LEN,
            );
        }
        _ => {}
    }
}

unsafe extern "C" fn att_read_callback(
    _connection_handle: hci_con_handle_t,
    att_handle: u16,
    offset: u16,
    buffer: *mut u8,
    buffer_size: u16,
) -> u16 {
    if att_handle == ATT_COUNTER_VALUE_HANDLE {
        return att_read_callback_handle_blob(
            std::ptr::addr_of!(COUNTER_BUFFER[0]),
            COUNTER_LEN,
            offset,
            buffer,
            buffer_size,
        );
    }
    0
}

unsafe extern "C" fn att_write_callback(
    connection_handle: hci_con_handle_t,
    att_handle: u16,
    _transaction_mode: u16,
    _offset: u16,
    buffer: *mut u8,
    buffer_size: u16,
) -> c_int {
    if buffer.is_null() || buffer_size == 0 {
        return 0;
    }

    match att_handle {
        ATT_COUNTER_CLIENT_CONFIGURATION_HANDLE => {
            let config = (*buffer as u16) | ((*buffer.add(1) as u16) << 8);
            NOTIFICATION_ENABLED = config == GATT_CLIENT_CHARACTERISTICS_CONFIGURATION_NOTIFICATION;
            CONNECTION_HANDLE = connection_handle;
        }
        ATT_COUNTER_VALUE_HANDLE => {
            // The original C sample prints incoming data; this library variant keeps the hook minimal.
        }
        _ => {}
    }

    0
}

unsafe fn beat() {
    COUNTER = COUNTER.wrapping_add(1);
    COUNTER_LEN = btstack_snprintf_assert_complete(
        std::ptr::addr_of_mut!(COUNTER_BUFFER[0]) as *mut i8,
        COUNTER_BUFFER_CAPACITY,
        b"BTstack counter %04u\0".as_ptr() as *const i8,
        COUNTER,
    );
}
