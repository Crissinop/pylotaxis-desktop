//! Comandi dei gruppi di avvio (v0.6.0). Creare, modificare ed eliminare passano dal dominio;
//! avviare passa, app per app e nell'ordine del gruppo, dall'unica funzione di avvio
//! (`commands::launch_app`), con le sue verifiche. Un eseguibile cambiato non parte in blocco:
//! finisce tra quelli da confermare, uno per uno.
//!
//! La palette non conferma nulla: se un gruppo avviato da lì lascia qualcosa in sospeso, Rust
//! mostra la finestra principale e le consegna l'esito (`group-result`). Nessun permesso in più
//! alla palette: l'esito parte da Rust, non dal suo webview.

use domain::groups::{Group, GroupInput};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Runtime, State, WebviewWindow};
use uuid::Uuid;

use crate::commands::launch_app;
use crate::errors::CommandError;
use crate::palette::PALETTE_LABEL;
use crate::state::{AppState, parse_id};
use crate::tray::{MAIN_LABEL, quietly, show_main};

/// Evento verso la finestra principale con l'esito di un avvio partito dalla palette, quando
/// qualcosa non è partito.
pub const GROUP_RESULT_EVENT: &str = "group-result";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupDto {
    id: String,
    name: String,
    app_ids: Vec<String>,
}

impl From<Group> for GroupDto {
    fn from(group: Group) -> Self {
        Self {
            id: group.id.to_string(),
            name: group.name,
            app_ids: group.app_ids.iter().map(Uuid::to_string).collect(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GroupInputDto {
    name: String,
    app_ids: Vec<String>,
}

fn to_input(dto: GroupInputDto) -> Result<GroupInput, CommandError> {
    Ok(GroupInput {
        name: dto.name,
        app_ids: dto
            .app_ids
            .iter()
            .map(|id| parse_id(id))
            .collect::<Result<_, _>>()?,
    })
}

/// Esito di un avvio di gruppo: aperte, da confermare (eseguibile cambiato), non riuscite.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupLaunchDto {
    launched: Vec<String>,
    changed: Vec<String>,
    failed: Vec<FailedLaunchDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FailedLaunchDto {
    id: String,
    code: &'static str,
}

/// Esito consegnato alla finestra principale, con il gruppo per poterlo mostrare.
#[derive(Debug, Clone, Serialize)]
struct GroupResultEvent {
    group: GroupDto,
    result: GroupLaunchDto,
}

impl GroupLaunchDto {
    /// Vero se qualcosa non è partito: un eseguibile da confermare o un errore.
    fn has_pending(&self) -> bool {
        !self.changed.is_empty() || !self.failed.is_empty()
    }

    fn record(&mut self, id: Uuid, outcome: Result<(), CommandError>) {
        match outcome {
            Ok(()) => self.launched.push(id.to_string()),
            Err(error) if error.code == domain::Error::HashMismatch.code() => {
                self.changed.push(id.to_string());
            }
            Err(error) => self.failed.push(FailedLaunchDto {
                id: id.to_string(),
                code: error.code,
            }),
        }
    }
}

#[tauri::command]
pub async fn group_create(
    state: State<'_, AppState>,
    input: GroupInputDto,
) -> Result<GroupDto, CommandError> {
    let input = to_input(input)?;
    Ok(state.with_db(|db| Ok(db.create_group(&input)?))?.into())
}

#[tauri::command]
pub async fn group_update(
    state: State<'_, AppState>,
    id: String,
    input: GroupInputDto,
) -> Result<GroupDto, CommandError> {
    let id = parse_id(&id)?;
    let input = to_input(input)?;
    Ok(state.with_db(|db| Ok(db.update_group(id, &input)?))?.into())
}

#[tauri::command]
pub async fn group_delete(state: State<'_, AppState>, id: String) -> Result<(), CommandError> {
    let id = parse_id(&id)?;
    state.with_db(|db| Ok(db.delete_group(id)?))
}

/// Dalla palette, un esito con qualcosa in sospeso va alla finestra principale; dalla finestra
/// principale lo mostra lei stessa.
fn hand_to_main(caller: &str, outcome: &GroupLaunchDto) -> bool {
    caller == PALETTE_LABEL && outcome.has_pending()
}

/// Avvia le app del gruppo in ordine, una dopo l'altra, e riferisce l'esito di ciascuna.
#[tauri::command]
pub async fn group_launch<R: Runtime>(
    app: AppHandle<R>,
    window: WebviewWindow<R>,
    state: State<'_, AppState>,
    id: String,
) -> Result<GroupLaunchDto, CommandError> {
    let id = parse_id(&id)?;
    let group = state.with_db(|db| Ok(db.group(id)?))?;
    let mut outcome = GroupLaunchDto::default();
    for app_id in &group.app_ids {
        outcome.record(*app_id, launch_app(&app, &state, *app_id, false).await);
    }
    if hand_to_main(window.label(), &outcome) {
        let event = GroupResultEvent {
            group: group.into(),
            result: outcome.clone(),
        };
        quietly(app.emit_to(MAIN_LABEL, GROUP_RESULT_EVENT, event));
        show_main(&app);
    }
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_changed_executable_is_set_aside_and_other_failures_keep_their_code() {
        let (a, b, c) = (Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7());
        let mut outcome = GroupLaunchDto::default();
        outcome.record(a, Ok(()));
        outcome.record(b, Err(domain::Error::HashMismatch.into()));
        outcome.record(c, Err(CommandError::LAUNCH_FAILED));
        assert_eq!(outcome.launched, [a.to_string()]);
        assert_eq!(outcome.changed, [b.to_string()]);
        assert_eq!(
            outcome.failed,
            [FailedLaunchDto {
                id: c.to_string(),
                code: "LAUNCH_FAILED"
            }]
        );
        let keys: Vec<String> = serde_json::to_value(&outcome)
            .unwrap()
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect();
        assert_eq!(keys, ["changed", "failed", "launched"]);
    }

    /// L'esito passa alla finestra principale solo se l'avvio viene dalla palette e qualcosa
    /// non è partito; il frontend legge `group` e `result` (src/lib/ipc.ts).
    #[test]
    fn only_a_palette_launch_with_something_pending_goes_to_the_main_window() {
        let mut outcome = GroupLaunchDto::default();
        outcome.record(Uuid::now_v7(), Ok(()));
        assert!(!hand_to_main(PALETTE_LABEL, &outcome), "tutto partito");
        outcome.record(Uuid::now_v7(), Err(domain::Error::HashMismatch.into()));
        assert!(hand_to_main(PALETTE_LABEL, &outcome));
        assert!(
            !hand_to_main(MAIN_LABEL, &outcome),
            "la finestra principale lo mostra già"
        );
        let mut failed = GroupLaunchDto::default();
        failed.record(Uuid::now_v7(), Err(CommandError::LAUNCH_FAILED));
        assert!(hand_to_main(PALETTE_LABEL, &failed));

        let event = GroupResultEvent {
            group: GroupDto {
                id: String::new(),
                name: String::new(),
                app_ids: Vec::new(),
            },
            result: failed,
        };
        let value = serde_json::to_value(&event).unwrap();
        let mut keys: Vec<&String> = value.as_object().unwrap().keys().collect();
        keys.sort_unstable();
        assert_eq!(keys, ["group", "result"]);
        let mut group_keys: Vec<&String> = value["group"].as_object().unwrap().keys().collect();
        group_keys.sort_unstable();
        assert_eq!(group_keys, ["appIds", "id", "name"]);
    }
}
