//! Registro delle app: modello, validazione e persistenza (v0.2.0).
//!
//! Ogni dato che entra passa da qui: nome, bersaglio (eseguibile, indirizzo web o
//! protocollo), categoria e tag. Il frontend non valida nulla per conto suo (A.7.3).

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use rusqlite::types::Type;
use rusqlite::{OptionalExtension, Row, params};
use sha2::{Digest, Sha256};
use url::Url;
use uuid::Uuid;

use crate::Error;
use crate::db::Database;
use crate::groups::Group;

/// Limiti degli input: abbastanza larghi per l'uso reale, abbastanza stretti da non
/// rompere l'impaginazione né accettare incollati accidentali. (v0.2.0)
pub const NAME_MAX_CHARS: usize = 80;
pub const TAG_MAX_CHARS: usize = 32;
pub const TAGS_MAX: usize = 12;

/// Byte letti per volta durante il calcolo dell'impronta.
const HASH_BUFFER_BYTES: usize = 64 * 1024;

/// Schemi mai ammessi come protocollo. `http`/`https` hanno il loro tipo; gli altri aprono
/// file o eseguono codice (`file`, `javascript`, `vbscript`, `data`) oppure sono stati usati
/// in attacchi noti (`ms-msdt`, `search-ms`, `ms-officecmd`, `ms-appinstaller`). (v0.2.0)
const BLOCKED_SCHEMES: &[&str] = &[
    "about",
    "blob",
    "data",
    "file",
    "http",
    "https",
    "javascript",
    "ms-appinstaller",
    "ms-msdt",
    "ms-officecmd",
    "search",
    "search-ms",
    "vbscript",
];

/// Cosa si apre quando si avvia un'app.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// Programma locale, con l'impronta SHA-256 confermata dall'utente.
    Executable { path: PathBuf, sha256: String },
    /// Indirizzo `http` o `https`, aperto nel browser di sistema.
    Web { url: String },
    /// URI di un protocollo registrato, come `vscode://` o `ms-settings:`.
    Protocol { uri: String },
}

impl Target {
    fn kind(&self) -> &'static str {
        match self {
            Self::Executable { .. } => "executable",
            Self::Web { .. } => "web",
            Self::Protocol { .. } => "protocol",
        }
    }
}

/// Ambiente di un'app (v0.6.0). Un insieme chiuso, così la produzione ha un segnale che non
/// si confonde con altri (A.8); `None` per le app che non ne hanno uno.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Environment {
    Development,
    Test,
    Production,
}

impl Environment {
    /// Codice stabile, lo stesso nel database e verso il frontend.
    pub fn code(self) -> &'static str {
        match self {
            Self::Development => "development",
            Self::Test => "test",
            Self::Production => "production",
        }
    }

    pub fn from_code(code: &str) -> Result<Self, Error> {
        match code {
            "development" => Ok(Self::Development),
            "test" => Ok(Self::Test),
            "production" => Ok(Self::Production),
            _ => Err(Error::EnvironmentInvalid),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct App {
    pub id: Uuid,
    pub name: String,
    pub target: Target,
    pub category_id: Option<Uuid>,
    pub tags: Vec<String>,
    /// Ambiente e controllo dello stato (v0.6.0).
    pub environment: Option<Environment>,
    pub health_check: bool,
    /// Revisione dell'icona salvata, `None` = iniziali (v0.7.0). L'immagine si legge a parte.
    pub icon_rev: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Category {
    pub id: Uuid,
    pub name: String,
}

/// Fotografia completa del registro, già ordinata per la presentazione.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registry {
    pub categories: Vec<Category>,
    pub apps: Vec<App>,
    /// Gruppi di avvio (v0.6.0).
    pub groups: Vec<Group>,
}

/// Dati di un'app da creare o aggiornare.
#[derive(Debug, Clone)]
pub struct AppInput {
    pub name: String,
    pub target: Target,
    pub category_id: Option<Uuid>,
    pub tags: Vec<String>,
    pub environment: Option<Environment>,
    pub health_check: bool,
}

/// Eseguibile scelto dall'utente e già ispezionato.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutableInfo {
    pub path: PathBuf,
    pub sha256: String,
    pub suggested_name: String,
}

pub fn validate_name(raw: &str) -> Result<String, Error> {
    let name = raw.trim();
    let length = name.chars().count();
    if length == 0 || length > NAME_MAX_CHARS || name.chars().any(char::is_control) {
        return Err(Error::NameInvalid);
    }
    Ok(name.to_owned())
}

/// Accetta solo `http` e `https` con un host, senza credenziali nell'indirizzo:
/// le credenziali stanno nel Credential Manager come segreti dell'app (v0.4.0), non in chiaro
/// nell'indirizzo né a schermo.
pub fn validate_web_url(raw: &str) -> Result<String, Error> {
    let url = Url::parse(raw.trim()).map_err(|_| Error::UrlInvalid)?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(Error::SchemeNotAllowed);
    }
    if url.host_str().is_none_or(str::is_empty)
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(Error::UrlInvalid);
    }
    Ok(url.into())
}

