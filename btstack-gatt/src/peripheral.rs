//! Local GATT peripheral server abstraction built on top of BTstack primitives.

use std::collections::HashMap;
use std::ffi::c_int;

use crate::runtime::BtstackRuntime;

use btstack_sys::{
    att_read_callback_handle_blob, att_server_init, att_server_register_packet_handler,
    btstack_packet_callback_registration_t, gap_advertisements_enable, gap_advertisements_set_data,
    gap_advertisements_set_params,
    gatt_server_get_client_configuration_handle_for_characteristic_with_uuid128,
    gatt_server_get_client_configuration_handle_for_characteristic_with_uuid16,
    gatt_server_get_handle_range_for_service_with_uuid128,
    gatt_server_get_handle_range_for_service_with_uuid16,
    gatt_server_get_value_handle_for_characteristic_with_uuid128,
    gatt_server_get_value_handle_for_characteristic_with_uuid16, hci_add_event_handler,
    hci_con_handle_t, l2cap_init, sm_init, ATT_EVENT_CONNECTED, ATT_EVENT_DISCONNECTED,
    ATT_PROPERTY_AUTHENTICATED_SIGNED_WRITE, ATT_PROPERTY_BROADCAST, ATT_PROPERTY_DYNAMIC,
    ATT_PROPERTY_EXTENDED_PROPERTIES, ATT_PROPERTY_INDICATE, ATT_PROPERTY_NOTIFY,
    ATT_PROPERTY_READ, ATT_PROPERTY_WRITE, ATT_PROPERTY_WRITE_WITHOUT_RESPONSE,
    ATT_SECURITY_AUTHENTICATED, ATT_SECURITY_AUTHENTICATED_SC, ATT_SECURITY_AUTHORIZED,
    ATT_SECURITY_ENCRYPTED, ATT_SECURITY_NONE, HCI_EVENT_PACKET,
};

/// Errors surfaced by [`GattPeripheralServer`] during setup.
///
/// The API intentionally keeps this enum small for now:
/// - discovery failures (service/characteristic lookup),
/// - and singleton lifecycle violations.
#[derive(Debug)]
pub enum GattPeripheralError {
    ServerAlreadyInitialized,
    ServiceNotFound,
    CharacteristicNotFound,
}

/// UUID type used by service/characteristic specifications.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GattUuid {
    Uuid16(u16),
    Uuid128([u8; 16]),
}

/// Characteristic property bitmask mapped to BTstack ATT property flags.
///
/// This mirrors WinRT-like high-level flags while keeping a direct mapping
/// to BTstack constants internally.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GattCharacteristicProperties(u16);

