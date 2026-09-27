//! Stato delle app web (v0.6.0).
//!
//! Il frontend chiede lo stato quando una sua finestra è in vista; Rust ricontrolla solo i
//! risultati scaduti (`domain::health`), uno per app alla volta, in un thread di lavoro, e
//! annuncia ogni risultato con `health-changed`. Mai in background: con la finestra nascosta o
//! ridotta a icona non parte nessuna richiesta, e da bloccata il comando è rifiutato e i
//! risultati in arrivo si scartano. La richiesta non porta credenziali né cookie, non segue i
//! reindirizzamenti e va diretta, senza proxy. Il TLS è quello di Windows (SChannel) con i
//! certificati di sistema: le autorità interne fidate da Windows valgono anche qui.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use domain::health::{HEALTH_TIMEOUT, HealthStatus, Probe, classify, is_stale};
use domain::registry::Target;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime, State, WebviewWindow};
use ureq::tls::{RootCerts, TlsConfig, TlsProvider};
use uuid::Uuid;

use crate::errors::CommandError;
use crate::state::AppState;
use crate::tray::quietly;

/// Evento a ogni risultato nuovo, con id e stato.
pub const HEALTH_EVENT: &str = "health-changed";

/// Ultimo risultato di ogni app con il controllo acceso. Solo in memoria: uno stato vecchio
/// salvato su disco direbbe il falso al prossimo avvio.
#[derive(Default)]
pub struct HealthState {
    entries: Mutex<HashMap<Uuid, Entry>>,
}

struct Entry {
    url: String,
    status: Option<HealthStatus>,
    checked: Option<Instant>,
    in_flight: bool,
}

impl Entry {
    fn new(url: &str) -> Self {
        Self {
            url: url.to_owned(),
            status: None,
            checked: None,
            in_flight: false,
        }
    }
}

/// Stato di un'app per il frontend; `None` finché non c'è un risultato.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthDto {
    id: String,
    status: Option<&'static str>,
}

type Due = Vec<(Uuid, String)>;

impl HealthState {
    /// Allinea la memoria alle app con il controllo acceso e sceglie quelle da controllare
    /// ora: scadute e senza una richiesta già in corso. Un indirizzo cambiato vale come mai
    /// controllato. Con `start` falso (finestra non in vista) non ne sceglie nessuna.
    fn plan(
        &self,
        apps: &[(Uuid, String)],
        now: Instant,
        start: bool,
    ) -> Result<(Vec<HealthDto>, Due), CommandError> {
        let mut entries = self
            .entries
            .lock()
            .map_err(|_| CommandError::STATE_UNAVAILABLE)?;
        entries.retain(|id, _| apps.iter().any(|(app, _)| app == id));
        let mut statuses = Vec::with_capacity(apps.len());
        let mut due = Vec::new();
        for (id, url) in apps {
            let entry = entries.entry(*id).or_insert_with(|| Entry::new(url));
            if entry.url != *url {
                *entry = Entry::new(url);
            }
            let age = entry.checked.map(|at| now.saturating_duration_since(at));
            if start && !entry.in_flight && is_stale(age) {
                entry.in_flight = true;
                due.push((*id, url.clone()));
            }
            statuses.push(HealthDto {
                id: id.to_string(),
                status: entry.status.map(HealthStatus::code),
            });
        }
        Ok((statuses, due))
    }

    /// Chiude una richiesta. Vero se il risultato è stato conservato: l'app ha ancora il
    /// controllo acceso, lo stesso indirizzo, e c'è un risultato (da bloccata si scarta).
    fn finish(&self, id: Uuid, url: &str, status: Option<HealthStatus>, now: Instant) -> bool {
        let Ok(mut entries) = self.entries.lock() else {
            return false;
        };
        match entries.get_mut(&id) {
            Some(entry) if entry.url == url => {
                entry.in_flight = false;
                if let Some(status) = status {
                    entry.status = Some(status);
                    entry.checked = Some(now);
                    return true;
                }
                false
            }
            _ => false,
        }
    }
}

/// Client senza reindirizzamenti, senza proxy, con un tempo massimo complessivo e il TLS di
/// sistema. Un codice HTTP di errore è una risposta, non un errore di `ureq`.
fn agent(timeout: Duration) -> ureq::Agent {
    let tls = TlsConfig::builder()
        .provider(TlsProvider::NativeTls)
        .root_certs(RootCerts::PlatformVerifier)
        .build();
    ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .max_redirects(0)
        .http_status_as_error(false)
        .proxy(None)
        .tls_config(tls)
        .build()
        .into()
}

/// Una richiesta GET, senza leggere il corpo della risposta.
pub fn probe(url: &str, timeout: Duration) -> Probe {
    match agent(timeout).get(url).call() {
        Ok(response) => Probe::Response(response.status().as_u16()),
        Err(ureq::Error::StatusCode(code)) => Probe::Response(code),
        Err(ureq::Error::Tls(_) | ureq::Error::NativeTls(_)) => Probe::TlsFailure,
        Err(_) => Probe::NoResponse,
    }
}

