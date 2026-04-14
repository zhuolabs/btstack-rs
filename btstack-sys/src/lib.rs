#![allow(non_camel_case_types)]

//! Minimal low-level FFI prototype for BTstack integration.

mod ffi;

pub mod att_db;
pub mod att_server;
pub mod bluetooth;
pub mod btstack_defines;
pub mod btstack_memory;
pub mod btstack_run_loop;
pub mod gap;
pub mod hci;
pub mod hci_cmd;
pub mod hci_transport;
pub mod l2cap;
pub mod sm;

pub use btstack_run_loop::BtstackRunLoop;
