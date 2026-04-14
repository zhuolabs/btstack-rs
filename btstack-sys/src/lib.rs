//! Minimal low-level FFI prototype for BTstack integration.

mod ffi;
pub mod btstack_memory;
pub mod btstack_run_loop;

pub use btstack_run_loop::BtstackRunLoop;