impl GattCharacteristicProperties {
    pub const READ: Self = Self(ATT_PROPERTY_READ as u16);
    pub const WRITE: Self = Self(ATT_PROPERTY_WRITE as u16);
    pub const WRITE_WITHOUT_RESPONSE: Self = Self(ATT_PROPERTY_WRITE_WITHOUT_RESPONSE as u16);
    pub const NOTIFY: Self = Self(ATT_PROPERTY_NOTIFY as u16);
    pub const INDICATE: Self = Self(ATT_PROPERTY_INDICATE as u16);
    pub const BROADCAST: Self = Self(ATT_PROPERTY_BROADCAST as u16);
    pub const AUTHENTICATED_SIGNED_WRITE: Self =
        Self(ATT_PROPERTY_AUTHENTICATED_SIGNED_WRITE as u16);
    pub const EXTENDED: Self = Self(ATT_PROPERTY_EXTENDED_PROPERTIES as u16);
    pub const DYNAMIC: Self = Self(ATT_PROPERTY_DYNAMIC as u16);

    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn bits(self) -> u16 {
        self.0
    }

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

impl std::ops::BitOr for GattCharacteristicProperties {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

/// Characteristic access permission level mapped to BTstack ATT security modes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GattCharacteristicPermissions {
    None,
    Encrypted,
    Authenticated,
    Authorized,
    AuthenticatedSecureConnections,
}

impl GattCharacteristicPermissions {
    pub const fn to_btstack_security(self) -> u16 {
        match self {
            Self::None => ATT_SECURITY_NONE as u16,
            Self::Encrypted => ATT_SECURITY_ENCRYPTED as u16,
            Self::Authenticated => ATT_SECURITY_AUTHENTICATED as u16,
            Self::Authorized => ATT_SECURITY_AUTHORIZED as u16,
            Self::AuthenticatedSecureConnections => ATT_SECURITY_AUTHENTICATED_SC as u16,
        }
    }
}

/// Public characteristic description used to bind app intent to ATT database entries.
#[derive(Clone, Debug)]
pub struct GattCharacteristicSpec {
    pub uuid: GattUuid,
    pub properties: GattCharacteristicProperties,
    pub permissions: GattCharacteristicPermissions,
    pub initial_value: Vec<u8>,
}

/// Public service description that groups characteristic specifications.
#[derive(Clone, Debug)]
pub struct GattServiceSpec {
    pub uuid: GattUuid,
    pub characteristics: Vec<GattCharacteristicSpec>,
}

/// GAP advertising configuration used by [`GattPeripheralServer::start_advertising`].
#[derive(Clone, Debug)]
pub struct AdvertisingConfig {
    pub interval_min: u16,
    pub interval_max: u16,
    pub adv_type: u8,
    pub channel_map: u8,
    pub filter_policy: u8,
    pub data: Vec<u8>,
}

impl Default for AdvertisingConfig {
    fn default() -> Self {
        Self {
            interval_min: 0x0030,
            interval_max: 0x0030,
            adv_type: 0,
            channel_map: 0x07,
            filter_policy: 0,
            data: Vec::new(),
        }
    }
}

/// Full server bootstrap specification.
///
/// `profile_data` is the compiled ATT database blob generated from a `.gatt` file.
/// `services` and `characteristics` are used only for handle resolution and metadata,
/// so callers never need to expose raw handles in application code.
#[derive(Clone, Debug)]
pub struct GattPeripheralSpec {
    pub profile_data: Vec<u8>,
    pub services: Vec<GattServiceSpec>,
    pub advertising: AdvertisingConfig,
}

/// Internal runtime metadata for a single characteristic.
#[derive(Debug)]
struct RuntimeCharacteristic {
    spec: GattCharacteristicSpec,
    value_handle: u16,
    ccc_handle: Option<u16>,
    value: Vec<u8>,
}

/// Process-global runtime storage.
///
/// BTstack callbacks are C-style globals without user context pointers, so we keep
/// exactly one active server runtime and route callbacks through this static state.
#[derive(Debug)]
struct RuntimeState {
    characteristics_by_value_handle: HashMap<u16, RuntimeCharacteristic>,
    ccc_to_value_handle: HashMap<u16, u16>,
    notification_enabled: HashMap<u16, bool>,
    _connected_handle: Option<hci_con_handle_t>,
    advertising: AdvertisingConfig,
    hci_event_cb: btstack_packet_callback_registration_t,
}

static mut RUNTIME: Option<RuntimeState> = None;

/// High-level local GATT server bound to a running [`BtstackRuntime`].
///
/// Lifecycle requirements:
/// - Call [`BtstackRuntime::start`](crate::runtime::BtstackRuntime::start) first.
/// - Create the server with [`GattPeripheralServer::new`].
/// - Call [`start_advertising`](Self::start_advertising) after successful construction.
///
/// Shutdown behavior:
/// - [`stop`](Self::stop) disables advertisements only.
/// - Dropping the runtime owner triggers BTstack run-loop shutdown; this server does
///   not own global BTstack teardown.
pub struct GattPeripheralServer<'runtime> {
    _runtime: &'runtime BtstackRuntime,
}

impl<'runtime> GattPeripheralServer<'runtime> {
    /// Initialize ATT server wiring and build internal metadata from public specs.
    ///
    /// This can only be called with an active [`BtstackRuntime`] owner.
    pub fn new(
        runtime: &'runtime BtstackRuntime,
        spec: GattPeripheralSpec,
    ) -> Result<Self, GattPeripheralError> {
        unsafe {
            if RUNTIME.is_some() {
                return Err(GattPeripheralError::ServerAlreadyInitialized);
            }

            l2cap_init();
            sm_init();
            att_server_init(
                spec.profile_data.as_ptr(),
                Some(att_read_callback),
                Some(att_write_callback),
            );
            att_server_register_packet_handler(Some(packet_handler));

            // Resolve all service/characteristic metadata immediately after ATT init.
            // This keeps raw handle usage fully internal to the server implementation.
            let runtime_state = RuntimeState {
                characteristics_by_value_handle: resolve_characteristics(&spec.services)?,
                ccc_to_value_handle: HashMap::new(),
                notification_enabled: HashMap::new(),
                _connected_handle: None,
                advertising: spec.advertising,
                hci_event_cb: btstack_packet_callback_registration_t {
                    item: btstack_sys::btstack_linked_item_t {
                        next: std::ptr::null_mut(),
                    },
                    callback: Some(packet_handler),
                },
            };

            RUNTIME = Some(runtime_state);
            let runtime_ref = RUNTIME.as_mut().expect("runtime must be initialized");

            // Precompute CCC -> value-handle lookup table once.
            // Read/write callbacks use this mapping to toggle notification state.
            let mut ccc_to_value_handle = HashMap::new();
            let mut notification_enabled = HashMap::new();
            for (&value_handle, meta) in &runtime_ref.characteristics_by_value_handle {
                if let Some(ccc_handle) = meta.ccc_handle {
                    ccc_to_value_handle.insert(ccc_handle, value_handle);
                    notification_enabled.insert(value_handle, false);
                }
            }
            runtime_ref.ccc_to_value_handle = ccc_to_value_handle;
            runtime_ref.notification_enabled = notification_enabled;

            hci_add_event_handler(std::ptr::addr_of_mut!(runtime_ref.hci_event_cb));
        }

        Ok(Self { _runtime: runtime })
    }

