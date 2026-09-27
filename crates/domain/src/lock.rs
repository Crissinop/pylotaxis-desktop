//! Blocco del portale: PIN, attesa dopo i tentativi falliti, inattività (v0.3.0).
//!
//! Qui si decide; il guscio applica il blocco ai comandi e parla con Windows (A.7.3).
//! Il PIN non esce da questo modulo: nel database va solo la sua impronta Argon2id, e
//! nessuna funzione pubblica restituisce l'impronta (A.7.10).
//!
//! È un blocco di presenza: ferma chi siede alla tastiera. Non protegge da codice già in
//! esecuzione nella sessione di Windows, che può leggere la chiave del database dal
//! Credential Manager e con essa l'intero registro.

use argon2::password_hash::{self, PasswordHasher, PasswordVerifier};
use argon2::{Algorithm, Argon2, Params, Version};
use rusqlite::params;

use crate::Error;
use crate::db::Database;

/// Lunghezza del PIN, in cifre. Il minimo è una decisione della v0.3.0; il massimo limita
/// l'input senza pesare su nessun uso reale.
pub const PIN_MIN_DIGITS: usize = 6;
pub const PIN_MAX_DIGITS: usize = 16;

/// Inattività: valore predefinito (lo scrive anche la migrazione 0003) e limiti ammessi.
pub const IDLE_DEFAULT_MINUTES: u32 = 15;
pub const IDLE_MIN_MINUTES: u32 = 1;
pub const IDLE_MAX_MINUTES: u32 = 240;

/// Tentativi senza attesa; dopo, l'attesa parte da 30 secondi e raddoppia fino a 15 minuti.
/// Con un PIN di 6 cifre, indovinarlo alla tastiera richiede in media circa 14 anni.
pub const PIN_FREE_ATTEMPTS: u32 = 5;
pub const PIN_FIRST_WAIT_MS: u64 = 30_000;
pub const PIN_MAX_WAIT_MS: u64 = 15 * 60_000;

/// Argon2id con la seconda configurazione raccomandata dalla RFC 9106 (§4): 64 MiB,
/// 3 passate, 4 corsie. I parametri finiscono nella stringa PHC, quindi alzarli in futuro
/// non invalida i PIN già salvati.
const PIN_KDF_MEMORY_KIB: u32 = 64 * 1024;
const PIN_KDF_PASSES: u32 = 3;
const PIN_KDF_LANES: u32 = 4;

/// Configurazione del blocco come la vede il resto dell'app: dice se il PIN c'è, mai quale
/// sia né quale sia la sua impronta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LockSettings {
    pub pin_set: bool,
    pub hello_enabled: bool,
    /// `None` = nessun blocco per inattività.
    pub idle_minutes: Option<u32>,
    pub failed_attempts: u32,
    pub last_failure_ms: Option<i64>,
    /// Numero di cifre del PIN, per lo sblocco senza Invio; `None` = non ancora nota (v0.7.0).
    pub pin_length: Option<u32>,
}

impl LockSettings {
    /// Millisecondi prima del prossimo tentativo di PIN ammesso.
    pub fn retry_after_ms(&self, now_ms: i64) -> u64 {
        pin_retry_after_ms(self.failed_attempts, self.last_failure_ms, now_ms)
    }
}

/// Riga completa, impronta compresa: resta privata a questo modulo.
struct LockRow {
    pin_hash: Option<String>,
    settings: LockSettings,
}

/// Controlla un PIN nuovo: solo cifre ASCII, lunghezza ammessa, e non tra i più facili da
/// indovinare (cifre tutte uguali, sequenze di passo uno come 123456 o 987654), che sono
/// tra i primi tentativi di chiunque.
pub fn validate_new_pin(pin: &str) -> Result<(), Error> {
    let digits = pin.as_bytes();
    let length_ok = (PIN_MIN_DIGITS..=PIN_MAX_DIGITS).contains(&digits.len());
    if !length_ok || !digits.iter().all(u8::is_ascii_digit) {
        return Err(Error::PinInvalid);
    }
    if is_trivial(digits) {
        return Err(Error::PinTooSimple);
    }
    Ok(())
}

