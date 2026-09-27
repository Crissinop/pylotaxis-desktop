//! Decisione di avvio: cosa aprire e se è sicuro farlo (v0.2.0).
//!
//! Qui si decide, nel guscio Tauri si esegue. Ogni avvio passa da `plan_launch`, così la
//! verifica dell'impronta e la rivalidazione valgono ovunque, palette compresa (A.7.3).

use std::path::PathBuf;

use crate::Error;
use crate::registry::{App, Target, sha256_file, validate_protocol_uri, validate_web_url};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchPlan {
    /// Avvia un programma, con la sua cartella come cartella di lavoro.
    Spawn {
        program: PathBuf,
        working_dir: Option<PathBuf>,
    },
    /// Apre un indirizzo o un URI con il programma predefinito di Windows.
    Open { uri: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedLaunch {
    pub plan: LaunchPlan,
    /// Impronta nuova da registrare: presente solo se l'eseguibile era cambiato e
    /// l'utente ha confermato di volerlo avviare comunque.
    pub new_sha256: Option<String>,
}

/// Decide come avviare `app`.
///
/// Un eseguibile cambiato dall'ultima conferma non parte senza `accept_changed`: le tue app
/// cambiano a ogni build, quindi la conferma deve essere un gesto rapido, ma mai implicito.
/// Indirizzi e URI si rivalidano qui, perché il dato salvato potrebbe essere stato alterato
/// o scritto da una versione con regole diverse (A.7.5).
pub fn plan_launch(app: &App, accept_changed: bool) -> Result<PlannedLaunch, Error> {
    match &app.target {
        Target::Executable { path, sha256 } => {
            if !path.is_file() {
                return Err(Error::ExecutableMissing);
            }
            // Limite noto: tra questa lettura e l'avvio il file potrebbe cambiare. Chi può
            // scriverlo ha già i permessi dell'utente; lo si dichiara, non lo si nasconde.
            let current = sha256_file(path)?;
            let new_sha256 = match (current == *sha256, accept_changed) {
                (true, _) => None,
                (false, true) => Some(current),
                (false, false) => return Err(Error::HashMismatch),
            };
            Ok(PlannedLaunch {
                plan: LaunchPlan::Spawn {
                    program: path.clone(),
                    working_dir: path.parent().map(PathBuf::from),
                },
                new_sha256,
            })
        }
        Target::Web { url } => Ok(PlannedLaunch {
            plan: LaunchPlan::Open {
                uri: validate_web_url(url)?,
            },
            new_sha256: None,
        }),
        Target::Protocol { uri } => Ok(PlannedLaunch {
            plan: LaunchPlan::Open {
                uri: validate_protocol_uri(uri)?,
            },
            new_sha256: None,
        }),
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;
    use crate::registry::tests::{ABC_SHA256, write_exe};

    fn app(target: Target) -> App {
        App {
            id: Uuid::now_v7(),
            name: "Prova".into(),
            target,
            category_id: None,
            tags: Vec::new(),
            environment: None,
            health_check: false,
        }
    }

    #[test]
    fn an_unchanged_executable_starts_from_its_folder() {
        let dir = tempfile::tempdir().unwrap();
        let exe = write_exe(dir.path(), "tool.exe", b"abc");
        let planned = plan_launch(
            &app(Target::Executable {
                path: exe.clone(),
                sha256: ABC_SHA256.into(),
            }),
            false,
        )
        .unwrap();
        assert_eq!(
            planned,
            PlannedLaunch {
                plan: LaunchPlan::Spawn {
                    program: exe,
                    working_dir: Some(dir.path().to_path_buf()),
                },
                new_sha256: None,
            }
        );
    }

    #[test]
    fn a_changed_executable_needs_explicit_consent() {
        let dir = tempfile::tempdir().unwrap();
        let exe = write_exe(dir.path(), "tool.exe", b"abc");
        let target = Target::Executable {
            path: exe.clone(),
            sha256: ABC_SHA256.into(),
        };
        std::fs::write(&exe, b"abcd").unwrap();

        assert!(matches!(
            plan_launch(&app(target.clone()), false),
            Err(Error::HashMismatch)
        ));
        let planned = plan_launch(&app(target), true).unwrap();
        assert_eq!(planned.new_sha256.as_deref().map(str::len), Some(64));
        assert_ne!(planned.new_sha256.as_deref(), Some(ABC_SHA256));
    }

    #[test]
    fn a_missing_executable_is_reported() {
        let target = Target::Executable {
            path: std::env::temp_dir().join("sicuramente-assente-7f3a.exe"),
            sha256: ABC_SHA256.into(),
        };
        assert!(matches!(
            plan_launch(&app(target), true),
            Err(Error::ExecutableMissing)
        ));
    }

    /// Un dato salvato che non rispetta più le regole non si apre: la validazione
    /// all'ingresso non basta (A.7.5, casi negativi A.6 n. 9).
    #[test]
    fn stored_targets_are_revalidated_at_launch() {
        let bad_protocol = app(Target::Protocol {
            uri: "ms-msdt:/id PCWDiagnostic".into(),
        });
        assert!(matches!(
            plan_launch(&bad_protocol, true),
            Err(Error::SchemeNotAllowed)
        ));
        let bad_web = app(Target::Web {
            url: "javascript:alert(1)".into(),
        });
        assert!(matches!(
            plan_launch(&bad_web, true),
            Err(Error::SchemeNotAllowed)
        ));

        let good = app(Target::Web {
            url: "https://example.com".into(),
        });
        assert_eq!(
            plan_launch(&good, false).unwrap().plan,
            LaunchPlan::Open {
                uri: "https://example.com/".into()
            }
        );
    }
}
