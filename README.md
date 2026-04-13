# btstack-rs

## Goal
This repository aims to make [BlueKitchen BTstack](https://github.com/bluekitchen/btstack) usable from Rust.

The initial target is to recreate the behavior of:
- Windows port: `port/windows-winusb`
- Example app: `example/gatt_counter.c`

## Repository structure

```text
/
├── btstack-sys/      # FFI + C build integration
├── btstack/          # safe wrapper (next step)
└── app/              # sample app (next step)
```

## Current status

This repository now contains:
- a Cargo workspace
- a `btstack-sys` crate prototype
- submodule metadata for vendoring BTstack at `btstack-sys/vendor/btstack`
- `build.rs` logic that tries to build vendored BTstack via CMake and falls back to a local shim when unavailable

## Submodule setup

Initialize the BTstack submodule:

```bash
git submodule update --init --recursive
```

If your environment blocks GitHub access, submodule initialization will fail. In that case, `cargo build` falls back to the local shim so bootstrap work can continue.

## Build behavior

`btstack-sys/build.rs` does the following during `cargo build`:
1. Checks whether `btstack-sys/vendor/btstack` exists and contains `CMakeLists.txt`.
2. If present, runs:
   - `cmake -S btstack-sys/vendor/btstack -B <OUT_DIR>/btstack-cmake-build`
   - `cmake --build <OUT_DIR>/btstack-cmake-build`
3. Regardless of vendor-build success, compiles a local C shim to keep the current bootstrap Rust API stable.

This is an incremental step toward full raw BTstack FFI exposure.

## Roadmap

### Milestone 1: `btstack-sys` foundation (in progress)
- Workspace setup
- Build script and C compilation path
- Minimal FFI API shape
- Submodule metadata and vendor build hook

### Milestone 2: full BTstack raw FFI
- Initialize submodule in CI and development environments
- Bind selected BTstack headers
- Replace shim API with direct BTstack symbols

### Milestone 3: safe wrapper crate (`btstack`)
- Introduce runtime abstraction
- Hide unsafe callback plumbing
- Expose ergonomic Rust API for GATT peripheral setup

### Milestone 4: sample app (`app`)
- Recreate `gatt_counter` behavior in Rust
- Document Windows setup and run instructions
