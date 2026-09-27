//! Valori dei segreti, nel Credential Manager di Windows (v0.4.0).
//!
//! È l'unico modulo che scrive o legge il valore di un segreto, e nessuna sua funzione lo
//! restituisce: `with_value` lo presta a una closure (la copia negli appunti) e restituisce
//! solo il risultato di quella. Il webview non lo vede mai (A.7.10).
//!
//! Registro e Credential Manager non condividono una transazione. L'ordine delle scritture
//! tiene il registro come fonte di verità: la credenziale nasce prima della riga e sparisce
//! dopo di lei; quelle rimaste orfane per un errore a metà si eliminano all'avvio (`sweep`).

use std::collections::HashSet;

use keyring_core::{Entry, Error as KeyringError};
use uuid::Uuid;

use crate::errors::CommandError;

/// Utente della credenziale di un segreto. Il Credential Manager compone il nome come
/// `secret:<id>.<identificativo>`, accanto alla chiave del database (`database-key.<…>`).
const SECRET_USER_PREFIX: &str = "secret:";

/// Dove stanno i valori: il Credential Manager in produzione, una mappa in memoria nei test.
pub trait SecretBackend: Send + Sync {
    fn put(&self, id: Uuid, value: &[u8]) -> Result<(), CommandError>;
    /// `Ok(None)` se la credenziale non esiste.
    fn get(&self, id: Uuid) -> Result<Option<Vec<u8>>, CommandError>;
    /// Eliminare una credenziale che non c'è non è un errore.
    fn remove(&self, id: Uuid) -> Result<(), CommandError>;
    /// Id dei segreti presenti nello store, per trovare le credenziali orfane.
    fn list(&self) -> Result<Vec<Uuid>, CommandError>;
}

pub struct Vault {
    backend: Box<dyn SecretBackend>,
}

impl Vault {
    pub fn new(backend: Box<dyn SecretBackend>) -> Self {
        Self { backend }
    }

    pub fn store(&self, id: Uuid, value: &str) -> Result<(), CommandError> {
        self.backend.put(id, value.as_bytes())
    }

    /// Presta il valore a `use_value` senza restituirlo: chi chiama riceve solo il risultato
    /// della closure. Una riga senza credenziale risponde `SECRET_MISSING`.
    pub fn with_value<T>(
        &self,
        id: Uuid,
        use_value: impl FnOnce(&str) -> Result<T, CommandError>,
    ) -> Result<T, CommandError> {
        let bytes = self.backend.get(id)?.ok_or(CommandError::SECRET_MISSING)?;
        let value = String::from_utf8(bytes).map_err(|_| CommandError::SECRET_MISSING)?;
        use_value(&value)
    }

    pub fn remove(&self, id: Uuid) -> Result<(), CommandError> {
        self.backend.remove(id)
    }

    /// Elimina le credenziali dei segreti che il registro non conosce, e dice quante. È
    /// accessoria (A.7.5): se non riesce, ci riprova il prossimo avvio.
    pub fn sweep(&self, known: &HashSet<Uuid>) -> usize {
        let Ok(found) = self.backend.list() else {
            return 0;
        };
        found
            .into_iter()
            .filter(|id| !known.contains(id))
            .filter(|id| self.backend.remove(*id).is_ok())
            .count()
    }

    #[cfg(test)]
    pub fn stored_ids(&self) -> Vec<Uuid> {
        self.backend.list().unwrap()
    }
}

/// Espressione regolare dei nomi delle credenziali dei segreti di questa app. Si accettano
/// solo identificativi fatti di lettere, cifre, punti e trattini: il punto si protegge,
/// il resto è già letterale.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn secret_target_pattern(service: &str) -> Option<String> {
    let plain = service
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-');
    if service.is_empty() || !plain {
        return None;
    }
    Some(format!(
        "^{SECRET_USER_PREFIX}[0-9a-f-]{{36}}\\.{}$",
        service.replace('.', "\\.")
    ))
}

