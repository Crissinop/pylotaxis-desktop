//! Database locale cifrato con SQLCipher (A.7.4). (v0.1.0)

use std::fmt;
use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};

use crate::Error;

/// Lunghezza della chiave in byte: 256 bit, la dimensione della chiave AES di SQLCipher. (v0.1.0)
pub const KEY_LEN: usize = 32;

/// Chiave grezza del database.
///
/// È casuale, non una password: per questo si passa a SQLCipher in forma
/// esadecimale (`x'…'`), che salta la derivazione PBKDF2 pensata per le password.
/// Dalla v0.3.0 verrà generata e custodita nel Credential Manager. (v0.1.0)
pub struct DatabaseKey([u8; KEY_LEN]);

impl DatabaseKey {
    pub fn from_bytes(bytes: [u8; KEY_LEN]) -> Self {
        Self(bytes)
    }

    /// Chiave nuova dal generatore casuale del sistema operativo. (v0.2.0)
    pub fn generate() -> Result<Self, Error> {
        let mut bytes = [0_u8; KEY_LEN];
        getrandom::fill(&mut bytes).map_err(|_| Error::KeyInvalid)?;
        Ok(Self(bytes))
    }

    /// Ricostruisce la chiave letta dal Credential Manager, rifiutando lunghezze diverse.
    pub fn from_slice(bytes: &[u8]) -> Result<Self, Error> {
        <[u8; KEY_LEN]>::try_from(bytes)
            .map(Self)
            .map_err(|_| Error::KeyInvalid)
    }

    /// Byte della chiave, solo per salvarla nel Credential Manager. Il nome rende
    /// visibile nel codice ogni punto in cui il segreto esce da questo tipo. (v0.2.0)
    pub fn expose_secret(&self) -> &[u8; KEY_LEN] {
        &self.0
    }

    /// Valore per `PRAGMA key` nella forma a chiave grezza di SQLCipher.
    fn pragma_value(&self) -> String {
        format!("x'{}'", crate::hex::upper(&self.0))
    }
}

impl fmt::Debug for DatabaseKey {
    // La chiave non deve mai finire in un log, nemmeno per errore (A.7.4). (v0.1.0)
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DatabaseKey(<nascosta>)")
    }
}

struct Migration {
    version: u32,
    sql: &'static str,
}

/// Migrazioni in ordine, numerate da 1 senza buchi. Una migrazione rilasciata
/// non si modifica mai: se ne aggiunge una nuova (A.7.4). (v0.1.0)
const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        sql: include_str!("../migrations/0001_settings.sql"),
    },
    Migration {
        version: 2,
        sql: include_str!("../migrations/0002_registry.sql"),
    },
];

/// Versione di schema più recente che questa build sa gestire.
pub fn latest_schema_version() -> u32 {
    MIGRATIONS.last().map_or(0, |m| m.version)
}

/// Cosa fare all'avvio, secondo ciò che esiste su disco e nel Credential Manager. (v0.2.0)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyPlan {
    /// Usare la chiave salvata (con o senza database esistente).
    UseStored,
    /// Primo avvio: generare una chiave, salvarla e solo dopo creare il database, così
    /// un'interruzione non lascia mai un database senza la sua chiave.
    GenerateAndStore,
    /// Il database esiste ma la chiave no: fermarsi, mai sovrascrivere i dati.
    Refuse,
}

pub fn plan_key(database_exists: bool, key_stored: bool) -> KeyPlan {
    match (database_exists, key_stored) {
        (_, true) => KeyPlan::UseStored,
        (false, false) => KeyPlan::GenerateAndStore,
        (true, false) => KeyPlan::Refuse,
    }
}

/// Connessione aperta, verificata e migrata.
pub struct Database {
    pub(crate) conn: Connection,
}

impl Database {
    /// Apre il database cifrato in `path`, creandolo se non esiste.
    ///
    /// Rifiuta di proseguire se SQLCipher non è attivo, se la chiave è sbagliata
    /// o se lo schema è più recente di questa build: in nessun caso si scrive in
    /// chiaro o si toccano dati che non si capiscono. (v0.1.0)
    pub fn open(path: &Path, key: &DatabaseKey) -> Result<Self, Error> {
        let mut conn = Connection::open(path)?;

        // La chiave va impostata prima di qualunque altra istruzione.
        conn.pragma_update(None, "key", key.pragma_value())?;

        // Con SQLite senza cifratura il pragma è ignorato e non restituisce righe:
        // è il controllo che impedisce di creare per errore un database in chiaro.
        let cipher: Option<String> = conn
            .query_row("PRAGMA cipher_version", [], |row| row.get(0))
            .optional()?;
        if cipher.is_none_or(|v| v.is_empty()) {
            return Err(Error::CipherUnavailable);
        }

        // Prima lettura reale del file: con una chiave sbagliata fallisce qui.
        conn.query_row("SELECT count(*) FROM sqlite_master", [], |row| {
            row.get::<_, i64>(0)
        })?;

        conn.pragma_update(None, "foreign_keys", true)?;
        migrate(&mut conn)?;
        Ok(Self { conn })
    }

    /// Versione di schema registrata nel file.
    pub fn schema_version(&self) -> Result<u32, Error> {
        Ok(self
            .conn
            .pragma_query_value(None, "user_version", |row| row.get(0))?)
    }

