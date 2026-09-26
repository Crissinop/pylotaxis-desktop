//! Fuori da Windows (sviluppo e CI su Linux): Hello non esiste e i controlli automatici
//! tacciono. Windows è l'unica piattaforma supportata (A.2). (v0.3.0)

use tauri::{Runtime, WebviewWindow};

use crate::errors::CommandError;

pub fn window_owner<R: Runtime>(_window: &WebviewWindow<R>) -> Result<usize, CommandError> {
    Ok(0)
}

pub fn hello_availability() -> Result<(), CommandError> {
    Err(CommandError::HELLO_NO_DEVICE)
}

pub fn hello_verify(_owner: usize, _message: &str) -> Result<(), CommandError> {
    Err(CommandError::HELLO_NO_DEVICE)
}

pub fn hello_cancel_pending() {}

pub fn idle_ms() -> Option<u64> {
    None
}

pub fn session_locked() -> Option<bool> {
    None
}
