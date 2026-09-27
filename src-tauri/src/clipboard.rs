//! Appunti per i segreti: copia privata e svuotamento automatico (v0.4.0).
//!
//! Il valore entra negli appunti con i formati che lo tengono fuori dalla cronologia di
//! Windows e dalla sincronizzazione tra dispositivi (platform). Dopo `CLIPBOARD_CLEAR_MS`, al
//! blocco e alla chiusura dell'app gli appunti si svuotano, ma solo se contengono ancora la
//! nostra copia: il numero di sequenza di Windows dice se nel frattempo qualcuno ha copiato
//! altro, e in quel caso non si tocca nulla.

use std::sync::Mutex;
use std::time::Duration;

use tauri::{AppHandle, Manager, Runtime};

use crate::platform;
use crate::state::AppState;

/// Quanto resta negli appunti un segreto copiato.
pub const CLIPBOARD_CLEAR_MS: u64 = 30_000;

#[derive(Default)]
pub struct ClipboardState {
    /// Numero di sequenza degli appunti subito dopo l'ultima copia di un segreto.
    pending: Mutex<Option<u32>>,
}

impl ClipboardState {
    pub fn remember(&self, sequence: u32) {
        if let Ok(mut pending) = self.pending.lock() {
            *pending = Some(sequence);
        }
    }

    /// Svuota subito gli appunti se contengono ancora l'ultima copia (blocco, chiusura).
    pub fn clear_now(&self) {
        let sequence = self
            .pending
            .lock()
            .ok()
            .and_then(|mut pending| pending.take());
        if let Some(sequence) = sequence {
            platform::clipboard_clear_if(sequence);
        }
    }

    /// Svuota se l'ultima copia è ancora `sequence`: una copia più recente ha il suo timer.
    fn clear_if_last(&self, sequence: u32) {
        let due = self
            .pending
            .lock()
            .ok()
            .and_then(|mut pending| (*pending == Some(sequence)).then(|| pending.take()))
            .is_some();
        if due {
            platform::clipboard_clear_if(sequence);
        }
    }
}

/// Programma lo svuotamento. È accessorio (A.7.5): se il thread non parte, restano lo
/// svuotamento al blocco e alla chiusura.
pub fn schedule_clear<R: Runtime>(app: AppHandle<R>, sequence: u32) {
    let _ = std::thread::Builder::new()
        .name("clipboard-clear".into())
        .spawn(move || {
            std::thread::sleep(Duration::from_millis(CLIPBOARD_CLEAR_MS));
            if let Some(state) = app.try_state::<AppState>() {
                state.clipboard.clear_if_last(sequence);
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_last_copy_is_cleared_and_only_once() {
        let clipboard = ClipboardState::default();
        clipboard.remember(7);
        clipboard.remember(9);
        // Il timer della copia 7 non tocca la copia 9.
        clipboard.clear_if_last(7);
        assert_eq!(*clipboard.pending.lock().unwrap(), Some(9));
        clipboard.clear_if_last(9);
        assert_eq!(*clipboard.pending.lock().unwrap(), None);
        clipboard.clear_now();
        assert_eq!(*clipboard.pending.lock().unwrap(), None);
    }
}