/// Accetta un URI di protocollo, esclusi gli schemi bloccati e le lettere di unità:
/// `C:\app.exe` si legge come schema `c`, e aprirlo equivarrebbe ad aprire un file.
pub fn validate_protocol_uri(raw: &str) -> Result<String, Error> {
    let url = Url::parse(raw.trim()).map_err(|_| Error::UrlInvalid)?;
    let scheme = url.scheme();
    if scheme.len() == 1 || BLOCKED_SCHEMES.contains(&scheme) {
        return Err(Error::SchemeNotAllowed);
    }
    Ok(url.into())
}

/// Tag puliti: spazi tolti, vuoti ignorati, duplicati rimossi senza badare alle
/// maiuscole, ordine alfabetico.
pub fn validate_tags(raw: &[String]) -> Result<Vec<String>, Error> {
    let mut tags: Vec<String> = Vec::new();
    for tag in raw.iter().map(|t| t.trim()).filter(|t| !t.is_empty()) {
        if tag.chars().count() > TAG_MAX_CHARS || tag.chars().any(|c| c.is_control() || c == ',') {
            return Err(Error::TagInvalid);
        }
        if !tags.iter().any(|t| t.to_lowercase() == tag.to_lowercase()) {
            tags.push(tag.to_owned());
        }
    }
    if tags.len() > TAGS_MAX {
        return Err(Error::TagInvalid);
    }
    tags.sort_by_key(|t| t.to_lowercase());
    Ok(tags)
}

/// Controlla che il file scelto sia un `.exe` esistente e ne calcola l'impronta.
pub fn inspect_executable(path: &Path) -> Result<ExecutableInfo, Error> {
    let is_exe = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("exe"));
    // Il percorso si salva come testo: un percorso non UTF-8 non si potrebbe rileggere uguale.
    if !path.is_absolute() || !is_exe || path.to_str().is_none() || !path.is_file() {
        return Err(Error::ExecutableInvalid);
    }
    let suggested_name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_owned();
    Ok(ExecutableInfo {
        path: path.to_path_buf(),
        sha256: sha256_file(path)?,
        suggested_name,
    })
}

/// Impronta SHA-256 del file, in esadecimale minuscolo.
pub fn sha256_file(path: &Path) -> Result<String, Error> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; HASH_BUFFER_BYTES];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(crate::hex::lower(&hasher.finalize()))
}

/// Rivalida un bersaglio prima di scriverlo: anche un dato costruito altrove passa di qui.
fn validate_target(target: &Target) -> Result<Target, Error> {
    match target {
        Target::Executable { path, sha256 } => {
            let valid_hash = sha256.len() == 64
                && sha256
                    .chars()
                    .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c));
            let is_exe = path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("exe"));
            if !valid_hash || !is_exe || !path.is_absolute() || path.to_str().is_none() {
                return Err(Error::ExecutableInvalid);
            }
            Ok(target.clone())
        }
        Target::Web { url } => Ok(Target::Web {
            url: validate_web_url(url)?,
        }),
        Target::Protocol { uri } => Ok(Target::Protocol {
            uri: validate_protocol_uri(uri)?,
        }),
    }
}

