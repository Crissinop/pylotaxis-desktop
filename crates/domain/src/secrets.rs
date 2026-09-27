//! Segreti delle app: le regole e i dati descrittivi (v0.4.0).
//!
//! Il valore di un segreto non entra mai nel registro: sta nel Credential Manager di Windows,
//! e lo scrive e lo legge solo il guscio (src-tauri/src/vault.rs). Qui si decide che cosa è
//! un segreto valido e si tengono etichetta, nome utente e data di modifica, cioè soltanto
//! ciò che l'interfaccia può mostrare (A.7.10).

use rusqlite::{OptionalExtension, params};
use uuid::Uuid;

use crate::Error;
use crate::db::Database;

/// Limite del Credential Manager per il valore di una credenziale
/// (`CRED_MAX_CREDENTIAL_BLOB_SIZE`). Il valore si salva in UTF-8: il limite è in byte.
pub const SECRET_MAX_BYTES: usize = 2560;
pub const SECRET_LABEL_MAX_CHARS: usize = 60;
pub const SECRET_USERNAME_MAX_CHARS: usize = 200;

/// Dati di un segreto che l'interfaccia può mostrare: nessun carattere del valore, nemmeno
/// gli ultimi, che ridurrebbero la ricerca di chi prova a indovinarlo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretInfo {
    pub id: Uuid,
    pub app_id: Uuid,
    pub label: String,
    pub username: Option<String>,
    pub updated_ms: i64,
}

/// Etichetta e nome utente già validati.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretFields {
    pub label: String,
    pub username: Option<String>,
}

/// Id di un segreto nuovo. Lo chiede il guscio prima di scrivere il valore: la credenziale
/// nasce prima della riga, così una riga non resta mai senza valore (vault.rs).
pub fn new_secret_id() -> Uuid {
    Uuid::now_v7()
}

/// Etichetta obbligatoria (1–60 caratteri); nome utente facoltativo (vuoto = assente).
pub fn validate_secret_fields(label: &str, username: Option<&str>) -> Result<SecretFields, Error> {
    let label = label.trim();
    let length = label.chars().count();
    if length == 0 || length > SECRET_LABEL_MAX_CHARS || label.chars().any(char::is_control) {
        return Err(Error::SecretLabelInvalid);
    }
    let username = match username.map(str::trim) {
        None | Some("") => None,
        Some(name)
            if name.chars().count() > SECRET_USERNAME_MAX_CHARS
                || name.chars().any(char::is_control) =>
        {
            return Err(Error::SecretUsernameInvalid);
        }
        Some(name) => Some(name.to_owned()),
    };
    Ok(SecretFields {
        label: label.to_owned(),
        username,
    })
}

/// Il valore non si accorcia né si ritocca: gli spazi possono farne parte. Si rifiutano solo
/// il valore vuoto e il carattere NUL, che negli appunti di Windows chiude il testo e
/// troncherebbe in silenzio ciò che si incolla.
pub fn validate_secret_value(value: &str) -> Result<(), Error> {
    if value.is_empty() || value.contains('\0') {
        return Err(Error::SecretValueInvalid);
    }
    if value.len() > SECRET_MAX_BYTES {
        return Err(Error::SecretTooLong);
    }
    Ok(())
}

const SECRET_COLUMNS: &str = "id, app_id, label, username, updated_ms";

fn secret_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<SecretInfo> {
    Ok(SecretInfo {
        id: crate::registry::parse_uuid(row, 0)?,
        app_id: crate::registry::parse_uuid(row, 1)?,
        label: row.get(2)?,
        username: row.get(3)?,
        updated_ms: row.get(4)?,
    })
}