/// Id del segreto dal nome della credenziale, solo nella forma che scrive questa app.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn secret_id_from_target(target: &str, service: &str) -> Option<Uuid> {
    let id = target
        .strip_prefix(SECRET_USER_PREFIX)?
        .strip_suffix(service)?
        .strip_suffix('.')?;
    let parsed = Uuid::parse_str(id).ok()?;
    (parsed.hyphenated().to_string() == id).then_some(parsed)
}

/// Credential Manager, tramite `keyring-core` (lo store è quello impostato da `init_store`).
pub struct KeyringBackend {
    service: String,
}

impl KeyringBackend {
    pub fn new(service: &str) -> Self {
        Self {
            service: service.to_owned(),
        }
    }

    fn entry(&self, id: Uuid) -> Result<Entry, CommandError> {
        let user = format!("{SECRET_USER_PREFIX}{id}");
        // Persistenza locale come per la chiave del database (lezione 34): i segreti non
        // seguono il profilo roaming. (v0.4.0)
        #[cfg(windows)]
        let entry = Entry::new_with_modifiers(
            &self.service,
            &user,
            &std::collections::HashMap::from([("persistence", "local")]),
        );
        #[cfg(not(windows))]
        let entry = Entry::new(&self.service, &user);
        entry.map_err(|_| CommandError::KEYSTORE_UNAVAILABLE)
    }
}

impl SecretBackend for KeyringBackend {
    fn put(&self, id: Uuid, value: &[u8]) -> Result<(), CommandError> {
        self.entry(id)?
            .set_secret(value)
            .map_err(|_| CommandError::KEYSTORE_UNAVAILABLE)
    }

    fn get(&self, id: Uuid) -> Result<Option<Vec<u8>>, CommandError> {
        match self.entry(id)?.get_secret() {
            Ok(bytes) => Ok(Some(bytes)),
            Err(KeyringError::NoEntry) => Ok(None),
            Err(_) => Err(CommandError::KEYSTORE_UNAVAILABLE),
        }
    }

    fn remove(&self, id: Uuid) -> Result<(), CommandError> {
        match self.entry(id)?.delete_credential() {
            Ok(()) | Err(KeyringError::NoEntry) => Ok(()),
            Err(_) => Err(CommandError::KEYSTORE_UNAVAILABLE),
        }
    }

    #[cfg(windows)]
    fn list(&self) -> Result<Vec<Uuid>, CommandError> {
        let pattern =
            secret_target_pattern(&self.service).ok_or(CommandError::KEYSTORE_UNAVAILABLE)?;
        let entries = Entry::search(&std::collections::HashMap::from([(
            "pattern",
            pattern.as_str(),
        )]))
        .map_err(|_| CommandError::KEYSTORE_UNAVAILABLE)?;
        Ok(entries
            .iter()
            .filter_map(|entry| entry.get_attributes().ok())
            .filter_map(|attributes| {
                secret_id_from_target(attributes.get("target_name")?, &self.service)
            })
            .collect())
    }

    /// Fuori da Windows lo store è quello simulato, senza persistenza: nulla da ripulire.
    #[cfg(not(windows))]
    fn list(&self) -> Result<Vec<Uuid>, CommandError> {
        Ok(Vec::new())
    }
}

/// Store in memoria per i test: stesso contratto, nessun effetto sul sistema.
#[cfg(test)]
#[derive(Default)]
pub struct MemoryBackend {
    values: std::sync::Mutex<std::collections::HashMap<Uuid, Vec<u8>>>,
}

#[cfg(test)]
impl SecretBackend for MemoryBackend {
    fn put(&self, id: Uuid, value: &[u8]) -> Result<(), CommandError> {
        self.values.lock().unwrap().insert(id, value.to_vec());
        Ok(())
    }

    fn get(&self, id: Uuid) -> Result<Option<Vec<u8>>, CommandError> {
        Ok(self.values.lock().unwrap().get(&id).cloned())
    }

    fn remove(&self, id: Uuid) -> Result<(), CommandError> {
        self.values.lock().unwrap().remove(&id);
        Ok(())
    }