/// Vero se tutte le cifre avanzano dello stesso passo, e il passo è -1, 0 o +1.
fn is_trivial(digits: &[u8]) -> bool {
    let mut steps = digits
        .windows(2)
        .map(|pair| i16::from(pair[1]) - i16::from(pair[0]));
    let Some(first) = steps.next() else {
        return true;
    };
    (-1..=1).contains(&first) && steps.all(|step| step == first)
}

/// Attesa imposta dopo `failures` tentativi falliti consecutivi.
pub fn pin_wait_ms(failures: u32) -> u64 {
    let Some(doublings) = failures.checked_sub(PIN_FREE_ATTEMPTS) else {
        return 0;
    };
    2_u64
        .checked_pow(doublings)
        .and_then(|factor| PIN_FIRST_WAIT_MS.checked_mul(factor))
        .map_or(PIN_MAX_WAIT_MS, |wait| wait.min(PIN_MAX_WAIT_MS))
}

/// Millisecondi prima del prossimo tentativo ammesso.
///
/// Un orologio tornato indietro (ora precedente all'ultimo errore) non azzera l'attesa: la
/// si conta per intero. `check_pin` riallinea poi l'ultimo errore all'ora attuale, così una
/// correzione dell'orologio allunga l'attesa al massimo di un giro e non chiude fuori l'utente.
pub fn pin_retry_after_ms(failures: u32, last_failure_ms: Option<i64>, now_ms: i64) -> u64 {
    let wait = pin_wait_ms(failures);
    match last_failure_ms {
        _ if wait == 0 => 0,
        Some(last) if now_ms >= last => {
            let elapsed = u64::try_from(now_ms.saturating_sub(last)).unwrap_or(u64::MAX);
            wait.saturating_sub(elapsed)
        }
        // Orologio indietro, oppure errori senza data (dato alterato): attesa intera.
        _ => wait,
    }
}

/// Cifre di un PIN valido (6–16 cifre ASCII): la conversione non può fallire; per un
/// valore fuori dall'intervallo lo schema rifiuta la scrittura.
fn pin_length(pin: &str) -> i64 {
    i64::try_from(pin.len()).unwrap_or(i64::MAX)
}

fn pin_kdf() -> Result<Argon2<'static>, Error> {
    let params = Params::new(PIN_KDF_MEMORY_KIB, PIN_KDF_PASSES, PIN_KDF_LANES, None)
        .map_err(|_| Error::PinHashFailed)?;
    Ok(Argon2::new(Algorithm::Argon2id, Version::V0x13, params))
}

/// Impronta PHC del PIN, con un sale casuale di 16 byte dal generatore del sistema.
fn hash_pin(pin: &str) -> Result<String, Error> {
    pin_kdf()?
        .hash_password(pin.as_bytes())
        .map(|hash| hash.to_string())
        .map_err(|_| Error::PinHashFailed)
}

/// La verifica usa i parametri scritti nell'impronta, non quelli correnti; il confronto
/// finale è a tempo costante (tipo `Output` del crate `phc`).
fn pin_matches(pin: &str, stored: &str) -> Result<bool, Error> {
    match pin_kdf()?.verify_password(pin.as_bytes(), stored) {
        Ok(()) => Ok(true),
        Err(password_hash::Error::PasswordInvalid) => Ok(false),
        Err(_) => Err(Error::PinHashFailed),
    }
}

impl Database {
    fn lock_row(&self) -> Result<LockRow, Error> {
        Ok(self.conn.query_row(
            "SELECT pin_hash, hello_enabled, idle_minutes, failed_attempts, last_failure_ms,
                    pin_length
             FROM lock_config WHERE id = 1",
            [],
            |row| {
                let pin_hash: Option<String> = row.get(0)?;
                Ok(LockRow {
                    settings: LockSettings {
                        pin_set: pin_hash.is_some(),
                        hello_enabled: row.get(1)?,
                        idle_minutes: row.get(2)?,
                        failed_attempts: row.get(3)?,
                        last_failure_ms: row.get(4)?,
                        pin_length: row.get(5)?,
                    },
                    pin_hash,
                })
            },
        )?)
    }

