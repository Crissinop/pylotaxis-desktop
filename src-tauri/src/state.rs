//! Stato condiviso tra i comandi e risoluzione sicura dei bersagli (v0.2.0).

use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use domain::registry::{App, ExecutableInfo, Target};
use domain::{Database, DatabaseKey, KeyPlan};
use serde::Deserialize;
use uuid::Uuid;

use crate::errors::CommandError;
use crate::keystore::KeyStore;

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
}

impl AppState {
    pub fn new(db: Result<Database, CommandError>) -> Self {
        Self {
            db: Mutex::new(db),
            pick: Mutex::new(None),
        }
    }

    pub fn with_db<T>(
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
        };
        let keep = || TargetInput::Executable { pick_token: None };
        assert!(resolve_target(keep(), &mut None, None).is_err());
        assert!(resolve_target(keep(), &mut None, Some(&web_app)).is_err());
    }

    #[test]
    fn unknown_ids_read_as_not_found() {
        assert_eq!(parse_id("non-un-uuid").unwrap_err().code, "NOT_FOUND");
    }
}
