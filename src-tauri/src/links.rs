//! Link diretti `<schema>://app/<id>` (v0.5.0).
//!
//! Qualunque pagina web può aprire un link, quindi un link non avvia mai nulla: apre la
//! palette con l'app preselezionata, e l'avvio chiede Invio. Si accetta solo la forma esatta;
//! id malformati o sconosciuti si ignorano, e da bloccata il link si scarta, senza restare in
//! attesa dello sblocco. Lo schema si legge dalla configurazione del plugin
//! (tauri.conf.json), la stessa che l'installer registra: nel codice non c'è il nome del
//! prodotto (A.2, `check:name`).

use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_deep_link::DeepLinkExt;
use uuid::Uuid;

use crate::palette;
use crate::state::AppState;

/// Primo schema della configurazione desktop del plugin (`plugins.deep-link`).
pub fn configured_scheme(deep_link: &serde_json::Value) -> Option<&str> {
    deep_link["desktop"]["schemes"][0]
        .as_str()
        .filter(|scheme| !scheme.is_empty())
}

fn scheme<R: Runtime>(app: &AppHandle<R>) -> Option<String> {
    app.config()
        .plugins
        .0
        .get("deep-link")
        .and_then(configured_scheme)
        .map(str::to_owned)
}

/// L'id di `<schema>://app/<id>`, con l'id nella forma canonica (minuscole e trattini) che
/// l'app stessa produce. Nient'altro: niente barra finale, query, frammento, credenziali,
/// porta, maiuscole o altre forme dell'id.
pub fn parse_app_link(link: &str, scheme: &str) -> Option<Uuid> {
    if scheme.is_empty() {
        return None;
    }
    let rest = link.strip_prefix(scheme)?.strip_prefix("://app/")?;
    let id = Uuid::try_parse(rest).ok()?;
    (id.hyphenated().to_string() == rest).then_some(id)
}

/// L'app da preselezionare, oppure nessuna: da bloccata, senza link validi, o con un id che il
/// registro non conosce. Con più link vale l'ultimo valido.
pub fn link_target<S: AsRef<str>>(state: &AppState, scheme: &str, links: &[S]) -> Option<Uuid> {
    if state.lock.is_locked() {
        return None;
    }
    let id = links
        .iter()
        .rev()
        .find_map(|link| parse_app_link(link.as_ref(), scheme))?;
    state.with_db(|db| Ok(db.app(id)?)).ok().map(|app| app.id)
}

/// Vero se gli argomenti di una seconda istanza portano un link: in quel caso la finestra
/// principale non si porta davanti, perché toglierebbe il fuoco alla palette e la chiuderebbe.
pub fn carries_link<R: Runtime>(app: &AppHandle<R>, args: &[String]) -> bool {
    scheme(app).is_some_and(|scheme| {
        let prefix = format!("{scheme}:");
        args.iter().any(|arg| arg.starts_with(&prefix))
    })
}

fn open_links<R: Runtime, S: AsRef<str>>(app: &AppHandle<R>, links: &[S]) {
    let (Some(scheme), Some(state)) = (scheme(app), app.try_state::<AppState>()) else {
        return;
    };
    if let Some(id) = link_target(&state, &scheme, links) {
        palette::open(app, Some(id));
    }
}

/// Ascolta i link: quelli di una seconda istanza (inoltrati dall'istanza singola) e quello
/// con cui è partita questa.
pub fn listen<R: Runtime>(app: &AppHandle<R>) {
    // Nelle build installate lo schema lo registra l'installer NSIS dalla configurazione;
    // qui solo in sviluppo, così `tauri dev` non sovrascrive l'associazione dell'app installata.
    #[cfg(debug_assertions)]
    if let Err(error) = app.deep_link().register_all() {
        eprintln!("schema dei link non registrato: {error}");
    }
    let handle = app.clone();
    app.deep_link()
        .on_open_url(move |event| open_links(&handle, &event.urls()));
    if let Ok(Some(urls)) = app.deep_link().get_current() {
        open_links(app, &urls);
    }
}