    /// Configure and enable BLE advertisements for the current peripheral.
    pub fn start_advertising(&self) {
        unsafe {
            if let Some(runtime) = RUNTIME.as_mut() {
                let mut null_addr = [0u8; 6];
                gap_advertisements_set_params(
                    runtime.advertising.interval_min,
                    runtime.advertising.interval_max,
                    runtime.advertising.adv_type,
                    0,
                    null_addr.as_mut_ptr(),
                    runtime.advertising.channel_map,
                    runtime.advertising.filter_policy,
                );
                gap_advertisements_set_data(
                    runtime.advertising.data.len() as u8,
                    runtime.advertising.data.as_ptr() as *mut u8,
                );
                gap_advertisements_enable(1);
            }
        }
    }

    /// Stop BLE advertisements.
    pub fn stop(&self) {
        unsafe {
            gap_advertisements_enable(0);
        }
    }
}

impl<'runtime> Drop for GattPeripheralServer<'runtime> {
    fn drop(&mut self) {
        unsafe {
            gap_advertisements_enable(0);
            RUNTIME = None;
        }
    }
}
unsafe fn resolve_characteristics(
    services: &[GattServiceSpec],
) -> Result<HashMap<u16, RuntimeCharacteristic>, GattPeripheralError> {
    let mut by_handle = HashMap::new();

    for service in services {
        let (start_handle, end_handle) = resolve_service_handle_range(service.uuid)?;

        for characteristic in &service.characteristics {
            // Resolve ATT value handle from UUID + service handle range.
            let value_handle = resolve_value_handle(start_handle, end_handle, characteristic.uuid)?;
            let ccc_handle = if characteristic
                .properties
                .contains(GattCharacteristicProperties::NOTIFY)
                || characteristic
                    .properties
                    .contains(GattCharacteristicProperties::INDICATE)
            {
                // Resolve CCC handle only for notifiable/indicatable characteristics.
                Some(resolve_ccc_handle(
                    start_handle,
                    end_handle,
                    characteristic.uuid,
                )?)
            } else {
                None
            };

            by_handle.insert(
                value_handle,
                RuntimeCharacteristic {
                    spec: characteristic.clone(),
                    value_handle,
                    ccc_handle,
                    value: characteristic.initial_value.clone(),
                },
            );
        }
    }

    Ok(by_handle)
}

unsafe fn resolve_service_handle_range(uuid: GattUuid) -> Result<(u16, u16), GattPeripheralError> {
    let mut start_handle = 0u16;
    let mut end_handle = 0u16;

    let found = match uuid {
        GattUuid::Uuid16(uuid16) => gatt_server_get_handle_range_for_service_with_uuid16(
            uuid16,
            std::ptr::addr_of_mut!(start_handle),
            std::ptr::addr_of_mut!(end_handle),
        ),
        GattUuid::Uuid128(uuid128) => gatt_server_get_handle_range_for_service_with_uuid128(
            uuid128.as_ptr(),
            std::ptr::addr_of_mut!(start_handle),
            std::ptr::addr_of_mut!(end_handle),
        ),
    };

    if !found {
        return Err(GattPeripheralError::ServiceNotFound);
    }

    Ok((start_handle, end_handle))
}

