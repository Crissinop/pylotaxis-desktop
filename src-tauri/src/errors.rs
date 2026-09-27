//! Errore restituito dai comandi: solo un codice stabile, che il frontend traduce (A.7.9).
//! Nessun messaggio di Rust, nessun percorso, nessun dettaglio interno raggiunge il webview. (v0.2.0)

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CommandError {
    pub code: &'static str,
}

impl CommandError {
    pub const KEYSTORE_UNAVAILABLE: Self = Self::new("KEYSTORE_UNAVAILABLE");
    pub const APP_DATA_UNAVAILABLE: Self = Self::new("APP_DATA_UNAVAILABLE");
    pub const STATE_UNAVAILABLE: Self = Self::new("STATE_UNAVAILABLE");
    pub const DIALOG_FAILED: Self = Self::new("DIALOG_FAILED");
    pub const PICK_EXPIRED: Self = Self::new("PICK_EXPIRED");
    pub const LAUNCH_FAILED: Self = Self::new("LAUNCH_FAILED");
    /// L'app è bloccata e il comando non serve a sbloccarla (v0.3.0).
    pub const LOCKED: Self = Self::new("LOCKED");
    /// Un prompt di sistema chiesto con la finestra in secondo piano (A.7.5). (v0.3.0)
    pub const NOT_FOREGROUND: Self = Self::new("NOT_FOREGROUND");
    pub const HELLO_NOT_ENABLED: Self = Self::new("HELLO_NOT_ENABLED");
    pub const HELLO_NO_DEVICE: Self = Self::new("HELLO_NO_DEVICE");
    pub const HELLO_NOT_CONFIGURED: Self = Self::new("HELLO_NOT_CONFIGURED");
    pub const HELLO_DISABLED_BY_POLICY: Self = Self::new("HELLO_DISABLED_BY_POLICY");
    pub const HELLO_BUSY: Self = Self::new("HELLO_BUSY");
    pub const HELLO_RETRIES_EXHAUSTED: Self = Self::new("HELLO_RETRIES_EXHAUSTED");
    /// Annullato dall'utente, o da una richiesta più recente: l'interfaccia non lo segnala.
    pub const HELLO_CANCELED: Self = Self::new("HELLO_CANCELED");
    pub const HELLO_FAILED: Self = Self::new("HELLO_FAILED");
    /// Il registro conosce il segreto, ma il Credential Manager non ha il valore (v0.4.0).
    pub const SECRET_MISSING: Self = Self::new("SECRET_MISSING");
    /// Gli appunti non si aprono (li tiene un'altra app) o non accettano i dati (v0.4.0).
    pub const CLIPBOARD_UNAVAILABLE: Self = Self::new("CLIPBOARD_UNAVAILABLE");
    /// Windows non concede la scorciatoia: la usa già un'altra app (v0.5.0).
    pub const SHORTCUT_TAKEN: Self = Self::new("SHORTCUT_TAKEN");
    /// L'icona nella tray non si crea: area di notifica non disponibile (v0.5.0).
    pub const TRAY_UNAVAILABLE: Self = Self::new("TRAY_UNAVAILABLE");

    const fn new(code: &'static str) -> Self {
        Self { code }
    }
}

/// Codici propri del guscio, oltre a quelli del dominio (`domain::Error::ALL_CODES`).
/// Serve ai test: verificano che ognuno abbia una traduzione in ogni lingua.
#[cfg(test)]
pub const SHELL_CODES: &[&str] = &[
    CommandError::KEYSTORE_UNAVAILABLE.code,
    CommandError::APP_DATA_UNAVAILABLE.code,
    CommandError::STATE_UNAVAILABLE.code,
    CommandError::DIALOG_FAILED.code,
    CommandError::PICK_EXPIRED.code,
    CommandError::LAUNCH_FAILED.code,
    CommandError::LOCKED.code,
    CommandError::NOT_FOREGROUND.code,
    CommandError::HELLO_NOT_ENABLED.code,
    CommandError::HELLO_NO_DEVICE.code,
    CommandError::HELLO_NOT_CONFIGURED.code,
    CommandError::HELLO_DISABLED_BY_POLICY.code,
    CommandError::HELLO_BUSY.code,
    CommandError::HELLO_RETRIES_EXHAUSTED.code,
    CommandError::HELLO_CANCELED.code,
    CommandError::HELLO_FAILED.code,
    CommandError::SECRET_MISSING.code,
    CommandError::CLIPBOARD_UNAVAILABLE.code,
    CommandError::SHORTCUT_TAKEN.code,
    CommandError::TRAY_UNAVAILABLE.code,
];

impl From<domain::Error> for CommandError {
    fn from(error: domain::Error) -> Self {
        // Il dettaglio resta nel processo Rust: utile in sviluppo, mai nel webview.
        #[cfg(debug_assertions)]
        eprintln!("errore di dominio: {error}");
        Self { code: error.code() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ogni codice che può arrivare all'interfaccia ha una traduzione in ogni lingua,
    /// sotto `errors.<CODICE>` (A.6 n. 6, A.7.9). I file letti sono gli stessi del frontend.
    #[test]
    fn every_error_code_is_translated() {
        let locales = [
            ("it", include_str!("../../src/i18n/locales/it.json")),
            ("en", include_str!("../../src/i18n/locales/en.json")),
        ];
        for (language, source) in locales {
            let json: serde_json::Value = serde_json::from_str(source).unwrap();
            let errors = json["errors"].as_object().unwrap();
            for code in domain::Error::ALL_CODES
                .iter()
                .chain(SHELL_CODES)
                .chain(&["UNKNOWN"])
            {
                let text = errors
                    .get(*code)
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                assert!(!text.trim().is_empty(), "{language}: manca errors.{code}");
            }
        }
    }

    #[test]
    fn shell_codes_do_not_collide_with_domain_codes() {
        for code in SHELL_CODES {
            assert!(!domain::Error::ALL_CODES.contains(code), "{code} duplicato");
        }
    }
}
