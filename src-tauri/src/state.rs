//! Stato condiviso tra i comandi e risoluzione sicura dei bersagli (v0.2.0).

use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use domain::registry::{App, ExecutableInfo, Target};
use domain::{Database, DatabaseKey, KeyPlan};
use serde::Deserialize;
use uuid::Uuid;

use crate::clipboard::ClipboardState;
use crate::errors::CommandError;
use crate::health::HealthState;
use crate::icons::IconState;
use crate::keystore::KeyStore;
use crate::lock::LockState;
use crate::palette::PaletteState;
use crate::shortcut::ShortcutState;
use crate::vault::Vault;

/// Nome del file del database nella cartella dati dell'app.
pub const DATABASE_FILE: &str = "registry.db";

/// Eseguibile scelto con la finestra di sistema e non ancora salvato.
#[derive(Debug, Clone)]
pub struct PendingPick {
    pub token: String,
    pub info: ExecutableInfo,
}

pub struct AppState {
    /// L'errore d'avvio si conserva: ogni comando lo restituisce, così l'interfaccia
    /// può spiegarlo invece di fallire in modi diversi a ogni azione.
    db: Mutex<Result<Database, CommandError>>,
    /// Una sola scelta in sospeso: il modulo di inserimento ne gestisce una alla volta.
    pick: Mutex<Option<PendingPick>>,
    pub lock: LockState,
    /// Valori dei segreti, nel Credential Manager (v0.4.0).
    pub vault: Vault,
    pub clipboard: ClipboardState,
    /// Scorciatoia della palette e aperture in attesa della sua pagina (v0.5.0).
    pub shortcut: ShortcutState,
    pub palette: PaletteState,
    /// Ultimo stato delle app web con il controllo acceso, solo in memoria (v0.6.0).
    pub health: HealthState,
    /// Eseguibili di cui si è già tentata l'icona in questa sessione (v0.7.0).
    pub icons: IconState,
}

impl AppState {
    /// La configurazione del blocco si legge subito: un'app con il PIN parte bloccata. Se la
    /// lettura fallisce, l'errore prende il posto del database, così nessun comando parte da
    /// uno stato del blocco sconosciuto (v0.3.0).
    pub fn new(db: Result<Database, CommandError>, vault: Vault) -> Self {
        let opened = db.and_then(|db| {
            let settings = db.lock_settings()?;
            Ok((db, settings))
        });
        let (db, lock) = match opened {
            Ok((db, settings)) => (Ok(db), LockState::starting_from(Some(&settings))),
            Err(error) => (Err(error), LockState::starting_from(None)),
        };
        Self {
            db: Mutex::new(db),
            pick: Mutex::new(None),
            lock,
            vault,
            clipboard: ClipboardState::default(),
            shortcut: ShortcutState::default(),
            palette: PaletteState::default(),
            health: HealthState::default(),
            icons: IconState::default(),
        }
    }

    /// Accesso al registro. Da bloccata rifiuta, anche se il comando ha superato il filtro un
    /// attimo prima del blocco (v0.3.0).
    pub fn with_db<T>(
        &self,
        f: impl FnOnce(&Database) -> Result<T, CommandError>,
    ) -> Result<T, CommandError> {
        if self.lock.is_locked() {
            return Err(CommandError::LOCKED);
        }
        self.with_db_even_if_locked(f)
    }

    /// Accesso anche da bloccata: solo per lo stato del blocco e lo sblocco (security.rs).
    /// Il nome rende visibile nel codice ogni punto in cui si fa un'eccezione. (v0.3.0)
    pub fn with_db_even_if_locked<T>(
        &self,
        f: impl FnOnce(&Database) -> Result<T, CommandError>,
    ) -> Result<T, CommandError> {
        let guard = self
            .db
            .lock()
            .map_err(|_| CommandError::STATE_UNAVAILABLE)?;
        match &*guard {
            Ok(db) => f(db),
            Err(error) => Err(error.clone()),
        }
    }

