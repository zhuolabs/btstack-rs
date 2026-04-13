# btstack-rs

## Goal
This repository aims to make [BlueKitchen BTstack](https://github.com/bluekitchen/btstack) usable from Rust.

The initial target is to recreate behavior similar to BTstack USB host usage while keeping transport concerns modular.

## Repository structure (current)

```text
/
├── btstack-sys/      # raw FFI + C build integration (BTstack core-only)
└── (planned crates)
```

## Planned crate layout

```text
/
├── btstack-sys/              # raw BTstack FFI (core stack only, no USB backend)
├── btstack-transport-nusb/   # nusb-based HCI transport backend
├── btstack/                  # safe wrapper crate
└── app/                      # sample app crate(s)
```

### Responsibilities

- `btstack-sys`
  - Build vendored BTstack **core layers only**.
  - Expose low-level symbols/types.
  - Do **not** include `libusb`/`winusb` transport linkage.
- `btstack-transport-nusb`
  - Implement the HCI transport boundary using `nusb`.
  - Own USB runtime/event-loop integration details.
- `btstack`
  - Provide ergonomic safe APIs.
  - Orchestrate runtime + transport selection.

## Current status

This repository currently contains:
- a Cargo workspace
- a `btstack-sys` prototype
- submodule metadata for vendoring BTstack at `btstack-sys/vendor/btstack`
- a dedicated CMake project at `btstack-sys/cmake/btstack-core-only` used by `btstack-sys/build.rs`

## Submodule setup

Initialize the BTstack submodule:

```bash
git submodule update --init --recursive
```

## Build behavior

`btstack-sys/build.rs` does the following during `cargo build`:
1. Checks whether `btstack-sys/vendor/btstack` exists.
2. Runs CMake against `btstack-sys/cmake/btstack-core-only`.
3. Passes `BTSTACK_ROOT=<repo>/btstack-sys/vendor/btstack`.
4. Builds static `btstack` and links it into the Rust crate.

### Notes

- `btstack-sys` intentionally avoids platform USB backends (`libusb`, `winusb`).
- USB transport integration is planned as a separate crate (`btstack-transport-nusb`).

## Quick check

```bash
cargo check -p btstack-sys -vv
```

When successful, output should include:
- `cargo:rustc-cfg=btstack_vendor_build`
- `cargo:rustc-link-lib=static=btstack`

## Roadmap

### Milestone 1: `btstack-sys` core-only foundation (in progress)
- Workspace setup
- Vendor submodule integration
- Core-only BTstack static library build
- Minimal Rust FFI surface

### Milestone 2: transport backend crate (`btstack-transport-nusb`)
- Define transport boundary between `btstack-sys` and Rust backend
- Implement nusb-based packet I/O
- Integrate lifecycle and callback handling

### Milestone 3: safe wrapper crate (`btstack`)
- Hide unsafe callback plumbing
- Expose ergonomic Rust APIs for common BLE/classic tasks

### Milestone 4: sample app (`app`)
- Recreate `gatt_counter` behavior in Rust
- Document setup and run flows