#[cfg(test)]
mod tests {
    use domain::{Database, DatabaseKey};

    use super::*;
    use crate::vault::{MemoryBackend, Vault};

    const SCHEME: &str = "portale";
    const ID: &str = "0190a000-0000-7000-8000-000000000001";

    #[test]
    fn only_the_exact_form_is_accepted() {
        let link = format!("{SCHEME}://app/{ID}");
        assert_eq!(parse_app_link(&link, SCHEME), ID.parse().ok());
        for other in [
            format!("{SCHEME}://app/{ID}/"),
            format!("{SCHEME}://app/{ID}?launch=1"),
            format!("{SCHEME}://app/{ID}#x"),
            format!("{SCHEME}://app/{}", ID.to_uppercase()),
            format!("{SCHEME}://app/{}", ID.replace('-', "")),
            format!("{SCHEME}://app/{{{ID}}}"),
            format!("{SCHEME}://app/urn:uuid:{ID}"),
            format!("{SCHEME}://user@app/{ID}"),
            format!("{SCHEME}://app:80/{ID}"),
            format!("{SCHEME}://App/{ID}"),
            format!("{SCHEME}://launch/{ID}"),
            format!("{SCHEME}:app/{ID}"),
            format!("altro://app/{ID}"),
            format!("x{SCHEME}://app/{ID}"),
            format!("{SCHEME}://app/"),
            format!("{SCHEME}://app/{ID}{ID}"),
        ] {
            assert_eq!(parse_app_link(&other, SCHEME), None, "{other}");
        }
        assert_eq!(parse_app_link(&link, ""), None);
    }

    /// Lo schema configurato è il nome del prodotto in minuscolo, letto dalla configurazione
    /// vera: nessuna copia del nome in questo file.
    #[test]
    fn the_configured_scheme_is_the_product_name_in_lowercase() {
        let config: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let expected = domain::APP_NAME.to_lowercase();
        assert_eq!(
            configured_scheme(&config["plugins"]["deep-link"]),
            Some(expected.as_str())
        );
        assert_eq!(configured_scheme(&serde_json::json!({})), None);
        assert_eq!(
            configured_scheme(&serde_json::json!({ "desktop": { "schemes": [""] } })),
            None
        );
    }

    fn state_with_app(pin: bool) -> (tempfile::TempDir, AppState, Uuid) {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::open(
            &dir.path().join("r.db"),
            &DatabaseKey::from_bytes([6; domain::db::KEY_LEN]),
        )
        .unwrap();
        let app = db
            .create_app(&domain::registry::AppInput {
                name: "Portale".into(),
                target: domain::registry::Target::Web {
                    url: "https://example.com/".into(),
                },
                category_id: None,
                tags: Vec::new(),
            })
            .unwrap();
        if pin {
            db.set_pin(None, "482915", 0).unwrap();
        }
        let state = AppState::new(Ok(db), Vault::new(Box::new(MemoryBackend::default())));
        (dir, state, app.id)
    }

    #[test]
    fn a_link_preselects_a_known_app_and_the_last_valid_one_wins() {
        let (_dir, state, id) = state_with_app(false);
        let links = [
            format!("{SCHEME}://app/{ID}"),
            format!("{SCHEME}://app/{id}"),
            "x".into(),
        ];
        assert_eq!(link_target(&state, SCHEME, &links), Some(id));
        // Un id valido ma sconosciuto al registro si ignora.
        assert_eq!(
            link_target(&state, SCHEME, &[format!("{SCHEME}://app/{ID}")]),
            None
        );
    }

    #[test]
    fn while_locked_links_are_dropped() {
        let (_dir, state, id) = state_with_app(true);
        assert!(state.lock.is_locked());
        assert_eq!(
            link_target(&state, SCHEME, &[format!("{SCHEME}://app/{id}")]),
            None
        );
    }
}