impl Database {
    /// Tutti i segreti, per app e per etichetta.
    pub fn secrets(&self) -> Result<Vec<SecretInfo>, Error> {
        Ok(self
            .conn
            .prepare(&format!(
                "SELECT {SECRET_COLUMNS} FROM secrets ORDER BY app_id, label COLLATE NOCASE"
            ))?
            .query_map([], secret_from_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn secret(&self, id: Uuid) -> Result<SecretInfo, Error> {
        self.conn
            .query_row(
                &format!("SELECT {SECRET_COLUMNS} FROM secrets WHERE id = ?1"),
                params![id.to_string()],
                secret_from_row,
            )
            .optional()?
            .ok_or(Error::NotFound)
    }

    /// Id dei segreti di un'app: il guscio li legge prima di eliminarla, per eliminare
    /// anche le credenziali.
    pub fn secret_ids_of_app(&self, app_id: Uuid) -> Result<Vec<Uuid>, Error> {
        Ok(self
            .conn
            .prepare(
                "SELECT id, app_id, label, username, updated_ms FROM secrets WHERE app_id = ?1",
            )?
            .query_map(params![app_id.to_string()], secret_from_row)?
            .map(|row| row.map(|secret| secret.id))
            .collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Controllo esplicito prima del vincolo UNIQUE: un errore che parla di etichette, non
    /// di nomi di app.
    fn label_taken(&self, app_id: Uuid, label: &str, except: Option<Uuid>) -> Result<bool, Error> {
        let except = except.map(|id| id.to_string()).unwrap_or_default();
        Ok(self.conn.query_row(
            "SELECT EXISTS (SELECT 1 FROM secrets
             WHERE app_id = ?1 AND label = ?2 COLLATE NOCASE AND id <> ?3)",
            params![app_id.to_string(), label, except],
            |row| row.get(0),
        )?)
    }

    pub fn create_secret(
        &self,
        id: Uuid,
        app_id: Uuid,
        fields: &SecretFields,
        now_ms: i64,
    ) -> Result<SecretInfo, Error> {
        if self.label_taken(app_id, &fields.label, None)? {
            return Err(Error::SecretLabelDuplicate);
        }
        self.conn.execute(
            "INSERT INTO secrets (id, app_id, label, username, created_ms, updated_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
            params![
                id.to_string(),
                app_id.to_string(),
                fields.label,
                fields.username,
                now_ms
            ],
        )?;
        self.secret(id)
    }

    pub fn update_secret(
        &self,
        id: Uuid,
        fields: &SecretFields,
        now_ms: i64,
    ) -> Result<SecretInfo, Error> {
        let current = self.secret(id)?;
        if self.label_taken(current.app_id, &fields.label, Some(id))? {
            return Err(Error::SecretLabelDuplicate);
        }
        self.conn.execute(
            "UPDATE secrets SET label = ?2, username = ?3, updated_ms = ?4 WHERE id = ?1",
            params![id.to_string(), fields.label, fields.username, now_ms],
        )?;
        self.secret(id)
    }

    /// Registra la sostituzione del valore, che il guscio ha già scritto nel Credential Manager.
    pub fn touch_secret(&self, id: Uuid, now_ms: i64) -> Result<SecretInfo, Error> {
        let updated = self.conn.execute(
            "UPDATE secrets SET updated_ms = ?2 WHERE id = ?1",
            params![id.to_string(), now_ms],
        )?;
        if updated == 0 {
            return Err(Error::NotFound);
        }
        self.secret(id)
    }

    pub fn delete_secret(&self, id: Uuid) -> Result<(), Error> {
        let deleted = self
            .conn
            .execute("DELETE FROM secrets WHERE id = ?1", params![id.to_string()])?;
        if deleted == 0 {
            return Err(Error::NotFound);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::tests::{open_db, web};

    const NOW: i64 = 1_790_000_000_000;

    fn fields(label: &str, username: Option<&str>) -> SecretFields {
        validate_secret_fields(label, username).unwrap()
    }

    #[test]
    fn labels_and_usernames_are_trimmed_and_bounded() {
        assert_eq!(
            fields("  Password ", Some("  mario@example.com ")),
            SecretFields {
                label: "Password".into(),
                username: Some("mario@example.com".into())
            }
        );
        assert_eq!(fields("Token", Some("   ")).username, None);
        assert_eq!(fields("Token", None).username, None);
        let long_label = "x".repeat(SECRET_LABEL_MAX_CHARS + 1);
        for label in ["", "   ", "riga\nnuova", long_label.as_str()] {
            assert!(
                matches!(
                    validate_secret_fields(label, None),
                    Err(Error::SecretLabelInvalid)
                ),
                "{label:?}"
            );
        }
        let long_user = "u".repeat(SECRET_USERNAME_MAX_CHARS + 1);
        for user in ["tab\tdentro", long_user.as_str()] {
            assert!(matches!(
                validate_secret_fields("Password", Some(user)),
                Err(Error::SecretUsernameInvalid)
            ));
        }
    }

    #[test]
    fn values_keep_their_spaces_and_respect_the_credential_manager_limit() {
        for valid in [
            " spazi ai lati ",
            "à".repeat(SECRET_MAX_BYTES / 2).as_str(),
            "riga\nriga",
        ] {
            assert!(validate_secret_value(valid).is_ok(), "{valid:?}");
        }
        // Il limite è in byte: 1281 "à" sono 2562 byte, oltre il limite.
        let too_long = "à".repeat(SECRET_MAX_BYTES / 2 + 1);
        assert!(matches!(
            validate_secret_value(&too_long),
            Err(Error::SecretTooLong)
        ));
        assert!(matches!(
            validate_secret_value(&"a".repeat(SECRET_MAX_BYTES + 1)),
            Err(Error::SecretTooLong)
        ));
        for invalid in ["", "prima\0dopo"] {
            assert!(matches!(
                validate_secret_value(invalid),
                Err(Error::SecretValueInvalid)
            ));
        }
    }

    #[test]
    fn secrets_are_created_listed_updated_and_deleted() {
        let (_dir, db) = open_db();
        let app = db.create_app(&web("Portale")).unwrap();
        let id = new_secret_id();
        let created = db
            .create_secret(id, app.id, &fields("Password", Some("mario")), NOW)
            .unwrap();
        assert_eq!((created.app_id, created.updated_ms), (app.id, NOW));
        assert_eq!(db.secrets().unwrap(), std::slice::from_ref(&created));

        let updated = db
            .update_secret(id, &fields("Password di rete", None), NOW + 1)
            .unwrap();
        assert_eq!(
            (updated.label.as_str(), updated.username),
            ("Password di rete", None)
        );
        assert_eq!(db.touch_secret(id, NOW + 2).unwrap().updated_ms, NOW + 2);

        db.delete_secret(id).unwrap();
        assert!(db.secrets().unwrap().is_empty());
        assert!(matches!(db.delete_secret(id), Err(Error::NotFound)));
        assert!(matches!(db.touch_secret(id, NOW), Err(Error::NotFound)));
    }

    #[test]
    fn labels_are_unique_within_an_app_regardless_of_case() {
        let (_dir, db) = open_db();
        let first = db.create_app(&web("Primo")).unwrap();
        let second = db.create_app(&web("Secondo")).unwrap();
        let token = db
            .create_secret(new_secret_id(), first.id, &fields("Token", None), NOW)
            .unwrap();
        assert!(matches!(
            db.create_secret(new_secret_id(), first.id, &fields("TOKEN", None), NOW),
            Err(Error::SecretLabelDuplicate)
        ));
        // Stessa etichetta in un'altra app: ammessa. Rinominare sé stessi: ammesso.
        db.create_secret(new_secret_id(), second.id, &fields("Token", None), NOW)
            .unwrap();
        db.update_secret(token.id, &fields("token", None), NOW)
            .unwrap();
        let other = db
            .create_secret(new_secret_id(), first.id, &fields("Altro", None), NOW)
            .unwrap();
        assert!(matches!(
            db.update_secret(other.id, &fields("Token", None), NOW),
            Err(Error::SecretLabelDuplicate)
        ));
    }

    #[test]
    fn a_secret_needs_an_existing_app_and_goes_away_with_it() {
        let (_dir, db) = open_db();
        assert!(matches!(
            db.create_secret(new_secret_id(), Uuid::now_v7(), &fields("Token", None), NOW),
            Err(Error::NotFound)
        ));
        let app = db.create_app(&web("Portale")).unwrap();
        let id = new_secret_id();
        db.create_secret(id, app.id, &fields("Token", None), NOW)
            .unwrap();
        assert_eq!(db.secret_ids_of_app(app.id).unwrap(), [id]);
        db.delete_app(app.id).unwrap();
        assert!(db.secrets().unwrap().is_empty());
    }

    /// Il registro non ha una colonna per il valore: lo schema stesso lo tiene fuori.
    #[test]
    fn the_schema_has_no_place_for_the_value() {
        let (_dir, db) = open_db();
        let columns: Vec<String> = db
            .conn
            .prepare("SELECT name FROM pragma_table_info('secrets')")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(
            columns,
            [
                "id",
                "app_id",
                "label",
                "username",
                "created_ms",
                "updated_ms"
            ]
        );
    }
}
