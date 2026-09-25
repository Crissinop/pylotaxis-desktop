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