    pub fn pick(&self) -> Result<MutexGuard<'_, Option<PendingPick>>, CommandError> {
        self.pick
            .lock()
            .map_err(|_| CommandError::STATE_UNAVAILABLE)
    }

    /// Blocca e scarta la scelta di un eseguibile in sospeso: un gettone non sopravvive al
    /// blocco. Vero se l'app era sbloccata. (v0.3.0)
    pub fn engage_lock(&self) -> bool {
        let changed = self.lock.engage();
        if changed && let Ok(mut pick) = self.pick.lock() {
            *pick = None;
        }
        changed
    }
}

/// Apre (o crea al primo avvio) il database cifrato in `dir`, con la chiave custodita in
/// `keys`. L'ordine conta: al primo avvio la chiave si salva prima di creare il file.
pub fn open_database(dir: &Path, keys: &KeyStore) -> Result<Database, CommandError> {
    std::fs::create_dir_all(dir).map_err(|_| CommandError::APP_DATA_UNAVAILABLE)?;
    let path = dir.join(DATABASE_FILE);
    let stored = keys.load()?;
    let key = match (domain::plan_key(path.exists(), stored.is_some()), stored) {
        (KeyPlan::UseStored, Some(key)) => key,
        (KeyPlan::GenerateAndStore, _) => {
            let key = DatabaseKey::generate()?;
            keys.save(&key)?;
            key
        }
        _ => return Err(domain::Error::KeyMissing.into()),
    };
    Ok(Database::open(&path, &key)?)
}

/// Bersaglio come lo invia il frontend. Per un eseguibile arriva solo il gettone della
/// scelta fatta con la finestra di sistema, mai un percorso: `deny_unknown_fields` fa
/// fallire la richiesta se qualcuno prova ad aggiungerne uno (A.7.10). (v0.2.0)
#[derive(Debug, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "lowercase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum TargetInput {
    Executable { pick_token: Option<String> },
    Web { url: String },
    Protocol { uri: String },
}

/// Traduce il bersaglio del frontend in un `Target` del dominio.
///
/// - Eseguibile con gettone: vale solo se coincide con la scelta in sospeso, che si consuma.
/// - Eseguibile senza gettone: solo in modifica, e solo se l'app era già un eseguibile.
/// - Indirizzi e URI: passano al dominio, che li valida prima di scriverli.
pub fn resolve_target(
    input: TargetInput,
    pending: &mut Option<PendingPick>,
    existing: Option<&App>,
) -> Result<Target, CommandError> {
    match input {
        TargetInput::Executable {
            pick_token: Some(token),
        } => match pending.take() {
            Some(pick) if pick.token == token => Ok(Target::Executable {
                path: pick.info.path,
                sha256: pick.info.sha256,
            }),
            other => {
                // Un gettone sbagliato non consuma la scelta valida ancora in sospeso.
                *pending = other;
                Err(CommandError::PICK_EXPIRED)
            }
        },
        TargetInput::Executable { pick_token: None } => match existing.map(|app| &app.target) {
            Some(target @ Target::Executable { .. }) => Ok(target.clone()),
            _ => Err(domain::Error::ExecutableInvalid.into()),
        },
        TargetInput::Web { url } => Ok(Target::Web { url }),
        TargetInput::Protocol { uri } => Ok(Target::Protocol { uri }),
    }
}

