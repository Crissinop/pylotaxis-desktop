//! Blocco del portale, applicato in Rust (v0.3.0).
//!
//! Da bloccata l'app rifiuta con `LOCKED` ogni comando che non serve a sbloccarla. Il filtro
//! avvolge il gestore generato da Tauri (lib.rs), quindi vale anche per i comandi che verranno
//! aggiunti in futuro, finché qualcuno non li mette di proposito in `ALLOWED_WHILE_LOCKED`.
//! In più `AppState::with_db` rifiuta da bloccata: un comando partito un attimo prima del
//! blocco non legge il registro dopo. La schermata di blocco del frontend è solo presentazione.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::Duration;

use domain::lock::LockSettings;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime};

use crate::errors::CommandError;
use crate::platform;
use crate::state::AppState;

/// Comandi ammessi da bloccata: le informazioni sull'app e ciò che serve alla schermata di
/// blocco. `lock_status` non mostra nulla del registro: stato del blocco, metodi attivi e
/// attesa del PIN, cioè quello che la schermata stessa rende visibile. Dalla v0.5.0
/// `tray_setup` (l'app può partire bloccata, e il comando cambia solo le etichette della
/// tray) e `palette_ready` (segna che la pagina della palette ascolta; nessun dato).
pub const ALLOWED_WHILE_LOCKED: &[&str] = &[
    "app_info",
    "lock_status",
    "unlock_pin",
    "unlock_hello",
    "tray_setup",
    "palette_ready",
];

/// Evento inviato al webview a ogni cambio di stato. Porta solo `locked`: basta a nascondere
/// subito il contenuto, anche se leggere il resto dello stato fallisse.
pub const LOCK_EVENT: &str = "lock-changed";

/// Ogni quanto si controllano inattività e sessione. Un blocco di Windows più breve sfugge,
/// ma per chiuderlo servono le credenziali dell'utente.
const MONITOR_INTERVAL: Duration = Duration::from_secs(1);

const MS_PER_MINUTE: u64 = 60_000;

/// Decide se un comando può passare. Rifiuto per default: conta l'elenco dei permessi,
/// non quello dei divieti.
pub fn gate(command: &str, locked: bool) -> Result<(), CommandError> {
    if locked && !ALLOWED_WHILE_LOCKED.contains(&command) {
        Err(CommandError::LOCKED)
    } else {
        Ok(())
    }
}

/// Vero se l'inattività di sistema ha raggiunto il limite o se la sessione di Windows è
/// bloccata. Un dato che Windows non fornisce (`None`) non blocca e non sblocca nulla.
pub fn auto_lock_due(
    idle_limit_ms: Option<u64>,
    idle_ms: Option<u64>,
    session_locked: Option<bool>,
) -> bool {
    let idle_reached =
        matches!((idle_limit_ms, idle_ms), (Some(limit), Some(idle)) if idle >= limit);
    idle_reached || session_locked == Some(true)
}

/// Copia in memoria di ciò che serve al filtro e al controllo automatico: il filtro gira a
/// ogni comando e il controllo ogni secondo, e nessuno dei due deve aspettare il database.
/// La fonte resta il database: la copia si riallinea con `sync` dopo ogni modifica.
pub struct LockState {
    locked: AtomicBool,
    configured: AtomicBool,
    /// Minuti di inattività prima del blocco; 0 = mai.
    idle_minutes: AtomicU32,
}

impl LockState {
    /// Un'app con il PIN impostato parte bloccata. Senza configurazione leggibile parte
    /// sbloccata: in quel caso lo stato contiene l'errore del database, e ogni comando che
    /// tocca il registro lo restituisce (state.rs).
    pub fn starting_from(settings: Option<&LockSettings>) -> Self {
        let state = Self {
            locked: AtomicBool::new(false),
            configured: AtomicBool::new(false),
            idle_minutes: AtomicU32::new(0),
        };
        if let Some(settings) = settings {
            state.sync(settings);
            state.locked.store(settings.pin_set, Ordering::SeqCst);
        }
        state
    }

    pub fn is_locked(&self) -> bool {
        self.locked.load(Ordering::SeqCst)
    }

    pub fn is_configured(&self) -> bool {
        self.configured.load(Ordering::SeqCst)
    }

    pub fn idle_limit_ms(&self) -> Option<u64> {
        match self.idle_minutes.load(Ordering::SeqCst) {
            0 => None,
            minutes => Some(u64::from(minutes) * MS_PER_MINUTE),
        }
    }

    /// Riallinea la copia dopo una modifica. Spegnere il blocco sblocca anche l'app.
    pub fn sync(&self, settings: &LockSettings) {
        self.configured.store(settings.pin_set, Ordering::SeqCst);
        self.idle_minutes
            .store(settings.idle_minutes.unwrap_or(0), Ordering::SeqCst);
        if !settings.pin_set {
            self.locked.store(false, Ordering::SeqCst);
        }
    }

    /// Blocca, se il blocco è configurato. Vero se lo stato è cambiato adesso.
    pub fn engage(&self) -> bool {
        self.is_configured() && !self.locked.swap(true, Ordering::SeqCst)
    }