fn target_text(target: &Target) -> Result<&str, Error> {
    match target {
        Target::Executable { path, .. } => path.to_str().ok_or(Error::ExecutableInvalid),
        Target::Web { url } => Ok(url),
        Target::Protocol { uri } => Ok(uri),
    }
}

fn target_sha256(target: &Target) -> Option<&str> {
    match target {
        Target::Executable { sha256, .. } => Some(sha256),
        _ => None,
    }
}

pub(crate) fn parse_uuid(row: &Row<'_>, index: usize) -> rusqlite::Result<Uuid> {
    let text: String = row.get(index)?;
    Uuid::parse_str(&text)
        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(index, Type::Text, Box::new(e)))
}

fn parse_optional_uuid(row: &Row<'_>, index: usize) -> rusqlite::Result<Option<Uuid>> {
    let text: Option<String> = row.get(index)?;
    text.map(|t| {
        Uuid::parse_str(&t)
            .map_err(|e| rusqlite::Error::FromSqlConversionFailure(index, Type::Text, Box::new(e)))
    })
    .transpose()
}

const APP_COLUMNS: &str =
    "id, name, kind, target, sha256, category_id, environment, health_check, icon_rev";

/// Stessa app nei diversi ambienti una accanto all'altra: senza ambiente, sviluppo, collaudo,
/// produzione (v0.6.0).
const APP_ORDER: &str = "name COLLATE NOCASE, CASE environment WHEN 'development' THEN 1 \
     WHEN 'test' THEN 2 WHEN 'production' THEN 3 ELSE 0 END";

/// Legge una riga di `apps`; i tag si aggiungono dopo.
fn app_from_row(row: &Row<'_>) -> rusqlite::Result<App> {
    let kind: String = row.get(2)?;
    let target_value: String = row.get(3)?;
    let sha256: Option<String> = row.get(4)?;
    let target = match (kind.as_str(), sha256) {
        ("executable", Some(sha256)) => Target::Executable {
            path: PathBuf::from(target_value),
            sha256,
        },
        ("web", None) => Target::Web { url: target_value },
        ("protocol", None) => Target::Protocol { uri: target_value },
        _ => {
            return Err(rusqlite::Error::FromSqlConversionFailure(
                2,
                Type::Text,
                format!("tipo di app sconosciuto: {kind}").into(),
            ));
        }
    };
    Ok(App {
        id: parse_uuid(row, 0)?,
        name: row.get(1)?,
        target,
        category_id: parse_optional_uuid(row, 5)?,
        tags: Vec::new(),
        environment: row
            .get::<_, Option<String>>(6)?
            .map(|code| {
                Environment::from_code(&code).map_err(|_| {
                    rusqlite::Error::FromSqlConversionFailure(
                        6,
                        Type::Text,
                        format!("ambiente sconosciuto: {code}").into(),
                    )
                })
            })
            .transpose()?,
        health_check: row.get::<_, i64>(7)? != 0,
        icon_rev: row.get(8)?,
    })
}