/// Stato delle app con il controllo acceso; avvia in un thread di lavoro i controlli scaduti,
/// solo se la finestra che chiede è in vista.
#[tauri::command]
pub async fn health_status<R: Runtime>(
    app: AppHandle<R>,
    window: WebviewWindow<R>,
    state: State<'_, AppState>,
) -> Result<Vec<HealthDto>, CommandError> {
    let apps: Vec<(Uuid, String)> = state
        .with_db(|db| Ok(db.registry()?.apps))?
        .into_iter()
        .filter(|entry| entry.health_check)
        .filter_map(|entry| match entry.target {
            Target::Web { url } => Some((entry.id, url)),
            _ => None,
        })
        .collect();
    // Il frontend chiede solo quando è in vista; qui lo si verifica comunque.
    let in_view = window.is_visible().unwrap_or(false) && !window.is_minimized().unwrap_or(true);
    let (statuses, due) = state.health.plan(&apps, Instant::now(), in_view)?;
    for (id, url) in due {
        let handle = app.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let status = classify(probe(&url, HEALTH_TIMEOUT));
            let Some(state) = handle.try_state::<AppState>() else {
                return;
            };
            let kept = (!state.lock.is_locked()).then_some(status);
            if state.health.finish(id, &url, kept, Instant::now()) {
                let payload = HealthDto {
                    id: id.to_string(),
                    status: Some(status.code()),
                };
                quietly(handle.emit(HEALTH_EVENT, payload));
            }
        });
    }
    Ok(statuses)
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    use super::*;

    const QUICK: Duration = Duration::from_millis(500);

    /// Server locale che risponde una volta con `response` (vuoto: accetta e tace).
    fn serve(response: &'static str) -> std::net::SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut request = [0; 2048];
                let _ = stream.read(&mut request);
                if response.is_empty() {
                    thread::sleep(Duration::from_secs(3));
                } else {
                    let _ = stream.write_all(response.as_bytes());
                }
            }
        });
        address
    }

    const OK: &str = "HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";

    /// Richieste vere, solo verso 127.0.0.1: codici, reindirizzamento non seguito, porta
    /// chiusa, server muto, TLS verso un server che non lo parla.
    #[test]
    fn a_real_request_is_classified_by_what_the_server_does() {
        let url = |address, scheme: &str| format!("{scheme}://{address}/");
        assert_eq!(probe(&url(serve(OK), "http"), QUICK), Probe::Response(200));
        let busy =
            "HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
        assert_eq!(
            probe(&url(serve(busy), "http"), QUICK),
            Probe::Response(503)
        );
        let moved = "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:9/\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
        assert_eq!(
            probe(&url(serve(moved), "http"), QUICK),
            Probe::Response(302)
        );
        let closed = TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap();
        assert_eq!(probe(&url(closed, "http"), QUICK), Probe::NoResponse);
        assert_eq!(probe(&url(serve(""), "http"), QUICK), Probe::NoResponse);
        assert_eq!(probe(&url(serve(OK), "https"), QUICK), Probe::TlsFailure);
    }

    fn apps(urls: &[(Uuid, &str)]) -> Vec<(Uuid, String)> {
        urls.iter()
            .map(|(id, url)| (*id, (*url).to_owned()))
            .collect()
    }

    #[test]
    fn only_expired_results_are_checked_once_and_only_in_view() {
        let state = HealthState::default();
        let (a, b) = (Uuid::now_v7(), Uuid::now_v7());
        let list = apps(&[(a, "https://a.example/"), (b, "https://b.example/")]);
        let start = Instant::now();
        let (_, due) = state.plan(&list, start, false).unwrap();
        assert!(due.is_empty(), "fuori vista nessuna richiesta");
        let (statuses, due) = state.plan(&list, start, true).unwrap();
        assert_eq!(due.len(), 2);
        assert!(statuses.iter().all(|s| s.status.is_none()));
        let (_, due) = state.plan(&list, start, true).unwrap();
        assert!(due.is_empty(), "una richiesta alla volta per app");
        assert!(state.finish(a, "https://a.example/", Some(HealthStatus::Active), start));
        assert!(
            !state.finish(b, "https://b.example/", None, start),
            "da bloccata si scarta"
        );
        let (statuses, due) = state
            .plan(&list, start + Duration::from_secs(30), true)
            .unwrap();
        assert_eq!(
            due,
            [(b, "https://b.example/".to_owned())],
            "a è ancora valido"
        );
        assert_eq!(statuses[0].status, Some("active"));
        state.finish(
            b,
            "https://b.example/",
            Some(HealthStatus::Unreachable),
            start + Duration::from_secs(30),
        );
        let (_, due) = state
            .plan(&list, start + Duration::from_secs(61), true)
            .unwrap();
        assert_eq!(due.len(), 1, "a è scaduto, b no");
    }

    #[test]
    fn a_changed_address_or_a_switched_off_check_forgets_the_old_result() {
        let state = HealthState::default();
        let a = Uuid::now_v7();
        let now = Instant::now();
        state
            .plan(&apps(&[(a, "https://a.example/")]), now, true)
            .unwrap();
        assert!(state.finish(a, "https://a.example/", Some(HealthStatus::Active), now));
        let (statuses, due) = state
            .plan(&apps(&[(a, "https://nuovo.example/")]), now, true)
            .unwrap();
        assert_eq!((statuses[0].status, due.len()), (None, 1));
        assert!(!state.finish(a, "https://a.example/", Some(HealthStatus::Active), now));
        let (statuses, _) = state.plan(&[], now, true).unwrap();
        assert!(statuses.is_empty());
        assert!(!state.finish(a, "https://nuovo.example/", Some(HealthStatus::Active), now));
    }
}
