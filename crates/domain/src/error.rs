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

    #[error("errore del database: {0}")]
    Sqlite(rusqlite::Error),
}

impl Error {
    /// Codice stabile: una volta rilasciato non cambia, perché il frontend e le
    /// traduzioni dipendono da lui. (v0.1.0)
    pub fn code(&self) -> &'static str {
        match self {
            Self::WrongKeyOrCorrupt => "DB_WRONG_KEY",
            Self::CipherUnavailable => "DB_CIPHER_UNAVAILABLE",
            Self::SchemaTooNew { .. } => "DB_SCHEMA_TOO_NEW",
            Self::Sqlite(_) => "DB_ERROR",
        }
    }
}

impl From<rusqlite::Error> for Error {
    fn from(err: rusqlite::Error) -> Self {
        // SQLITE_NOTADB è la risposta di SQLCipher a una chiave sbagliata: la
        // riconosciamo qui, in un solo punto, invece di confrontare messaggi. (v0.1.0)
        match err.sqlite_error_code() {
            Some(ErrorCode::NotADatabase) => Self::WrongKeyOrCorrupt,
            _ => Self::Sqlite(err),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_unique_and_upper_snake_case() {
        let codes = [
            Error::WrongKeyOrCorrupt.code(),
            Error::CipherUnavailable.code(),
            Error::SchemaTooNew {
                found: 2,
                supported: 1,
            }
            .code(),
            Error::Sqlite(rusqlite::Error::InvalidQuery).code(),
        ];
        let mut sorted = codes.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), codes.len(), "codici duplicati: {codes:?}");
        for code in codes {
            assert!(
                code.chars().all(|c| c.is_ascii_uppercase() || c == '_'),
                "codice non valido: {code}"
            );
        }
    }
}