    fn list(&self) -> Result<Vec<Uuid>, CommandError> {
        Ok(self.values.lock().unwrap().keys().copied().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SERVICE: &str = "com.example.portale";

    fn memory_vault() -> Vault {
        Vault::new(Box::new(MemoryBackend::default()))
    }

    #[test]
    fn the_value_is_lent_to_a_closure_and_a_missing_one_is_reported() {
        let vault = memory_vault();
        let id = Uuid::now_v7();
        vault.store(id, "s3greto").unwrap();
        assert_eq!(vault.with_value(id, |value| Ok(value.len())).unwrap(), 7);
        vault.remove(id).unwrap();
        vault.remove(id).unwrap();
        assert_eq!(
            vault.with_value(id, |_| Ok(())).unwrap_err(),
            CommandError::SECRET_MISSING
        );
    }

    #[test]
    fn the_sweep_removes_only_credentials_the_registry_does_not_know() {
        let vault = memory_vault();
        let (kept, orphan) = (Uuid::now_v7(), Uuid::now_v7());
        vault.store(kept, "a").unwrap();
        vault.store(orphan, "b").unwrap();
        assert_eq!(vault.sweep(&HashSet::from([kept])), 1);
        assert_eq!(vault.stored_ids(), [kept]);
    }

    #[test]
    fn target_names_are_recognised_only_in_the_exact_form_written_here() {
        let id = Uuid::now_v7();
        let target = format!("secret:{id}.{SERVICE}");
        assert_eq!(secret_id_from_target(&target, SERVICE), Some(id));
        // Casi negativi: la chiave del database, un'altra app, id non canonici o alterati.
        for other in [
            format!("database-key.{SERVICE}"),
            format!("secret:{id}.com.altro.app"),
            format!("secret:{}.{SERVICE}", id.simple()),
            format!(
                "secret:{}.{SERVICE}",
                id.hyphenated().to_string().to_uppercase()
            ),
            format!("secret:{id}{SERVICE}"),
            format!("xsecret:{id}.{SERVICE}"),
        ] {
            assert_eq!(secret_id_from_target(&other, SERVICE), None, "{other}");
        }
    }

    /// Il Credential Manager vero, solo su Windows (in CI): scrive, rilegge, cerca con il nome
    /// reale ed elimina. Trasforma in un fatto verificato a ogni commit l'uso dell'attributo
    /// `target_name`, preso dalla documentazione del crate. Un identificativo di prova unico
    /// tiene il test lontano dalle credenziali dell'app installata; la guardia le elimina
    /// anche se un'asserzione fallisce. (v0.5.0)
    #[cfg(windows)]
    #[test]
    fn the_real_credential_manager_round_trips_and_is_found_by_target_name() {
        struct Cleanup<'a>(&'a KeyringBackend, Uuid);
        impl Drop for Cleanup<'_> {
            fn drop(&mut self) {
                let _ = self.0.remove(self.1);
            }
        }
        crate::keystore::init_store().unwrap();
        let service = format!("com.example.prova{}", Uuid::now_v7().simple());
        let backend = KeyringBackend::new(&service);
        let id = Uuid::now_v7();
        let _cleanup = Cleanup(&backend, id);
        let value = "valore di prova, con àccenti".as_bytes();
        backend.put(id, value).unwrap();
        assert_eq!(backend.get(id).unwrap().as_deref(), Some(value));
        assert_eq!(backend.list().unwrap(), [id]);
        backend.remove(id).unwrap();
        assert_eq!(backend.get(id).unwrap(), None);
        assert!(backend.list().unwrap().is_empty());
    }

    #[test]
    fn the_search_pattern_protects_dots_and_refuses_odd_identifiers() {
        assert_eq!(
            secret_target_pattern(SERVICE).as_deref(),
            Some(r"^secret:[0-9a-f-]{36}\.com\.example\.portale$")
        );
        for odd in ["", "com.app(1)", "com app", "com.app*"] {
            assert_eq!(secret_target_pattern(odd), None, "{odd}");
        }
    }
}