impl Database {
    /// Tutto il registro in una lettura: categorie per posizione, app per nome.
    pub fn registry(&self) -> Result<Registry, Error> {
        let categories = self
            .conn
            .prepare("SELECT id, name FROM categories ORDER BY position, name COLLATE NOCASE")?
            .query_map([], |row| {
                Ok(Category {
                    id: parse_uuid(row, 0)?,
                    name: row.get(1)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let mut tags: HashMap<Uuid, Vec<String>> = HashMap::new();
        for pair in self
            .conn
            .prepare("SELECT app_id, tag FROM app_tags ORDER BY tag COLLATE NOCASE")?
            .query_map([], |row| {
                Ok((parse_uuid(row, 0)?, row.get::<_, String>(1)?))
            })?
        {
            let (app_id, tag) = pair?;
            tags.entry(app_id).or_default().push(tag);
        }

        let mut apps = self
            .conn
            .prepare(&format!(
                "SELECT {APP_COLUMNS} FROM apps ORDER BY {APP_ORDER}"
            ))?
            .query_map([], app_from_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        for app in &mut apps {
            app.tags = tags.remove(&app.id).unwrap_or_default();
        }
        let groups = self.groups()?;
        Ok(Registry {
            categories,
            apps,
            groups,
        })
    }

    pub fn app(&self, id: Uuid) -> Result<App, Error> {
        let mut app = self
            .conn
            .query_row(
                &format!("SELECT {APP_COLUMNS} FROM apps WHERE id = ?1"),
                params![id.to_string()],
                app_from_row,
            )
            .optional()?
            .ok_or(Error::NotFound)?;
        app.tags = self
            .conn
            .prepare("SELECT tag FROM app_tags WHERE app_id = ?1 ORDER BY tag COLLATE NOCASE")?
            .query_map(params![id.to_string()], |row| row.get(0))?
            .collect::<rusqlite::Result<Vec<String>>>()?;
        Ok(app)
    }

    pub fn create_app(&self, input: &AppInput) -> Result<App, Error> {
        let id = Uuid::now_v7();
        self.write_app(id, input, true)?;
        self.app(id)
    }

    pub fn update_app(&self, id: Uuid, input: &AppInput) -> Result<App, Error> {
        self.write_app(id, input, false)?;
        self.app(id)
    }

    /// Scrive app e tag in una sola transazione: o tutto, o niente (A.7.3).
    fn write_app(&self, id: Uuid, input: &AppInput, is_new: bool) -> Result<(), Error> {
        let name = validate_name(&input.name)?;
        let target = validate_target(&input.target)?;
        let tags = validate_tags(&input.tags)?;
        let category = input.category_id.map(|c| c.to_string());
        // Il controllo dello stato è una richiesta HTTP: ha senso solo per le app web. (v0.6.0)
        if input.health_check && !matches!(target, Target::Web { .. }) {
            return Err(Error::HealthCheckWebOnly);
        }
        let environment = input.environment.map(Environment::code);

        let tx = self.conn.unchecked_transaction()?;
        let changed = if is_new {
            tx.execute(
                "INSERT INTO apps (id, name, kind, target, sha256, category_id, environment,
                                   health_check)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    id.to_string(),
                    name,
                    target.kind(),
                    target_text(&target)?,
                    target_sha256(&target),
                    category,
                    environment,
                    input.health_check
                ],
            )?
        } else {
            tx.execute(
                "UPDATE apps SET name = ?2, kind = ?3, target = ?4, sha256 = ?5, category_id = ?6,
                                 environment = ?7, health_check = ?8
                 WHERE id = ?1",
                params![
                    id.to_string(),
                    name,
                    target.kind(),
                    target_text(&target)?,
                    target_sha256(&target),
                    category,
                    environment,
                    input.health_check
                ],
            )?
        };
        if changed == 0 {
            return Err(Error::NotFound);
        }
        // Un'app che non è più un eseguibile perde l'icona estratta; una scelta resta. (v0.7.0)
        tx.execute(
            "UPDATE apps SET icon = NULL, icon_kind = NULL, icon_rev = NULL
             WHERE id = ?1 AND kind <> 'executable' AND icon_kind = 'executable'",
            params![id.to_string()],
        )?;
        tx.execute(
            "DELETE FROM app_tags WHERE app_id = ?1",
            params![id.to_string()],
        )?;
        for tag in &tags {
            tx.execute(
                "INSERT INTO app_tags (app_id, tag) VALUES (?1, ?2)",
                params![id.to_string(), tag],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn delete_app(&self, id: Uuid) -> Result<(), Error> {
        let deleted = self
            .conn
            .execute("DELETE FROM apps WHERE id = ?1", params![id.to_string()])?;
        if deleted == 0 {
            return Err(Error::NotFound);
        }
        Ok(())
    }

    /// Registra la nuova impronta di un eseguibile dopo la conferma esplicita dell'utente.
    pub fn repin_executable(&self, id: Uuid, sha256: &str) -> Result<(), Error> {
        let updated = self.conn.execute(
            "UPDATE apps SET sha256 = ?2 WHERE id = ?1 AND kind = 'executable'",
            params![id.to_string(), sha256],
        )?;
        if updated == 0 {
            return Err(Error::NotFound);
        }
        Ok(())
    }

    pub fn create_category(&self, name: &str) -> Result<Category, Error> {
        let name = validate_name(name)?;
        let id = Uuid::now_v7();
        self.conn.execute(
            "INSERT INTO categories (id, name, position)
             VALUES (?1, ?2, (SELECT coalesce(max(position), -1) + 1 FROM categories))",
            params![id.to_string(), name],
        )?;
        Ok(Category { id, name })
    }

    pub fn rename_category(&self, id: Uuid, name: &str) -> Result<Category, Error> {
        let name = validate_name(name)?;
        let updated = self.conn.execute(
            "UPDATE categories SET name = ?2 WHERE id = ?1",
            params![id.to_string(), name],
        )?;
        if updated == 0 {
            return Err(Error::NotFound);
        }
        Ok(Category { id, name })
    }

    /// Elimina la categoria; le sue app restano, senza categoria (ON DELETE SET NULL).
    pub fn delete_category(&self, id: Uuid) -> Result<(), Error> {
        let deleted = self.conn.execute(
            "DELETE FROM categories WHERE id = ?1",
            params![id.to_string()],
        )?;
        if deleted == 0 {
            return Err(Error::NotFound);
        }
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::db::{DatabaseKey, KEY_LEN};

    /// SHA-256 di "abc", dal vettore di prova di FIPS 180-2.
    pub(crate) const ABC_SHA256: &str =
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

    pub(crate) fn open_db() -> (tempfile::TempDir, Database) {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::open(
            &dir.path().join("r.db"),
            &DatabaseKey::from_bytes([3; KEY_LEN]),
        )
        .unwrap();
        (dir, db)
    }

    pub(crate) fn write_exe(dir: &Path, name: &str, content: &[u8]) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, content).unwrap();
        path
    }

    pub(crate) fn web(name: &str) -> AppInput {
        AppInput {
            name: name.to_owned(),
            target: Target::Web {
                url: "https://example.com/".to_owned(),
            },
            category_id: None,
            tags: Vec::new(),
            environment: None,
            health_check: false,
        }
    }

    #[test]
    fn names_are_trimmed_and_bounded() {
        assert_eq!(validate_name("  Margin  ").unwrap(), "Margin");
        assert!(matches!(validate_name("   "), Err(Error::NameInvalid)));
        assert!(matches!(
            validate_name(&"x".repeat(NAME_MAX_CHARS + 1)),
            Err(Error::NameInvalid)
        ));
        assert!(matches!(
            validate_name("a\u{0007}b"),
            Err(Error::NameInvalid)
        ));
        assert!(
            validate_name(&"è".repeat(NAME_MAX_CHARS)).is_ok(),
            "limite in caratteri, non in byte"
        );
    }

    #[test]
    fn web_urls_accept_only_http_and_https_without_credentials() {
        assert_eq!(
            validate_web_url(" https://example.com ").unwrap(),
            "https://example.com/"
        );
        assert!(validate_web_url("http://localhost:8080/app").is_ok());
        for bad in [
            "ftp://example.com",
            "javascript:alert(1)",
            "file:///C:/x.txt",
            "data:text/html,x",
        ] {
            assert!(
                matches!(validate_web_url(bad), Err(Error::SchemeNotAllowed)),
                "{bad}"
            );
        }
        for bad in ["https://utente:segreto@example.com", "non un indirizzo", ""] {
            assert!(
                matches!(validate_web_url(bad), Err(Error::UrlInvalid)),
                "{bad}"
            );
        }
    }

    #[test]
    fn protocols_reject_dangerous_schemes_and_drive_letters() {
        assert!(validate_protocol_uri("vscode://file/C:/progetti").is_ok());
        assert!(validate_protocol_uri("ms-settings:display").is_ok());
        for bad in [
            "file:///C:/Windows/System32/cmd.exe",
            "javascript:alert(1)",
            "ms-msdt:/id PCWDiagnostic",
            "search-ms:query=x",
            "https://example.com",
            "C:\\Windows\\System32\\cmd.exe",
        ] {
            assert!(
                matches!(validate_protocol_uri(bad), Err(Error::SchemeNotAllowed)),
                "{bad}"
            );
        }
    }

    #[test]
    fn tags_are_cleaned_deduplicated_and_bounded() {
        let raw = [" lavoro ", "Rust", "", "rust", "Casa"].map(String::from);
        assert_eq!(validate_tags(&raw).unwrap(), ["Casa", "lavoro", "Rust"]);
        assert!(matches!(
            validate_tags(&["x".repeat(TAG_MAX_CHARS + 1)]),
            Err(Error::TagInvalid)
        ));
        assert!(matches!(
            validate_tags(&["a,b".to_owned()]),
            Err(Error::TagInvalid)
        ));
        let many: Vec<String> = (0..=TAGS_MAX).map(|i| format!("t{i}")).collect();
        assert!(matches!(validate_tags(&many), Err(Error::TagInvalid)));
    }

    #[test]
    fn inspects_only_existing_absolute_exe_files() {
        let dir = tempfile::tempdir().unwrap();
        let exe = write_exe(dir.path(), "Strumento.EXE", b"abc");
        let info = inspect_executable(&exe).unwrap();
        assert_eq!(info.sha256, ABC_SHA256);
        assert_eq!(info.suggested_name, "Strumento");

        let not_exe = write_exe(dir.path(), "note.txt", b"abc");
        std::fs::create_dir(dir.path().join("cartella.exe")).unwrap();
        for bad in [
            not_exe,
            dir.path().join("cartella.exe"),
            dir.path().join("mancante.exe"),
            PathBuf::from("relativo.exe"),
        ] {
            assert!(
                matches!(inspect_executable(&bad), Err(Error::ExecutableInvalid)),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn creates_lists_updates_and_deletes_apps() {
        let (_dir, db) = open_db();
        let category = db.create_category("Lavoro").unwrap();
        let mut input = web("Margin web");
        input.category_id = Some(category.id);
        input.tags = vec!["Rust".into(), "rust".into(), "tauri".into()];
        let app = db.create_app(&input).unwrap();
        assert_eq!(app.tags, ["Rust", "tauri"]);

        let registry = db.registry().unwrap();
        assert_eq!(registry.categories, std::slice::from_ref(&category));
        assert_eq!(registry.apps, std::slice::from_ref(&app));

        input.name = "Margin".into();
        input.tags = vec!["nuovo".into()];
        let updated = db.update_app(app.id, &input).unwrap();
        assert_eq!(
            (updated.name.as_str(), updated.tags.as_slice()),
            ("Margin", ["nuovo".to_owned()].as_slice())
        );

        db.delete_app(app.id).unwrap();
        assert!(db.registry().unwrap().apps.is_empty());
        assert!(matches!(db.delete_app(app.id), Err(Error::NotFound)));
        assert!(matches!(
            db.update_app(app.id, &input),
            Err(Error::NotFound)
        ));
    }

    /// Lo stesso nome una volta per ambiente; "senza ambiente" vale come un ambiente a sé.
    /// Le varianti di una stessa app stanno una accanto all'altra. (v0.6.0)
    #[test]
    fn a_name_is_unique_per_environment() {
        use Environment::{Development, Production, Test};
        let (_dir, db) = open_db();
        let with = |name: &str, environment| AppInput {
            environment,
            ..web(name)
        };
        for environment in [Some(Production), None, Some(Test), Some(Development)] {
            db.create_app(&with("Portale", environment)).unwrap();
        }
        for environment in [None, Some(Development), Some(Production)] {
            assert!(
                matches!(
                    db.create_app(&with("PORTALE", environment)),
                    Err(Error::NameDuplicate)
                ),
                "{environment:?}"
            );
        }
        let order: Vec<_> = db
            .registry()
            .unwrap()
            .apps
            .iter()
            .map(|a| a.environment)
            .collect();
        assert_eq!(
            order,
            [None, Some(Development), Some(Test), Some(Production)]
        );
        assert!(matches!(
            Environment::from_code("prod"),
            Err(Error::EnvironmentInvalid)
        ));
    }

    /// Il controllo dello stato è una richiesta HTTP: solo per le app web, e si rilegge com'è
    /// stato scritto. (v0.6.0)
    #[test]
    fn the_health_check_is_only_for_web_apps() {
        let (_dir, db) = open_db();
        let app = db
            .create_app(&AppInput {
                health_check: true,
                environment: Some(Environment::Test),
                ..web("Stato")
            })
            .unwrap();
        assert!(app.health_check);
        assert_eq!(app.environment, Some(Environment::Test));
        let protocol = AppInput {
            target: Target::Protocol {
                uri: "vscode://file".into(),
            },
            health_check: true,
            ..web("Protocollo")
        };
        assert!(matches!(
            db.create_app(&protocol),
            Err(Error::HealthCheckWebOnly)
        ));
    }

    #[test]
    fn names_are_unique_regardless_of_case() {
        let (_dir, db) = open_db();
        db.create_app(&web("Margin")).unwrap();
        assert!(matches!(
            db.create_app(&web("MARGIN")),
            Err(Error::NameDuplicate)
        ));
        db.create_category("Lavoro").unwrap();
        assert!(matches!(
            db.create_category(" lavoro "),
            Err(Error::NameDuplicate)
        ));
    }

    #[test]
    fn a_missing_category_is_rejected_and_deleting_one_keeps_its_apps() {
        let (_dir, db) = open_db();
        let mut input = web("App");
        input.category_id = Some(Uuid::now_v7());
        assert!(matches!(db.create_app(&input), Err(Error::NotFound)));

        let category = db.create_category("Da togliere").unwrap();
        input.category_id = Some(category.id);
        let app = db.create_app(&input).unwrap();
        db.delete_category(category.id).unwrap();
        assert_eq!(db.app(app.id).unwrap().category_id, None);
    }

    #[test]
    fn executables_keep_path_and_hash_and_can_be_repinned() {
        let (dir, db) = open_db();
        let exe = write_exe(dir.path(), "tool.exe", b"abc");
        let info = inspect_executable(&exe).unwrap();
        let app = db
            .create_app(&AppInput {
                name: info.suggested_name.clone(),
                target: Target::Executable {
                    path: info.path.clone(),
                    sha256: info.sha256.clone(),
                },
                category_id: None,
                tags: Vec::new(),
                environment: None,
                health_check: false,
            })
            .unwrap();
        assert_eq!(
            app.target,
            Target::Executable {
                path: exe,
                sha256: ABC_SHA256.to_owned()
            }
        );
        let other = "0".repeat(64);
        db.repin_executable(app.id, &other).unwrap();
        assert!(
            matches!(db.app(app.id).unwrap().target, Target::Executable { sha256, .. } if sha256 == other)
        );
    }

    #[test]
    fn invalid_targets_never_reach_the_database() {
        let (_dir, db) = open_db();
        let bad_targets = [
            Target::Web {
                url: "javascript:alert(1)".into(),
            },
            Target::Protocol {
                uri: "file:///C:/x.exe".into(),
            },
            Target::Executable {
                path: PathBuf::from("relativo.exe"),
                sha256: ABC_SHA256.into(),
            },
            Target::Executable {
                path: std::env::temp_dir().join("x.exe"),
                sha256: "non-esadecimale".into(),
            },
        ];
        for target in bad_targets {
            let input = AppInput {
                target,
                ..web("Prova")
            };
            assert!(db.create_app(&input).is_err());
        }
        assert!(db.registry().unwrap().apps.is_empty());
    }
}
