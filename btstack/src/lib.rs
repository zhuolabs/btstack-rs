//! Safe wrappers around selected `btstack-sys` APIs used by BTstack's `port/libusb/main.c`.

use std::ffi::CString;
use std::fmt;
use std::mem::{self, MaybeUninit};

pub const PACKET_LOG_FORMAT: btstack_sys::hci_dump_format_t =
    btstack_sys::hci_dump_format_t_HCI_DUMP_PACKETLOGGER;

#[derive(Debug)]
pub enum BtstackError {
    InteriorNul(std::ffi::NulError),
    ValueOutOfRange(&'static str),
    Failed(&'static str, i32),
}

impl fmt::Display for BtstackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InteriorNul(err) => write!(f, "string contains interior NUL: {err}"),
            Self::ValueOutOfRange(name) => write!(f, "value out of range: {name}"),
            Self::Failed(name, code) => write!(f, "{name} failed with code {code}"),
        }
    }
}

impl std::error::Error for BtstackError {}

impl From<std::ffi::NulError> for BtstackError {
    fn from(value: std::ffi::NulError) -> Self {
        Self::InteriorNul(value)
    }
}

pub type PacketHandler =
    unsafe extern "C" fn(packet_type: u8, channel: u16, packet: *mut u8, size: u16);
pub type SignalHandler = unsafe extern "C" fn();

#[derive(Debug, Clone)]
pub struct EventHandlerRegistration {
    inner: btstack_sys::btstack_packet_callback_registration_t,
}

impl EventHandlerRegistration {
    pub fn new(callback: PacketHandler) -> Self {
        let mut inner: btstack_sys::btstack_packet_callback_registration_t =
            unsafe { mem::zeroed() };
        inner.callback = Some(callback);
        Self { inner }
    }

    pub fn as_mut_ptr(&mut self) -> *mut btstack_sys::btstack_packet_callback_registration_t {
        &mut self.inner
    }
}

pub fn memory_init() {
    unsafe { btstack_sys::btstack_memory_init() }
}

pub fn init_posix_run_loop() {
    unsafe {
        let run_loop = btstack_sys::btstack_run_loop_posix_get_instance();
        btstack_sys::btstack_run_loop_init(run_loop);
    }
}

pub fn set_usb_bus_and_path(bus: u8, path: &[u8]) -> Result<(), BtstackError> {
    let len =
        i32::try_from(path.len()).map_err(|_| BtstackError::ValueOutOfRange("path length"))?;
    unsafe {
        btstack_sys::hci_transport_usb_set_bus_and_path(bus, len, path.as_ptr().cast_mut());
    }
    Ok(())
}

pub fn open_packet_log(path: &str) -> Result<(), BtstackError> {
    let path = CString::new(path)?;
    let rc = unsafe { btstack_sys::hci_dump_posix_fs_open(path.as_ptr(), PACKET_LOG_FORMAT) };
    if rc != 0 {
        return Err(BtstackError::Failed("hci_dump_posix_fs_open", rc));
    }

    unsafe {
        let hci_dump_impl = btstack_sys::hci_dump_posix_fs_get_instance();
        btstack_sys::hci_dump_init(hci_dump_impl);
    }

    Ok(())
}

pub fn init_usb_hci() {
    unsafe {
        btstack_sys::hci_init(btstack_sys::hci_transport_usb_instance(), std::ptr::null());
    }
}

pub fn add_event_handler(registration: &mut EventHandlerRegistration) {
    unsafe {
        btstack_sys::hci_add_event_handler(registration.as_mut_ptr());
    }
}

pub fn register_signal_callback(signal: i32, callback: SignalHandler) {
    unsafe {
        btstack_sys::btstack_signal_register_callback(signal, Some(callback));
    }
}

pub fn register_realtek_usb_devices() {
    unsafe {
        let total = btstack_sys::btstack_chipset_realtek_get_num_usb_controllers();
        for index in 0..total {
            let mut vendor_id = 0;
            let mut product_id = 0;
            btstack_sys::btstack_chipset_realtek_get_vendor_product_id(
                index,
                &mut vendor_id,
                &mut product_id,
            );
            btstack_sys::hci_transport_usb_add_device(vendor_id, product_id);
        }
    }
}

pub fn configure_realtek_chipset(product_id: u16) {
    unsafe {
        btstack_sys::btstack_chipset_realtek_set_product_id(product_id);
        btstack_sys::hci_set_chipset(btstack_sys::btstack_chipset_realtek_instance());
        btstack_sys::hci_enable_custom_pre_init();
    }
}

pub fn configure_zephyr_chipset() {
    unsafe {
        btstack_sys::hci_set_chipset(btstack_sys::btstack_chipset_zephyr_instance());
        btstack_sys::sm_init();
    }
}

pub fn local_bd_addr() -> [u8; 6] {
    let mut addr = [0u8; 6];
    unsafe { btstack_sys::gap_local_bd_addr(addr.as_mut_ptr()) }
    addr
}

pub fn set_random_address(addr: [u8; 6]) {
    unsafe { btstack_sys::gap_random_address_set(addr.as_ptr()) }
}

pub fn power_off() -> Result<(), BtstackError> {
    let rc = unsafe { btstack_sys::hci_power_control(btstack_sys::HCI_POWER_MODE_HCI_POWER_OFF) };
    if rc != 0 {
        return Err(BtstackError::Failed("hci_power_control", rc));
    }
    Ok(())
}

pub fn run_loop_execute() {
    unsafe { btstack_sys::btstack_run_loop_execute() }
}

pub struct TlvStore {
    context: btstack_sys::btstack_tlv_posix_t,
    tlv_impl: *const btstack_sys::btstack_tlv_t,
    active: bool,
}

impl TlvStore {
    pub fn open(path: &str) -> Result<Self, BtstackError> {
        let path = CString::new(path)?;
        let mut context = MaybeUninit::<btstack_sys::btstack_tlv_posix_t>::zeroed();
        let tlv_impl = unsafe {
            btstack_sys::btstack_tlv_posix_init_instance(context.as_mut_ptr(), path.as_ptr())
        };
        let context = unsafe { context.assume_init() };

        if tlv_impl.is_null() {
            return Err(BtstackError::Failed("btstack_tlv_posix_init_instance", -1));
        }

        Ok(Self {
            context,
            tlv_impl,
            active: true,
        })
    }

    pub fn install_as_global(&mut self) {
        unsafe {
            btstack_sys::btstack_tlv_set_instance(
                self.tlv_impl,
                (&mut self.context as *mut _) as *mut _,
            );
        }
    }

    pub fn configure_le_device_db(&mut self) {
        unsafe {
            btstack_sys::le_device_db_tlv_configure(
                self.tlv_impl,
                (&mut self.context as *mut _) as *mut _,
            );
        }
    }
}

impl Drop for TlvStore {
    fn drop(&mut self) {
        if self.active {
            unsafe { btstack_sys::btstack_tlv_posix_deinit(&mut self.context) }
        }
    }
}
