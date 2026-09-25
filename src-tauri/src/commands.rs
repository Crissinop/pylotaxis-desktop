//! Comandi esposti al webview. Ogni comando va elencato anche in build.rs e concesso
//! in capabilities/default.json, altrimenti viene rifiutato (A.7.10). (v0.1.0)

use serde::Serialize;

/// Dati dell'app mostrati dall'interfaccia. Nessun dato sensibile (A.7.10). (v0.1.0)
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub name: &'static str,
    pub version: &'static str,
}

#[tauri::command]
pub fn app_info() -> AppInfo {
    AppInfo {
        name: domain::APP_NAME,
        version: env!("CARGO_PKG_VERSION"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_info_reports_the_domain_name_and_the_crate_version() {
        let info = app_info();
        assert_eq!(info.name, domain::APP_NAME);
        assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
    }

    /// Il frontend legge esattamente queste chiavi (src/lib/ipc.ts): se cambiano,
    /// il test fallisce prima che l'interfaccia si rompa in silenzio.
    #[test]
    fn app_info_serializes_with_the_keys_the_frontend_expects() {
        let json = serde_json::to_value(app_info()).unwrap();
        let mut keys: Vec<&str> = json
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        assert_eq!(keys, ["name", "version"]);
    }
}
