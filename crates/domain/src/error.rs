use rusqlite::ErrorCode;

/// Errori del dominio. Il messaggio serve ai log; all'interfaccia arriva solo
/// `code()`, che il frontend traduce nella lingua dell'utente (A.7.3, A.7.9). (v0.1.0)
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// La chiave non apre il file, oppure il file non è un database valido.
    /// SQLCipher non distingue i due casi, quindi non li distinguiamo nemmeno noi.
    #[error("la chiave non apre il database, oppure il file è danneggiato")]
    WrongKeyOrCorrupt,

    /// Il motore SQL non è SQLCipher: il database verrebbe scritto in chiaro.
    #[error("SQLCipher non è attivo: il database non sarebbe cifrato")]
    CipherUnavailable,

    /// Il file è stato scritto da una versione più recente dell'app.
    #[error("lo schema del database è alla versione {found}, questa build arriva alla {supported}")]
    SchemaTooNew { found: u32, supported: u32 },

    /// Esiste un database ma non la sua chiave: non lo si sovrascrive mai. (v0.2.0)
    #[error("il database esiste ma la sua chiave non è nel Credential Manager")]
    KeyMissing,

    /// La chiave salvata non ha la lunghezza attesa. (v0.2.0)
    #[error("la chiave salvata non è valida")]
    KeyInvalid,

    #[error("il nome non è valido")]
    NameInvalid,

    #[error("esiste già un elemento con questo nome")]
    NameDuplicate,

    #[error("l'indirizzo non è valido")]
    UrlInvalid,

    /// Schema non ammesso: `file:`, `javascript:`, schemi noti per abusi. (v0.2.0)
    #[error("lo schema dell'indirizzo non è ammesso")]
    SchemeNotAllowed,

    #[error("il file scelto non è un eseguibile valido")]
    ExecutableInvalid,

    /// L'eseguibile registrato non esiste più nel percorso salvato.
    #[error("l'eseguibile registrato non esiste più")]
    ExecutableMissing,

    /// L'eseguibile è cambiato dall'ultima conferma: non si avvia senza un nuovo consenso.
    #[error("l'eseguibile è cambiato dall'ultima conferma")]
    HashMismatch,

    #[error("un tag non è valido o sono troppi")]
    TagInvalid,

    #[error("elemento non trovato")]
    NotFound,

    /// Il PIN nuovo non è fatto di 6–16 cifre. (v0.3.0)
    #[error("il PIN deve avere da 6 a 16 cifre")]
    PinInvalid,

    /// Il PIN nuovo è tra i più facili da indovinare (000000, 123456…). (v0.3.0)
    #[error("il PIN è troppo semplice")]
    PinTooSimple,

    /// Si chiede di verificare un PIN, ma il blocco non è configurato. (v0.3.0)
    #[error("il PIN non è impostato")]
    PinNotSet,

    #[error("il PIN non è corretto")]
    PinWrong,

    /// Troppi tentativi falliti: il prossimo è ammesso dopo un'attesa. (v0.3.0)
    #[error("troppi tentativi: attendere prima di riprovare")]
    PinThrottled,

    /// Il calcolo o la lettura dell'impronta del PIN non sono riusciti. (v0.3.0)
    #[error("impronta del PIN non calcolabile")]
    PinHashFailed,

    #[error("il tempo di inattività non è ammesso")]
    IdleInvalid,

    /// Etichetta di un segreto vuota, troppo lunga o con caratteri di controllo. (v0.4.0)
    #[error("etichetta del segreto non valida")]
    SecretLabelInvalid,

    #[error("etichetta già usata per un altro segreto di questa app")]
    SecretLabelDuplicate,

    #[error("nome utente non valido")]
    SecretUsernameInvalid,

    /// Valore vuoto o con il carattere NUL. (v0.4.0)
    #[error("valore del segreto non valido")]
    SecretValueInvalid,

    /// Oltre i 2560 byte che il Credential Manager accetta. (v0.4.0)
    #[error("valore del segreto troppo lungo")]
    SecretTooLong,

    /// La scorciatoia non è tra quelle dell'elenco (v0.5.0).
    #[error("scorciatoia non ammessa")]
    ShortcutInvalid,

    /// Ambiente sconosciuto: ammessi sviluppo, collaudo, produzione o nessuno (v0.6.0).
    #[error("ambiente non valido")]
    EnvironmentInvalid,

    /// Il controllo dello stato vale solo per le app web (v0.6.0).
    #[error("controllo dello stato solo per le app web")]
    HealthCheckWebOnly,

    /// Un gruppo ha più app del massimo consentito (v0.6.0).
    #[error("troppe app nel gruppo")]
    GroupTooLarge,

    /// La stessa app compare due volte nello stesso gruppo (v0.6.0).
    #[error("app ripetuta nel gruppo")]
    GroupAppDuplicate,

    #[error("errore di lettura del file: {0}")]
    Io(#[from] std::io::Error),

    #[error("errore del database: {0}")]
    Sqlite(rusqlite::Error),
}

