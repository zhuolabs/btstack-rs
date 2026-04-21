//! BTstack runtime owner that controls process-wide run loop lifecycle.
//!
//! BTstack uses global singletons for memory pools, HCI, and run loop state.
//! [`BtstackRuntime`] centralizes that lifecycle so peripheral servers and samples
//! can only start after the runtime has been initialized.

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};

use btstack_hci_transport_nusb::hci_transport_nusb_instance;
use btstack_sys::{
    btstack_memory_deinit, btstack_memory_init, btstack_run_loop_deinit, btstack_run_loop_execute,
    btstack_run_loop_init, btstack_run_loop_posix_get_instance, btstack_run_loop_trigger_exit,
    hci_close, hci_deinit, hci_init,
};

#[cfg(target_os = "windows")]
unsafe extern "C" {
    fn btstack_run_loop_windows_get_instance() -> *const btstack_sys::btstack_run_loop_t;
}

static RUNTIME_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Errors surfaced by [`BtstackRuntime`] lifecycle operations.
#[derive(Debug)]
pub enum BtstackRuntimeError {
    AlreadyStarted,
    LoopThreadPanicked,
}

#[cfg(target_os = "windows")]
unsafe fn run_loop_instance() -> *const btstack_sys::btstack_run_loop_t {
    btstack_run_loop_windows_get_instance()
}

#[cfg(target_os = "linux")]
unsafe fn run_loop_instance() -> *const btstack_sys::btstack_run_loop_t {
    btstack_run_loop_posix_get_instance()
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
unsafe fn run_loop_instance() -> *const btstack_sys::btstack_run_loop_t {
    btstack_run_loop_posix_get_instance()
}

/// Process-wide owner for BTstack memory, HCI stack, and run loop thread.
///
/// Start this type before constructing any [`crate::peripheral::GattPeripheralServer`].
/// The runtime spawns a dedicated thread that runs `btstack_run_loop_execute()`.
///
/// Shutdown behavior:
/// - [`shutdown`](Self::shutdown) triggers `btstack_run_loop_trigger_exit()`, joins the run-loop
///   thread, then deinitializes BTstack globals.
/// - [`Drop`] performs the same sequence as a best effort cleanup.
pub struct BtstackRuntime {
    loop_thread: Option<JoinHandle<()>>,
}

impl BtstackRuntime {
    /// Initializes BTstack globals and starts the dedicated run-loop thread.
    pub fn start() -> Result<Self, BtstackRuntimeError> {
        if RUNTIME_ACTIVE.swap(true, Ordering::SeqCst) {
            return Err(BtstackRuntimeError::AlreadyStarted);
        }

        unsafe {
            btstack_memory_init();
            btstack_run_loop_init(run_loop_instance());
            hci_init(hci_transport_nusb_instance(), std::ptr::null::<c_void>());
        }

        let loop_thread = thread::spawn(|| unsafe {
            btstack_run_loop_execute();
        });

        Ok(Self {
            loop_thread: Some(loop_thread),
        })
    }

    /// Blocks until the run-loop thread exits.
    pub fn join(&mut self) -> Result<(), BtstackRuntimeError> {
        if let Some(handle) = self.loop_thread.take() {
            if handle.join().is_err() {
                return Err(BtstackRuntimeError::LoopThreadPanicked);
            }
        }
        Ok(())
    }

    /// Requests run-loop exit, waits for the thread, and deinitializes BTstack.
    pub fn shutdown(&mut self) -> Result<(), BtstackRuntimeError> {
        unsafe {
            btstack_run_loop_trigger_exit();
        }

        self.join()?;

        unsafe {
            hci_close();
            hci_deinit();
            btstack_run_loop_deinit();
            btstack_memory_deinit();
        }

        RUNTIME_ACTIVE.store(false, Ordering::SeqCst);
        Ok(())
    }
}

impl Drop for BtstackRuntime {
    fn drop(&mut self) {
        if self.loop_thread.is_some() {
            let _ = self.shutdown();
        }
    }
}