    pub fn setting(&self, key: &str) -> Result<Option<String>, Error> {
        Ok(self
            .conn
            .query_row(
                "SELECT value FROM settings WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()?)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), Error> {
        self.conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }
}

/// Applica in ordine le migrazioni mancanti, ognuna nella sua transazione:
/// un errore a metà lascia il file alla versione precedente, non in uno stato misto.
fn migrate(conn: &mut Connection) -> Result<(), Error> {
    let current: u32 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    let supported = latest_schema_version();
    if current > supported {
        return Err(Error::SchemaTooNew {
            found: current,
            supported,
        });
    }
    for migration in MIGRATIONS.iter().filter(|m| m.version > current) {
        let tx = conn.transaction()?;
        tx.execute_batch(migration.sql)?;
        tx.pragma_update(None, "user_version", migration.version)?;
        tx.commit()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Intestazione dei file SQLite in chiaro.
    const PLAINTEXT_HEADER: &[u8] = b"SQLite format 3\0";

    fn key(fill: u8) -> DatabaseKey {
        DatabaseKey::from_bytes([fill; KEY_LEN])
    }

    fn temp_db() -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("portal.db");
        (dir, path)
    }

    #[test]
    fn creates_closes_and_reopens_with_the_same_key() {
        let (_dir, path) = temp_db();
        {
            let db = Database::open(&path, &key(7)).unwrap();
            db.set_setting("theme", "dark").unwrap();
        }
        let db = Database::open(&path, &key(7)).unwrap();
        assert_eq!(db.setting("theme").unwrap().as_deref(), Some("dark"));
    }

    #[test]
    fn rejects_a_wrong_key() {
        let (_dir, path) = temp_db();
        Database::open(&path, &key(1)).unwrap();
        let err = Database::open(&path, &key(2)).err().unwrap();
        assert!(matches!(err, Error::WrongKeyOrCorrupt), "{err:?}");
        assert_eq!(err.code(), "DB_WRONG_KEY");
    }

    #[test]
    fn writes_nothing_in_plaintext() {
        let (_dir, path) = temp_db();
        let marker = "valore-riconoscibile-in-chiaro";
        {
            let db = Database::open(&path, &key(9)).unwrap();
            db.set_setting("marker", marker).unwrap();
        }
        let bytes = std::fs::read(&path).unwrap();
        assert!(
            !bytes.starts_with(PLAINTEXT_HEADER),
            "intestazione in chiaro"
        );
        assert!(
            !bytes.windows(marker.len()).any(|w| w == marker.as_bytes()),
            "il valore compare in chiaro nel file"
        );
    }

    #[test]
    fn a_new_database_is_at_the_latest_schema() {
        let (_dir, path) = temp_db();
        let db = Database::open(&path, &key(3)).unwrap();
        assert_eq!(db.schema_version().unwrap(), latest_schema_version());
    }

    #[test]
    fn reopening_does_not_reapply_migrations() {
        let (_dir, path) = temp_db();
        Database::open(&path, &key(4))
            .unwrap()
            .set_setting("k", "v")
            .unwrap();
        // Una seconda esecuzione di CREATE TABLE fallirebbe: se l'apertura riesce,
        // le migrazioni già applicate sono state saltate.
        let db = Database::open(&path, &key(4)).unwrap();
        assert_eq!(db.setting("k").unwrap().as_deref(), Some("v"));
    }

    #[test]
    fn refuses_a_schema_newer_than_the_build() {
        let (_dir, path) = temp_db();
        let db = Database::open(&path, &key(5)).unwrap();
        db.conn.pragma_update(None, "user_version", 99_u32).unwrap();
        drop(db);
        let err = Database::open(&path, &key(5)).err().unwrap();
        assert!(
            matches!(err, Error::SchemaTooNew { found: 99, .. }),
            "{err:?}"
        );
    }

    #[test]
    fn migrations_are_numbered_from_one_without_gaps() {
        for (index, migration) in MIGRATIONS.iter().enumerate() {
            assert_eq!(migration.version as usize, index + 1);
        }
    }

    #[test]
    fn key_plan_never_overwrites_an_existing_database() {
        assert_eq!(plan_key(true, true), KeyPlan::UseStored);
        assert_eq!(plan_key(false, true), KeyPlan::UseStored);
        assert_eq!(plan_key(false, false), KeyPlan::GenerateAndStore);
        assert_eq!(plan_key(true, false), KeyPlan::Refuse);
    }

    #[test]
    fn generated_keys_are_random_and_round_trip() {
        let a = DatabaseKey::generate().unwrap();
        let b = DatabaseKey::generate().unwrap();
        assert_ne!(a.expose_secret(), b.expose_secret());
        let copy = DatabaseKey::from_slice(a.expose_secret()).unwrap();
        assert_eq!(copy.expose_secret(), a.expose_secret());
        assert!(matches!(
            DatabaseKey::from_slice(&[1, 2, 3]),
            Err(Error::KeyInvalid)
        ));
    }

    #[test]
    fn debug_output_never_shows_the_key() {
        let shown = format!("{:?}", key(0xAB));
        assert!(!shown.contains("AB"), "{shown}");
        assert_eq!(key(0xAB).pragma_value().len(), KEY_LEN * 2 + 3);
    }
}