/// Gli id arrivano come testo dal webview: uno non valido equivale a un elemento assente.
pub fn parse_id(raw: &str) -> Result<Uuid, CommandError> {
    Uuid::parse_str(raw).map_err(|_| domain::Error::NotFound.into())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn pick(token: &str) -> PendingPick {
        PendingPick {
            token: token.into(),
            info: ExecutableInfo {
                path: PathBuf::from("C:/Strumenti/tool.exe"),
                sha256: "a".repeat(64),
                suggested_name: "tool".into(),
            },
        }
    }

    /// La sonda della roadmap v0.2.0: un percorso inviato dal frontend viene rifiutato
    /// già alla lettura della richiesta, prima di arrivare a qualunque comando.
    #[test]
    fn a_path_sent_by_the_frontend_is_rejected() {
        let forged = r#"{"kind":"executable","path":"C:\\Windows\\System32\\cmd.exe"}"#;
        assert!(serde_json::from_str::<TargetInput>(forged).is_err());
        let smuggled = r#"{"kind":"executable","pickToken":null,"path":"C:\\x.exe"}"#;
        assert!(serde_json::from_str::<TargetInput>(smuggled).is_err());
        let fine = r#"{"kind":"executable","pickToken":"t"}"#;
        assert!(serde_json::from_str::<TargetInput>(fine).is_ok());
    }

    #[test]
    fn a_pick_token_is_valid_once_and_only_if_it_matches() {
        let mut pending = Some(pick("giusto"));
        let wrong = resolve_target(
            TargetInput::Executable {
                pick_token: Some("sbagliato".into()),
            },
            &mut pending,
            None,
        );
        assert_eq!(wrong, Err(CommandError::PICK_EXPIRED));
        assert!(
            pending.is_some(),
            "un gettone sbagliato non consuma la scelta"
        );

        let right = resolve_target(
            TargetInput::Executable {
                pick_token: Some("giusto".into()),
            },
            &mut pending,
            None,
        );
        assert!(matches!(right, Ok(Target::Executable { .. })));
        let again = resolve_target(
            TargetInput::Executable {
                pick_token: Some("giusto".into()),
            },
            &mut pending,
            None,
        );
        assert_eq!(again, Err(CommandError::PICK_EXPIRED), "gettone monouso");
    }

    #[test]
    fn keeping_an_executable_requires_an_existing_executable() {
        let web_app = App {
            id: Uuid::now_v7(),
            name: "Sito".into(),
            target: Target::Web {
                url: "https://example.com/".into(),
            },
            category_id: None,
            tags: Vec::new(),
            environment: None,
            health_check: false,
            icon_rev: None,
        };
        let keep = || TargetInput::Executable { pick_token: None };
        assert!(resolve_target(keep(), &mut None, None).is_err());
        assert!(resolve_target(keep(), &mut None, Some(&web_app)).is_err());
    }

    fn memory_vault() -> Vault {
        Vault::new(Box::new(crate::vault::MemoryBackend::default()))
    }

    fn locked_state() -> (tempfile::TempDir, AppState) {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::open(
            &dir.path().join("r.db"),
            &DatabaseKey::from_bytes([5; domain::db::KEY_LEN]),
        )
        .unwrap();
        db.set_pin(None, "482915", 0).unwrap();
        (dir, AppState::new(Ok(db), memory_vault()))
    }

    /// Secondo controllo, dietro il filtro: da bloccata il registro non si legge nemmeno
    /// da un comando che il filtro avesse lasciato passare un attimo prima (v0.3.0).
    #[test]
    fn a_locked_state_refuses_the_registry_but_not_the_lock_screen() {
        let (_dir, state) = locked_state();
        assert!(state.lock.is_locked(), "configurata: parte bloccata");
        assert_eq!(
            state.with_db(|db| Ok(db.registry()?)).unwrap_err(),
            CommandError::LOCKED
        );
        assert!(
            state
                .with_db_even_if_locked(|db| Ok(db.lock_settings()?))
                .is_ok()
        );
        state.lock.release();
        assert!(state.with_db(|db| Ok(db.registry()?)).is_ok());
    }

    #[test]
    fn locking_discards_a_pending_pick() {
        let (_dir, state) = locked_state();
        state.lock.release();
        *state.pick().unwrap() = Some(pick("in-sospeso"));
        assert!(state.engage_lock());
        assert!(state.pick().unwrap().is_none());
    }

    #[test]
    fn unknown_ids_read_as_not_found() {
        assert_eq!(parse_id("non-un-uuid").unwrap_err().code, "NOT_FOUND");
    }
}