impl Error {
    /// Tutti i codici, nell'ordine delle varianti. Un test verifica che coincidano con
    /// `code()`; un altro, nel guscio, che ciascuno abbia una traduzione. (v0.2.0)
    pub const ALL_CODES: &[&str] = &[
        "DB_WRONG_KEY",
        "DB_CIPHER_UNAVAILABLE",
        "DB_SCHEMA_TOO_NEW",
        "DB_KEY_MISSING",
        "DB_KEY_INVALID",
        "NAME_INVALID",
        "NAME_DUPLICATE",
        "URL_INVALID",
        "SCHEME_NOT_ALLOWED",
        "EXECUTABLE_INVALID",
        "EXECUTABLE_MISSING",
        "HASH_MISMATCH",
        "TAG_INVALID",
        "NOT_FOUND",
        "PIN_INVALID",
        "PIN_TOO_SIMPLE",
        "PIN_NOT_SET",
        "PIN_WRONG",
        "PIN_THROTTLED",
        "PIN_HASH_FAILED",
        "IDLE_INVALID",
        "SECRET_LABEL_INVALID",
        "SECRET_LABEL_DUPLICATE",
        "SECRET_USERNAME_INVALID",
        "SECRET_VALUE_INVALID",
        "SECRET_TOO_LONG",
        "SHORTCUT_INVALID",
        "ENVIRONMENT_INVALID",
        "HEALTH_CHECK_WEB_ONLY",
        "GROUP_TOO_LARGE",
        "GROUP_APP_DUPLICATE",
        "IO_ERROR",
        "DB_ERROR",
    ];

    /// Codice stabile: una volta rilasciato non cambia, perché il frontend e le
    /// traduzioni dipendono da lui. (v0.1.0)
    pub fn code(&self) -> &'static str {
        match self {
            Self::WrongKeyOrCorrupt => "DB_WRONG_KEY",
            Self::CipherUnavailable => "DB_CIPHER_UNAVAILABLE",
            Self::SchemaTooNew { .. } => "DB_SCHEMA_TOO_NEW",
            Self::KeyMissing => "DB_KEY_MISSING",
            Self::KeyInvalid => "DB_KEY_INVALID",
            Self::NameInvalid => "NAME_INVALID",
            Self::NameDuplicate => "NAME_DUPLICATE",
            Self::UrlInvalid => "URL_INVALID",
            Self::SchemeNotAllowed => "SCHEME_NOT_ALLOWED",
            Self::ExecutableInvalid => "EXECUTABLE_INVALID",
            Self::ExecutableMissing => "EXECUTABLE_MISSING",
            Self::HashMismatch => "HASH_MISMATCH",
            Self::TagInvalid => "TAG_INVALID",
            Self::NotFound => "NOT_FOUND",
            Self::PinInvalid => "PIN_INVALID",
            Self::PinTooSimple => "PIN_TOO_SIMPLE",
            Self::PinNotSet => "PIN_NOT_SET",
            Self::PinWrong => "PIN_WRONG",
            Self::PinThrottled => "PIN_THROTTLED",
            Self::PinHashFailed => "PIN_HASH_FAILED",
            Self::IdleInvalid => "IDLE_INVALID",
            Self::SecretLabelInvalid => "SECRET_LABEL_INVALID",
            Self::SecretLabelDuplicate => "SECRET_LABEL_DUPLICATE",
            Self::SecretUsernameInvalid => "SECRET_USERNAME_INVALID",
            Self::SecretValueInvalid => "SECRET_VALUE_INVALID",
            Self::SecretTooLong => "SECRET_TOO_LONG",
            Self::ShortcutInvalid => "SHORTCUT_INVALID",
            Self::EnvironmentInvalid => "ENVIRONMENT_INVALID",
            Self::HealthCheckWebOnly => "HEALTH_CHECK_WEB_ONLY",
            Self::GroupTooLarge => "GROUP_TOO_LARGE",
            Self::GroupAppDuplicate => "GROUP_APP_DUPLICATE",
            Self::Io(_) => "IO_ERROR",
            Self::Sqlite(_) => "DB_ERROR",
        }
    }
}