    pub fn lock_settings(&self) -> Result<LockSettings, Error> {
        Ok(self.lock_row()?.settings)
    }

    /// Verifica un PIN e ne registra l'esito.
    ///
    /// Il tentativo si conta prima di calcolare l'impronta: un'interruzione a metà (app chiusa
    /// dal Task Manager) non regala un tentativo. Il guscio tiene il database dietro un mutex
    /// per tutta la chiamata: due tentativi paralleli non leggono lo stesso contatore.
    pub fn check_pin(&self, pin: &str, now_ms: i64) -> Result<(), Error> {
        let row = self.lock_row()?;
        let stored = row.pin_hash.ok_or(Error::PinNotSet)?;
        if row.settings.retry_after_ms(now_ms) > 0 {
            if row
                .settings
                .last_failure_ms
                .is_some_and(|last| now_ms < last)
            {
                self.conn.execute(
                    "UPDATE lock_config SET last_failure_ms = ?1 WHERE id = 1",
                    params![now_ms],
                )?;
            }
            return Err(Error::PinThrottled);
        }
        self.conn.execute(
            "UPDATE lock_config
             SET failed_attempts = failed_attempts + 1, last_failure_ms = ?1
             WHERE id = 1",
            params![now_ms],
        )?;
        if !pin_matches(pin, &stored)? {
            return Err(Error::PinWrong);
        }
        // Un PIN giusto rivela la propria lunghezza: la si impara se manca (PIN impostati
        // prima della v0.7.0), così dal prossimo sblocco basta digitarlo. (v0.7.0)
        self.conn.execute(
            "UPDATE lock_config
             SET failed_attempts = 0, last_failure_ms = NULL,
                 pin_length = coalesce(pin_length, ?1)
             WHERE id = 1",
            params![pin_length(pin)],
        )?;
        Ok(())
    }

    /// Imposta o cambia il PIN. Se ce n'è già uno serve quello attuale, e la sua verifica
    /// conta come un tentativo: le impostazioni non sono una strada per provare PIN senza attesa.
    pub fn set_pin(&self, current: Option<&str>, new_pin: &str, now_ms: i64) -> Result<(), Error> {
        // Prima il PIN nuovo: un errore di battitura lì non consuma un tentativo.
        validate_new_pin(new_pin)?;
        if self.lock_row()?.pin_hash.is_some() {
            self.check_pin(current.unwrap_or_default(), now_ms)?;
        }
        let hash = hash_pin(new_pin)?;
        self.conn.execute(
            "UPDATE lock_config
             SET pin_hash = ?1, pin_length = ?2, failed_attempts = 0, last_failure_ms = NULL
             WHERE id = 1",
            params![hash, pin_length(new_pin)],
        )?;
        Ok(())
    }

    /// Spegne il blocco togliendo PIN e Windows Hello, previa verifica del PIN attuale.
    /// L'inattività resta com'era, pronta per quando il blocco verrà riacceso.
    pub fn disable_lock(&self, current: &str, now_ms: i64) -> Result<(), Error> {
        self.check_pin(current, now_ms)?;
        self.conn.execute(
            "UPDATE lock_config
             SET pin_hash = NULL, pin_length = NULL, hello_enabled = 0, failed_attempts = 0,
                 last_failure_ms = NULL
             WHERE id = 1",
            [],
        )?;
        Ok(())
    }

    /// Windows Hello si accende solo se esiste il PIN di ripiego (vincolo anche nello schema).
    pub fn set_hello_enabled(&self, enabled: bool) -> Result<(), Error> {
        if enabled && !self.lock_settings()?.pin_set {
            return Err(Error::PinNotSet);
        }
        self.conn.execute(
            "UPDATE lock_config SET hello_enabled = ?1 WHERE id = 1",
            params![enabled],
        )?;
        Ok(())
    }

