#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
CRATE_DIR="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
REPO_DIR="$(cd -- "${CRATE_DIR}/.." && pwd)"

BTSTACK_ROOT="${BTSTACK_ROOT:-${CRATE_DIR}/vendor/btstack}"
OUT_FILE="${1:-${CRATE_DIR}/src/generated/libusb_main_bindings.rs}"
WRAPPER="${CRATE_DIR}/include/wrapper_libusb_main.h"

if ! command -v bindgen >/dev/null 2>&1; then
  echo "error: bindgen executable was not found in PATH" >&2
  echo "hint: cargo install bindgen-cli --locked" >&2
  exit 1
fi

if [[ ! -d "${BTSTACK_ROOT}" ]]; then
  echo "error: BTSTACK_ROOT does not exist: ${BTSTACK_ROOT}" >&2
  echo "hint: git -C ${REPO_DIR} submodule update --init --recursive" >&2
  exit 1
fi

mkdir -p "$(dirname -- "${OUT_FILE}")"

bindgen "${WRAPPER}" \
  --output "${OUT_FILE}" \
  --use-core \
  --no-layout-tests \
  --allowlist-function '^(btstack_|hci_|l2cap_|sm_|att_|gap_|gatt_|le_device_db_|sdp_).*' \
  --allowlist-type '^(btstack_|hci_|l2cap_|att_|gap_|bd_addr_t|hci_con_handle_t).*' \
  --allowlist-var '^(HCI_|ATT_|BLUETOOTH_|USB_).*' \
  -- \
  -I"${CRATE_DIR}/cmake/btstack-core-only" \
  -I"${BTSTACK_ROOT}/src" \
  -I"${BTSTACK_ROOT}/src/ble" \
  -I"${BTSTACK_ROOT}/src/classic" \
  -I"${BTSTACK_ROOT}/platform/posix" \
  -I"${BTSTACK_ROOT}/platform/embedded" \
  -I"${BTSTACK_ROOT}/port/libusb" \
  -I"${BTSTACK_ROOT}/chipset/realtek" \
  -I"${BTSTACK_ROOT}/chipset/zephyr"

echo "Generated: ${OUT_FILE}"