    pub fn release(&self) {
        self.locked.store(false, Ordering::SeqCst);
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LockEvent {
    pub(crate) locked: bool,
}

/// Avvisa il webview. È accessorio (A.7.5): se l'evento non parte, il filtro blocca comunque
/// e il frontend lo scopre al primo comando rifiutato.
pub fn announce<R: Runtime>(app: &AppHandle<R>, locked: bool) {
    if let Err(_error) = app.emit(LOCK_EVENT, LockEvent { locked }) {
        #[cfg(debug_assertions)]
        eprintln!("evento {LOCK_EVENT} non inviato: {_error}");
    }
}

/// Blocca l'app, chiude un'eventuale richiesta a Windows Hello e lo dice al webview.
pub fn engage<R: Runtime>(app: &AppHandle<R>, state: &AppState) -> bool {
    let changed = state.engage_lock();
    if changed {
        platform::hello_cancel_pending();
        // Un segreto copiato non resta negli appunti di un'app bloccata (v0.4.0).
        state.clipboard.clear_now();
        // La palette elenca il registro: si nasconde con il blocco (v0.5.0).
        crate::palette::hide(app);
        announce(app, true);
    }
    changed
}

/// Sblocca l'app dopo una verifica riuscita, e chiude la richiesta a Hello rimasta aperta
/// (es. sbloccata con il PIN mentre il prompt era ancora sullo schermo).
pub fn release<R: Runtime>(app: &AppHandle<R>, state: &AppState) {
    state.lock.release();
    platform::hello_cancel_pending();
    announce(app, false);
}

/// Controllo automatico in un thread proprio: inattività di sistema e blocco della sessione.
pub fn spawn_monitor<R: Runtime>(app: AppHandle<R>) {
    let started = std::thread::Builder::new()
        .name("lock-monitor".into())
        .spawn(move || {
            loop {
                std::thread::sleep(MONITOR_INTERVAL);
                let Some(state) = app.try_state::<AppState>() else {
                    continue;
                };
                // Windows si interroga solo quando serve: blocco configurato e app aperta.
                if !state.lock.is_configured() || state.lock.is_locked() {
                    continue;
                }
                if auto_lock_due(
                    state.lock.idle_limit_ms(),
                    platform::idle_ms(),
                    platform::session_locked(),
                ) {
                    engage(&app, &state);
                }
            }
        });
    // Senza il thread restano il blocco all'avvio e quello manuale; manca solo l'automatico.
    if let Err(_error) = started {
        #[cfg(debug_assertions)]
        eprintln!("controllo del blocco non avviato: {_error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod app_commands {
        include!("../app_commands.rs");
    }
    use app_commands::APP_COMMANDS;

    fn settings(pin_set: bool, idle_minutes: Option<u32>) -> LockSettings {
        LockSettings {
            pin_set,
            hello_enabled: false,
            idle_minutes,
            failed_attempts: 0,
            last_failure_ms: None,
            pin_length: None,
        }
    }

    #[test]
    fn the_gate_refuses_everything_but_unlocking_while_locked() {
        for command in APP_COMMANDS {
            assert_eq!(gate(command, false), Ok(()), "{command} da sbloccata");
            let expected = if ALLOWED_WHILE_LOCKED.contains(command) {
                Ok(())
            } else {
                Err(CommandError::LOCKED)
            };
            assert_eq!(gate(command, true), expected, "{command} da bloccata");
        }
        // Rifiuto per default: un comando che non esiste ancora nasce bloccato.
        assert_eq!(gate("comando_futuro", true), Err(CommandError::LOCKED));
    }

    #[test]
    fn every_allowed_command_exists() {
        for command in ALLOWED_WHILE_LOCKED {
            assert!(APP_COMMANDS.contains(command), "{command} non è un comando");
        }
    }

    #[test]
    fn a_configured_app_starts_locked_and_an_unconfigured_one_does_not() {
        assert!(LockState::starting_from(Some(&settings(true, Some(15)))).is_locked());
        assert!(!LockState::starting_from(Some(&settings(false, Some(15)))).is_locked());
        assert!(!LockState::starting_from(None).is_locked());
    }

    #[test]
    fn engaging_needs_a_configuration_and_reports_only_real_changes() {
        let unconfigured = LockState::starting_from(Some(&settings(false, None)));
        assert!(!unconfigured.engage());
        assert!(!unconfigured.is_locked());

        let lock = LockState::starting_from(Some(&settings(true, None)));
        lock.release();
        assert!(lock.engage());
        assert!(!lock.engage(), "già bloccata");
        // Spegnere il blocco dalle impostazioni sblocca anche l'app.
        lock.sync(&settings(false, None));
        assert!(!lock.is_locked() && !lock.is_configured());
    }

    #[test]
    fn idle_minutes_become_milliseconds_and_zero_means_never() {
        let lock = LockState::starting_from(Some(&settings(true, Some(15))));
        assert_eq!(lock.idle_limit_ms(), Some(900_000));
        lock.sync(&settings(true, None));
        assert_eq!(lock.idle_limit_ms(), None);
    }

    #[test]
    fn auto_lock_follows_idle_time_and_the_windows_session() {
        let limit = Some(900_000);
        assert!(!auto_lock_due(limit, Some(899_999), Some(false)));
        assert!(auto_lock_due(limit, Some(900_000), Some(false)));
        assert!(auto_lock_due(limit, Some(0), Some(true)));
        assert!(auto_lock_due(None, Some(u64::MAX), Some(true)));
        assert!(!auto_lock_due(None, Some(u64::MAX), Some(false)));
        // Dati mancanti non bloccano: fuori da Windows il controllo tace.
        assert!(!auto_lock_due(limit, None, None));
    }
}
