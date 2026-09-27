//! Parti specifiche di Windows, dietro un'interfaccia che compila ovunque (v0.3.0).
//!
//! Su Windows: Windows Hello legato alla finestra, inattività di sistema, stato della
//! sessione. Altrove (sviluppo e CI su Linux) Hello non è disponibile e i controlli
//! automatici tacciono; il blocco all'avvio, quello manuale e il PIN funzionano uguale.

#[cfg(not(windows))]
mod other;
#[cfg(windows)]
mod win;

#[cfg(not(windows))]
pub use other::{
    clipboard_clear_if, clipboard_copy_private, hello_availability, hello_cancel_pending,
    hello_verify, idle_ms, session_locked, window_owner,
};
#[cfg(windows)]
pub use win::{
    clipboard_clear_if, clipboard_copy_private, hello_availability, hello_cancel_pending,
    hello_verify, idle_ms, session_locked, window_owner,
};

use crate::errors::CommandError;

/// `UserConsentVerifierAvailability`: 0 disponibile, poi i motivi per cui non lo è.
/// I numeri sono quelli dell'API WinRT; un test su Windows li confronta con le costanti.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn availability_result(value: i32) -> Result<(), CommandError> {
    match value {
        0 => Ok(()),
        1 => Err(CommandError::HELLO_NO_DEVICE),
        2 => Err(CommandError::HELLO_NOT_CONFIGURED),
        3 => Err(CommandError::HELLO_DISABLED_BY_POLICY),
        4 => Err(CommandError::HELLO_BUSY),
        _ => Err(CommandError::HELLO_FAILED),
    }
}

/// `UserConsentVerificationResult`: 0 verificato; da 1 a 4 come la disponibilità; 5 troppi
/// tentativi; 6 annullato.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn verification_result(value: i32) -> Result<(), CommandError> {
    match value {
        5 => Err(CommandError::HELLO_RETRIES_EXHAUSTED),
        6 => Err(CommandError::HELLO_CANCELED),
        other => availability_result(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_verified_counts_as_verified() {
        assert_eq!(verification_result(0), Ok(()));
        for (value, code) in [
            (1, "HELLO_NO_DEVICE"),
            (2, "HELLO_NOT_CONFIGURED"),
            (3, "HELLO_DISABLED_BY_POLICY"),
            (4, "HELLO_BUSY"),
            (5, "HELLO_RETRIES_EXHAUSTED"),
            (6, "HELLO_CANCELED"),
            (7, "HELLO_FAILED"),
            (-1, "HELLO_FAILED"),
        ] {
            assert_eq!(
                verification_result(value).unwrap_err().code,
                code,
                "{value}"
            );
        }
        assert_eq!(availability_result(0), Ok(()));
        assert_eq!(availability_result(5).unwrap_err().code, "HELLO_FAILED");
    }
}
