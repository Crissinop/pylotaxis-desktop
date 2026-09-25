//! Chiave del database nel Credential Manager di Windows (A.2, A.7.4). (v0.2.0)
//!
//! La chiave non lascia mai il processo Rust: si legge qui, si passa a `Database::open`
//! e non esiste alcun comando che la restituisca al webview (A.7.10).

use domain::DatabaseKey;
use keyring_core::{Entry, Error as KeyringError};

use crate::errors::CommandError;

/// Utente della voce nel Credential Manager; il servizio è l'identificatore dell'app.
const KEY_ENTRY_USER: &str = "database-key";

/// Sceglie l'archivio delle credenziali. Va chiamata una volta, all'avvio.
pub fn init_store() -> Result<(), CommandError> {
    #[cfg(windows)]
    let store = windows_native_keyring_store::Store::new();
    // Fuori da Windows (sviluppo e CI su Linux) un archivio in memoria: la chiave non
    // sopravvive al riavvio, quindi un database esistente verrebbe rifiutato. Windows è
    // l'unica piattaforma supportata (A.2).
    #[cfg(not(windows))]
    let store = keyring_core::mock::Store::new();

    keyring_core::set_default_store(store.map_err(|_| CommandError::KEYSTORE_UNAVAILABLE)?);
    Ok(())
}

pub struct KeyStore {
    entry: Entry,
}

impl KeyStore {
    /// Su Windows la voce usa la persistenza "local": la chiave resta su questa macchina,
    /// come il database in %LOCALAPPDATA%. Il predefinito dello store ("Enterprise") la farebbe
    /// viaggiare con i profili roaming, separata da un registro che contiene percorsi locali.
    /// L'archivio in memoria usato fuori da Windows non accetta modificatori. (v0.2.0)
    pub fn open(service: &str) -> Result<Self, CommandError> {
        #[cfg(windows)]
        let entry = Entry::new_with_modifiers(
            service,
            KEY_ENTRY_USER,
            &std::collections::HashMap::from([("persistence", "local")]),
        );
        #[cfg(not(windows))]
        let entry = Entry::new(service, KEY_ENTRY_USER);

        entry
            .map(|entry| Self { entry })
            .map_err(|_| CommandError::KEYSTORE_UNAVAILABLE)
    }

    /// `Ok(None)` se la chiave non è mai stata salvata.
    pub fn load(&self) -> Result<Option<DatabaseKey>, CommandError> {
        match self.entry.get_secret() {
            Ok(bytes) => Ok(Some(DatabaseKey::from_slice(&bytes)?)),
            Err(KeyringError::NoEntry) => Ok(None),
            Err(_) => Err(CommandError::KEYSTORE_UNAVAILABLE),
        }
    }

    pub fn save(&self, key: &DatabaseKey) -> Result<(), CommandError> {
        self.entry
            .set_secret(key.expose_secret())
            .map_err(|_| CommandError::KEYSTORE_UNAVAILABLE)
    }
}
