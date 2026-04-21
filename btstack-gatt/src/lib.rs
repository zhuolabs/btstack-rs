//! High-level BLE GATT examples built on top of `btstack-sys`.
//!
//! This crate currently provides a Rust port of BTstack's `gatt_counter.c`
//! example as a reusable module.

pub mod gatt_counter;
pub mod peripheral;
pub mod runtime;

pub mod gatt_counter_main;