    pub fn set_idle_minutes(&self, minutes: Option<u32>) -> Result<(), Error> {
        if minutes.is_some_and(|m| !(IDLE_MIN_MINUTES..=IDLE_MAX_MINUTES).contains(&m)) {
            return Err(Error::IdleInvalid);
        }
        self.conn.execute(
            "UPDATE lock_config SET idle_minutes = ?1 WHERE id = 1",
            params![minutes],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::tests::open_db;

    const PIN: &str = "482915";
    const OTHER_PIN: &str = "730268";
    const T0: i64 = 1_790_000_000_000;

    #[test]
    fn new_pins_need_six_to_sixteen_ascii_digits() {
        for valid in ["482915", "4829150", "0000000000000001", "1029384756102938"] {
            assert!(validate_new_pin(valid).is_ok(), "{valid}");
        }
        // Casi negativi (A.6 n. 9): troppo corti o lunghi, spazi, lettere, cifre non ASCII.
        for invalid in [
            "",
            "48291",
            "48291504829150482",
            " 482915",
            "48 2915",
            "48a915",
            "４８２９１５",
            "482915\n",
        ] {
            assert!(
                matches!(validate_new_pin(invalid), Err(Error::PinInvalid)),
                "{invalid:?}"
            );
        }
    }

    #[test]
    fn the_most_guessable_pins_are_refused() {
        for trivial in [
            "000000",
            "999999",
            "123456",
            "012345",
            "3456789",
            "654321",
            "9876543210",
        ] {
            assert!(
                matches!(validate_new_pin(trivial), Err(Error::PinTooSimple)),
                "{trivial}"
            );
        }
        // Somigliano a una sequenza ma non lo sono: restano ammessi.
        for fine in ["123457", "112233", "135791", "901234"] {
            assert!(validate_new_pin(fine).is_ok(), "{fine}");
        }
    }

    #[test]
    fn the_wait_starts_after_the_free_attempts_doubles_and_is_capped() {
        let waits: Vec<u64> = (0..=12).map(pin_wait_ms).collect();
        assert_eq!(
            waits,
            [
                0, 0, 0, 0, 0, 30_000, 60_000, 120_000, 240_000, 480_000, 900_000, 900_000, 900_000
            ]
        );
        assert_eq!(pin_wait_ms(u32::MAX), PIN_MAX_WAIT_MS);
    }

    #[test]
    fn the_remaining_wait_counts_from_the_last_failure() {
        assert_eq!(pin_retry_after_ms(5, Some(T0), T0 + 10_000), 20_000);
        assert_eq!(pin_retry_after_ms(5, Some(T0), T0 + 30_000), 0);
        assert_eq!(pin_retry_after_ms(4, Some(T0), T0), 0);
        // Orologio tornato indietro, o errori senza data: attesa intera, mai zero.
        assert_eq!(pin_retry_after_ms(5, Some(T0), T0 - 3_600_000), 30_000);
        assert_eq!(pin_retry_after_ms(6, None, T0), 60_000);
    }

    #[test]
    fn a_new_database_starts_unlocked_with_the_default_idle_time() {
        let (_dir, db) = open_db();
        assert_eq!(
            db.lock_settings().unwrap(),
            LockSettings {
                pin_set: false,
                hello_enabled: false,
                idle_minutes: Some(IDLE_DEFAULT_MINUTES),
                failed_attempts: 0,
                last_failure_ms: None,
                pin_length: None,
            }
        );
        assert!(matches!(db.check_pin(PIN, T0), Err(Error::PinNotSet)));
    }

    #[test]
    fn only_the_argon2id_hash_is_stored() {
        let (_dir, db) = open_db();
        db.set_pin(None, PIN, T0).unwrap();
        let stored = db.lock_row().unwrap().pin_hash.unwrap();
        assert!(
            stored.starts_with("$argon2id$v=19$m=65536,t=3,p=4$"),
            "{stored}"
        );
        assert!(!stored.contains(PIN));
        assert!(db.lock_settings().unwrap().pin_set);
    }

    #[test]
    fn a_right_pin_resets_the_count_and_a_wrong_one_is_counted() {
        let (_dir, db) = open_db();
        db.set_pin(None, PIN, T0).unwrap();
        assert!(matches!(db.check_pin(OTHER_PIN, T0), Err(Error::PinWrong)));
        let after_wrong = db.lock_settings().unwrap();
        assert_eq!(
            (after_wrong.failed_attempts, after_wrong.last_failure_ms),
            (1, Some(T0))
        );
        db.check_pin(PIN, T0 + 1).unwrap();
        let after_right = db.lock_settings().unwrap();
        assert_eq!(
            (after_right.failed_attempts, after_right.last_failure_ms),
            (0, None)
        );
    }

    #[test]
    fn after_five_failures_even_the_right_pin_waits_and_the_wait_survives_a_restart() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("r.db");
        let key = || crate::DatabaseKey::from_bytes([3; crate::db::KEY_LEN]);
        {
            let db = Database::open(&path, &key()).unwrap();
            db.set_pin(None, PIN, T0).unwrap();
            for _ in 0..PIN_FREE_ATTEMPTS {
                assert!(matches!(db.check_pin(OTHER_PIN, T0), Err(Error::PinWrong)));
            }
            assert!(matches!(
                db.check_pin(PIN, T0 + 1),
                Err(Error::PinThrottled)
            ));
        }
        // Riaprire l'app non azzera nulla: contatore e attesa stanno nel database.
        let db = Database::open(&path, &key()).unwrap();
        let settings = db.lock_settings().unwrap();
        assert_eq!(settings.failed_attempts, PIN_FREE_ATTEMPTS);
        assert_eq!(settings.retry_after_ms(T0 + 10_000), 20_000);
        assert!(matches!(
            db.check_pin(PIN, T0 + 10_000),
            Err(Error::PinThrottled)
        ));
        // Un tentativo respinto per l'attesa non conta come fallito.
        assert_eq!(
            db.lock_settings().unwrap().failed_attempts,
            PIN_FREE_ATTEMPTS
        );
        db.check_pin(PIN, T0 + PIN_FIRST_WAIT_MS as i64).unwrap();
    }

    #[test]
    fn a_clock_set_back_restarts_the_wait_once_instead_of_locking_out() {
        let (_dir, db) = open_db();
        db.set_pin(None, PIN, T0).unwrap();
        for _ in 0..PIN_FREE_ATTEMPTS {
            let _ = db.check_pin(OTHER_PIN, T0);
        }
        let an_hour_earlier = T0 - 3_600_000;
        assert!(matches!(
            db.check_pin(PIN, an_hour_earlier),
            Err(Error::PinThrottled)
        ));
        assert_eq!(
            db.lock_settings().unwrap().last_failure_ms,
            Some(an_hour_earlier)
        );
        db.check_pin(PIN, an_hour_earlier + PIN_FIRST_WAIT_MS as i64)
            .unwrap();
    }

    /// Il tentativo si registra prima di calcolare l'impronta: anche quando il calcolo
    /// fallisce (qui un'impronta alterata), il contatore è già salito.
    #[test]
    fn the_attempt_is_counted_before_hashing() {
        let (_dir, db) = open_db();
        db.set_pin(None, PIN, T0).unwrap();
        db.conn
            .execute(
                "UPDATE lock_config SET pin_hash = '$argon2id$alterata' WHERE id = 1",
                [],
            )
            .unwrap();
        assert!(matches!(db.check_pin(PIN, T0), Err(Error::PinHashFailed)));
        assert_eq!(db.lock_settings().unwrap().failed_attempts, 1);
    }

    #[test]
    fn changing_the_pin_needs_the_current_one_and_counts_as_an_attempt() {
        let (_dir, db) = open_db();
        db.set_pin(None, PIN, T0).unwrap();

        // Un PIN nuovo non valido si rifiuta prima di toccare il contatore.
        assert!(matches!(
            db.set_pin(Some(PIN), "123456", T0),
            Err(Error::PinTooSimple)
        ));
        assert_eq!(db.lock_settings().unwrap().failed_attempts, 0);

        assert!(matches!(
            db.set_pin(None, OTHER_PIN, T0),
            Err(Error::PinWrong)
        ));
        assert!(matches!(
            db.set_pin(Some("111112"), OTHER_PIN, T0),
            Err(Error::PinWrong)
        ));
        assert_eq!(db.lock_settings().unwrap().failed_attempts, 2);

        db.set_pin(Some(PIN), OTHER_PIN, T0).unwrap();
        assert!(matches!(db.check_pin(PIN, T0), Err(Error::PinWrong)));
        db.check_pin(OTHER_PIN, T0).unwrap();
    }

    /// La lunghezza si conosce dal PIN impostato, si impara al primo sblocco se manca, non
    /// cambia con un PIN sbagliato e sparisce con il blocco. (v0.7.0)
    #[test]
    fn the_pin_length_is_known_learned_and_forgotten() {
        let (_dir, db) = open_db();
        db.set_pin(None, "48291573", T0).unwrap();
        assert_eq!(db.lock_settings().unwrap().pin_length, Some(8));
        db.conn
            .execute("UPDATE lock_config SET pin_length = NULL WHERE id = 1", [])
            .unwrap();
        assert!(matches!(db.check_pin("111111", T0), Err(Error::PinWrong)));
        assert_eq!(db.lock_settings().unwrap().pin_length, None);
        db.check_pin("48291573", T0).unwrap();
        assert_eq!(db.lock_settings().unwrap().pin_length, Some(8));
        db.disable_lock("48291573", T0).unwrap();
        assert_eq!(db.lock_settings().unwrap().pin_length, None);
    }

    #[test]
    fn disabling_needs_the_pin_and_clears_hello_but_keeps_the_idle_time() {
        let (_dir, db) = open_db();
        db.set_pin(None, PIN, T0).unwrap();
        db.set_hello_enabled(true).unwrap();
        db.set_idle_minutes(Some(30)).unwrap();

        assert!(matches!(
            db.disable_lock(OTHER_PIN, T0),
            Err(Error::PinWrong)
        ));
        assert!(db.lock_settings().unwrap().pin_set);

        db.disable_lock(PIN, T0).unwrap();
        let settings = db.lock_settings().unwrap();
        assert!(!settings.pin_set && !settings.hello_enabled);
        assert_eq!(settings.idle_minutes, Some(30));
    }

    #[test]
    fn hello_needs_the_pin_as_a_fallback() {
        let (_dir, db) = open_db();
        assert!(matches!(db.set_hello_enabled(true), Err(Error::PinNotSet)));
        db.set_pin(None, PIN, T0).unwrap();
        db.set_hello_enabled(true).unwrap();
        assert!(db.lock_settings().unwrap().hello_enabled);
        db.set_hello_enabled(false).unwrap();
        assert!(!db.lock_settings().unwrap().hello_enabled);
    }

    #[test]
    fn idle_minutes_stay_within_the_allowed_range() {
        let (_dir, db) = open_db();
        for invalid in [0, IDLE_MAX_MINUTES + 1, u32::MAX] {
            assert!(matches!(
                db.set_idle_minutes(Some(invalid)),
                Err(Error::IdleInvalid)
            ));
        }
        for valid in [
            Some(IDLE_MIN_MINUTES),
            Some(IDLE_DEFAULT_MINUTES),
            Some(IDLE_MAX_MINUTES),
            None,
        ] {
            db.set_idle_minutes(valid).unwrap();
            assert_eq!(db.lock_settings().unwrap().idle_minutes, valid);
        }
    }

    /// I vincoli dello schema sono l'ultima difesa: nemmeno una scrittura diretta può lasciare
    /// Hello senza PIN, un PIN in chiaro o una seconda configurazione.
    #[test]
    fn the_schema_rejects_inconsistent_rows() {
        let (_dir, db) = open_db();
        for sql in [
            "UPDATE lock_config SET hello_enabled = 1 WHERE id = 1",
            "UPDATE lock_config SET pin_hash = '482915' WHERE id = 1",
            "UPDATE lock_config SET idle_minutes = 0 WHERE id = 1",
            "UPDATE lock_config SET failed_attempts = -1 WHERE id = 1",
            "INSERT INTO lock_config (id) VALUES (2)",
        ] {
            assert!(db.conn.execute(sql, []).is_err(), "{sql}");
        }
    }
}