unsafe fn resolve_value_handle(
    start_handle: u16,
    end_handle: u16,
    uuid: GattUuid,
) -> Result<u16, GattPeripheralError> {
    let handle = match uuid {
        GattUuid::Uuid16(uuid16) => gatt_server_get_value_handle_for_characteristic_with_uuid16(
            start_handle,
            end_handle,
            uuid16,
        ),
        GattUuid::Uuid128(uuid128) => gatt_server_get_value_handle_for_characteristic_with_uuid128(
            start_handle,
            end_handle,
            uuid128.as_ptr(),
        ),
    };

    if handle == 0 {
        return Err(GattPeripheralError::CharacteristicNotFound);
    }

    Ok(handle)
}

unsafe fn resolve_ccc_handle(
    start_handle: u16,
    end_handle: u16,
    uuid: GattUuid,
) -> Result<u16, GattPeripheralError> {
    let handle = match uuid {
        GattUuid::Uuid16(uuid16) => {
            gatt_server_get_client_configuration_handle_for_characteristic_with_uuid16(
                start_handle,
                end_handle,
                uuid16,
            )
        }
        GattUuid::Uuid128(uuid128) => {
            gatt_server_get_client_configuration_handle_for_characteristic_with_uuid128(
                start_handle,
                end_handle,
                uuid128.as_ptr(),
            )
        }
    };

    if handle == 0 {
        return Err(GattPeripheralError::CharacteristicNotFound);
    }

    Ok(handle)
}

unsafe extern "C" fn packet_handler(packet_type: u8, _channel: u16, packet: *mut u8, _size: u16) {
    if packet_type != HCI_EVENT_PACKET as u8 || packet.is_null() {
        return;
    }

    if let Some(runtime) = RUNTIME.as_mut() {
        match *packet {
            x if x == ATT_EVENT_CONNECTED as u8 => {
                // ATT_EVENT_CONNECTED embeds the connection handle in little-endian.
                let connection_handle = u16::from_le_bytes([*packet.add(2), *packet.add(3)]);
                runtime._connected_handle = Some(connection_handle);
            }
            x if x == ATT_EVENT_DISCONNECTED as u8 => {
                // Reset all CCC states on disconnect to match BTstack sample behavior.
                runtime._connected_handle = None;
                for enabled in runtime.notification_enabled.values_mut() {
                    *enabled = false;
                }
            }
            _ => {}
        }
    }
}

unsafe extern "C" fn att_read_callback(
    _connection_handle: hci_con_handle_t,
    att_handle: u16,
    offset: u16,
    buffer: *mut u8,
    buffer_size: u16,
) -> u16 {
    // Return dynamic values for handles we resolved from the spec.
    // For unknown handles, return 0 and let ATT treat it as not handled.
    if let Some(runtime) = RUNTIME.as_ref() {
        if let Some(ch) = runtime.characteristics_by_value_handle.get(&att_handle) {
            return att_read_callback_handle_blob(
                ch.value.as_ptr(),
                ch.value.len() as u16,
                offset,
                buffer,
                buffer_size,
            );
        }
    }

    0
}

unsafe extern "C" fn att_write_callback(
    _connection_handle: hci_con_handle_t,
    att_handle: u16,
    _transaction_mode: u16,
    offset: u16,
    buffer: *mut u8,
    buffer_size: u16,
) -> c_int {
    if buffer.is_null() {
        return 0;
    }

    if let Some(runtime) = RUNTIME.as_mut() {
        if let Some(value_handle) = runtime.ccc_to_value_handle.get(&att_handle).copied() {
            // CCC writes toggle notification state for the corresponding value handle.
            if buffer_size >= 2 {
                let config = (*buffer as u16) | ((*buffer.add(1) as u16) << 8);
                runtime
                    .notification_enabled
                    .insert(value_handle, config & 0x0001 != 0);
            }
            return 0;
        }

        if let Some(ch) = runtime.characteristics_by_value_handle.get_mut(&att_handle) {
            // Ignore writes to characteristics that are not declared writable.
            if !ch
                .spec
                .properties
                .contains(GattCharacteristicProperties::WRITE)
                && !ch
                    .spec
                    .properties
                    .contains(GattCharacteristicProperties::WRITE_WITHOUT_RESPONSE)
            {
                return 0;
            }

            // Support long writes by extending the value buffer as needed.
            let offset = offset as usize;
            let size = buffer_size as usize;
            let required = offset + size;
            if ch.value.len() < required {
                ch.value.resize(required, 0);
            }
            std::ptr::copy_nonoverlapping(buffer, ch.value.as_mut_ptr().add(offset), size);
        }
    }

    0
}