impl From<rusqlite::Error> for Error {
    fn from(err: rusqlite::Error) -> Self {
        // SQLITE_NOTADB è la risposta di SQLCipher a una chiave sbagliata; un vincolo UNIQUE
        // violato è un nome duplicato; una chiave esterna violata punta a un elemento che non
        // esiste. Si riconoscono qui, in un solo punto. (v0.2.0)
        match err.sqlite_error_code() {
            Some(ErrorCode::NotADatabase) => Self::WrongKeyOrCorrupt,
            Some(ErrorCode::ConstraintViolation) => {
                let message = err.to_string();
                if message.contains("UNIQUE constraint failed") {
                    Self::NameDuplicate
                } else if message.contains("FOREIGN KEY constraint failed") {
                    Self::NotFound
                } else {
                    Self::Sqlite(err)
                }
            }
            _ => Self::Sqlite(err),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one_of_each() -> Vec<Error> {
        vec![
            Error::WrongKeyOrCorrupt,
            Error::CipherUnavailable,
            Error::SchemaTooNew {
                found: 2,
                supported: 1,
            },
            Error::KeyMissing,
            Error::KeyInvalid,
            Error::NameInvalid,
            Error::NameDuplicate,
            Error::UrlInvalid,
            Error::SchemeNotAllowed,
            Error::ExecutableInvalid,
            Error::ExecutableMissing,
            Error::HashMismatch,
            Error::TagInvalid,
            Error::NotFound,
            Error::PinInvalid,
            Error::PinTooSimple,
            Error::PinNotSet,
            Error::PinWrong,
            Error::PinThrottled,
            Error::PinHashFailed,
            Error::IdleInvalid,
            Error::SecretLabelInvalid,
            Error::SecretLabelDuplicate,
            Error::SecretUsernameInvalid,
            Error::SecretValueInvalid,
            Error::SecretTooLong,
            Error::ShortcutInvalid,
            Error::EnvironmentInvalid,
            Error::HealthCheckWebOnly,
            Error::GroupTooLarge,
            Error::GroupAppDuplicate,
            Error::Io(std::io::Error::other("prova")),
            Error::Sqlite(rusqlite::Error::InvalidQuery),
        ]
    }

    #[test]
    fn all_codes_lists_every_variant_in_order() {
        let codes: Vec<&str> = one_of_each().iter().map(Error::code).collect();
        assert_eq!(codes, Error::ALL_CODES);
    }

    #[test]
    fn codes_are_unique_and_upper_snake_case() {
        let mut sorted = Error::ALL_CODES.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), Error::ALL_CODES.len(), "codici duplicati");
        for code in Error::ALL_CODES {
            assert!(
                code.chars().all(|c| c.is_ascii_uppercase() || c == '_'),
                "codice non valido: {code}"
            );
        }
    }
}
