//! Stato delle app web: regole pure (v0.6.0). La richiesta la fa il guscio; qui si decide che
//! cosa significa la risposta e quando va ripetuta.

use std::time::Duration;

/// Validità di un risultato: si ricontrolla solo ciò che è più vecchio (A.7.6).
pub const HEALTH_TTL: Duration = Duration::from_secs(60);

/// Attesa massima di una risposta, connessione compresa.
pub const HEALTH_TIMEOUT: Duration = Duration::from_secs(5);

/// Esito di una richiesta, come lo riferisce il guscio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Probe {
    /// Il server ha risposto con questo codice HTTP.
    Response(u16),
    /// La connessione sicura non si è stabilita: certificato non valido o TLS rifiutato.
    TlsFailure,
    /// Nessuna risposta: nome sconosciuto, connessione rifiutata, tempo scaduto.
    NoResponse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    Active,
    ServerError,
    Unreachable,
    InvalidCertificate,
}

impl HealthStatus {
    /// Codice stabile per il frontend, che lo traduce.
    pub fn code(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::ServerError => "server_error",
            Self::Unreachable => "unreachable",
            Self::InvalidCertificate => "invalid_certificate",
        }
    }
}

/// Una risposta sotto 500 vuol dire che il server risponde, anche con 401 o 404: il controllo
/// non manda credenziali. Un reindirizzamento non si segue e conta come risposta.
pub fn classify(probe: Probe) -> HealthStatus {
    match probe {
        Probe::Response(code) if code < 500 => HealthStatus::Active,
        Probe::Response(_) => HealthStatus::ServerError,
        Probe::TlsFailure => HealthStatus::InvalidCertificate,
        Probe::NoResponse => HealthStatus::Unreachable,
    }
}

/// Vero se il risultato va ripetuto: mai controllato, o più vecchio della validità.
pub fn is_stale(age: Option<Duration>) -> bool {
    age.is_none_or(|age| age >= HEALTH_TTL)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn responses_below_500_are_active_and_the_rest_are_server_errors() {
        for code in [200, 204, 301, 302, 401, 403, 404, 499] {
            assert_eq!(
                classify(Probe::Response(code)),
                HealthStatus::Active,
                "{code}"
            );
        }
        for code in [500, 502, 503, 599] {
            assert_eq!(
                classify(Probe::Response(code)),
                HealthStatus::ServerError,
                "{code}"
            );
        }
        assert_eq!(
            classify(Probe::TlsFailure),
            HealthStatus::InvalidCertificate
        );
        assert_eq!(classify(Probe::NoResponse), HealthStatus::Unreachable);
    }

    #[test]
    fn a_result_is_repeated_only_once_expired() {
        assert!(is_stale(None));
        assert!(!is_stale(Some(Duration::from_secs(59))));
        assert!(is_stale(Some(HEALTH_TTL)));
    }

    #[test]
    fn codes_are_stable() {
        let codes = [
            HealthStatus::Active,
            HealthStatus::ServerError,
            HealthStatus::Unreachable,
            HealthStatus::InvalidCertificate,
        ]
        .map(HealthStatus::code);
        assert_eq!(
            codes,
            [
                "active",
                "server_error",
                "unreachable",
                "invalid_certificate"
            ]
        );
    }
}
