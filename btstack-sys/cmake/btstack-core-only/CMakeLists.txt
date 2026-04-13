cmake_minimum_required(VERSION 3.12)

project(btstack-core-only C)

if(NOT DEFINED BTSTACK_ROOT)
    message(FATAL_ERROR "BTSTACK_ROOT is required")
endif()

set(CMAKE_EXPORT_COMPILE_COMMANDS ON)

include_directories(.)
include_directories(${BTSTACK_ROOT}/src)
include_directories(${BTSTACK_ROOT}/3rd-party/micro-ecc)
include_directories(${BTSTACK_ROOT}/3rd-party/bluedroid/decoder/include)
include_directories(${BTSTACK_ROOT}/3rd-party/bluedroid/encoder/include)
include_directories(${BTSTACK_ROOT}/3rd-party/lc3-google/include)
include_directories(${BTSTACK_ROOT}/3rd-party/md5)
include_directories(${BTSTACK_ROOT}/3rd-party/hxcmod-player)
include_directories(${BTSTACK_ROOT}/3rd-party/hxcmod-player/mod)
include_directories(${BTSTACK_ROOT}/3rd-party/qr-code-generator)
include_directories(${BTSTACK_ROOT}/3rd-party/rijndael)
include_directories(${BTSTACK_ROOT}/3rd-party/yxml)
include_directories(${BTSTACK_ROOT}/chipset/realtek)
include_directories(${BTSTACK_ROOT}/chipset/zephyr)

file(GLOB SOURCES_SRC "${BTSTACK_ROOT}/src/*.c")
file(GLOB SOURCES_BLE "${BTSTACK_ROOT}/src/ble/*.c")
file(GLOB SOURCES_GATT "${BTSTACK_ROOT}/src/ble/gatt-service/*.c")
file(GLOB SOURCES_CLASSIC "${BTSTACK_ROOT}/src/classic/*.c")
file(GLOB SOURCES_MESH "${BTSTACK_ROOT}/src/mesh/*.c" "${BTSTACK_ROOT}/src/mesh/gatt-service/*.c")
file(GLOB SOURCES_BLUEDROID "${BTSTACK_ROOT}/3rd-party/bluedroid/encoder/srce/*.c" "${BTSTACK_ROOT}/3rd-party/bluedroid/decoder/srce/*.c")
file(GLOB SOURCES_UECC "${BTSTACK_ROOT}/3rd-party/micro-ecc/uECC.c")
file(GLOB SOURCES_MD5 "${BTSTACK_ROOT}/3rd-party/md5/md5.c")
file(GLOB SOURCES_HXCMOD "${BTSTACK_ROOT}/3rd-party/hxcmod-player/*.c" "${BTSTACK_ROOT}/3rd-party/hxcmod-player/mods/*.c")
file(GLOB SOURCES_QRCODEGEN "${BTSTACK_ROOT}/3rd-party/qr-code-generator/qrcodegen.c")
file(GLOB SOURCES_RIJNDAEL "${BTSTACK_ROOT}/3rd-party/rijndael/rijndael.c")
file(GLOB SOURCES_YXML "${BTSTACK_ROOT}/3rd-party/yxml/yxml.c")
file(GLOB SOURCES_SRC_OFF "${BTSTACK_ROOT}/src/hci_transport_*.c")
list(REMOVE_ITEM SOURCES_SRC ${SOURCES_SRC_OFF})

set(SOURCES
    ${SOURCES_SRC}
    ${SOURCES_BLE}
    ${SOURCES_GATT}
    ${SOURCES_CLASSIC}
    ${SOURCES_MESH}
    ${SOURCES_BLUEDROID}
    ${SOURCES_UECC}
    ${SOURCES_MD5}
    ${SOURCES_HXCMOD}
    ${SOURCES_QRCODEGEN}
    ${SOURCES_RIJNDAEL}
    ${SOURCES_YXML}
)

list(SORT SOURCES)
add_library(btstack STATIC ${SOURCES})
