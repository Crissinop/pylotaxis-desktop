//! Scorciatoia globale della palette: un elenco chiuso di combinazioni (v0.5.0).
//!
//! Elenco chiuso e non combinazione libera: ogni voce è scelta per non scontrarsi con le
//! scorciatoie di Windows, e il registro non può contenere un testo che il plugin non sa
//! interpretare. Il testo è quello del plugin (`global-hotkey`); il guscio ha un test che
//! lo interpreta voce per voce.

use crate::{Database, Error};

/// Chiave nella tabella `settings`, che esiste dalla v0.1.0: nessuna migrazione.
pub const PALETTE_SHORTCUT_SETTING: &str = "palette_shortcut";

/// Combinazioni ammesse, la prima è la predefinita. Con la tastiera italiana AltGr vale
/// Ctrl+Alt, quindi AltGr+Spazio aprirebbe la palette: Ctrl+Maiusc+Spazio è l'alternativa
/// senza Alt. (Ipotesi da provare sulla macchina.)
pub const PALETTE_SHORTCUTS: &[&str] = &["Ctrl+Alt+Space", "Ctrl+Shift+Space", "Ctrl+Alt+P"];

pub const DEFAULT_PALETTE_SHORTCUT: &str = PALETTE_SHORTCUTS[0];

/// La voce dell'elenco identica a `raw`; qualunque altra forma è `SHORTCUT_INVALID`.
pub fn validate_shortcut(raw: &str) -> Result<&'static str, Error> {
    PALETTE_SHORTCUTS
        .iter()
        .copied()
        .find(|shortcut| *shortcut == raw)
        .ok_or(Error::ShortcutInvalid)
}

impl Database {
    /// Scorciatoia salvata. Un valore assente, o cambiato fuori dall'app e non più
    /// nell'elenco, vale la predefinita: una palette raggiungibile è meglio di nessuna.
    pub fn palette_shortcut(&self) -> Result<&'static str, Error> {
        Ok(self
            .setting(PALETTE_SHORTCUT_SETTING)?
            .and_then(|saved| validate_shortcut(&saved).ok())
            .unwrap_or(DEFAULT_PALETTE_SHORTCUT))
    }

    /// Salva una voce dell'elenco; un testo non ammesso non tocca il valore salvato.
    pub fn set_palette_shortcut(&self, raw: &str) -> Result<&'static str, Error> {
        let shortcut = validate_shortcut(raw)?;
        self.set_setting(PALETTE_SHORTCUT_SETTING, shortcut)?;
        Ok(shortcut)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DatabaseKey;
    use crate::db::KEY_LEN;

    fn database() -> (tempfile::TempDir, Database) {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::open(
            &dir.path().join("r.db"),
            &DatabaseKey::from_bytes([5; KEY_LEN]),
        )
        .unwrap();
        (dir, db)
    }

    #[test]
    fn without_a_saved_value_the_default_applies() {
        let (_dir, db) = database();
        assert_eq!(db.palette_shortcut().unwrap(), "Ctrl+Alt+Space");
    }

    #[test]
    fn a_listed_shortcut_is_saved_and_read_back() {
        let (_dir, db) = database();
        for shortcut in PALETTE_SHORTCUTS {
            assert_eq!(db.set_palette_shortcut(shortcut).unwrap(), *shortcut);
            assert_eq!(db.palette_shortcut().unwrap(), *shortcut);
        }
    }

    /// Solo la forma esatta dell'elenco: niente varianti di maiuscole, spazi o combinazioni
    /// di sistema, e il valore salvato resta quello di prima.
    #[test]
    fn anything_else_is_refused_and_leaves_the_saved_value_alone() {
        let (_dir, db) = database();
        db.set_palette_shortcut("Ctrl+Alt+P").unwrap();
        for raw in [
            "",
            "ctrl+alt+space",
            " Ctrl+Alt+Space",
            "Ctrl+Alt+Space ",
            "Ctrl+Alt+Space+",
            "Alt+Ctrl+Space",
            "Ctrl+Alt+Delete",
            "Super+L",
        ] {
            assert!(
                matches!(db.set_palette_shortcut(raw), Err(Error::ShortcutInvalid)),
                "{raw:?} deve essere rifiutata"
            );
        }
        assert_eq!(db.palette_shortcut().unwrap(), "Ctrl+Alt+P");
    }

    #[test]
    fn a_value_changed_outside_the_app_falls_back_to_the_default() {
        let (_dir, db) = database();
        db.set_setting(PALETTE_SHORTCUT_SETTING, "Ctrl+Alt+Delete")
            .unwrap();
        assert_eq!(db.palette_shortcut().unwrap(), DEFAULT_PALETTE_SHORTCUT);
    }
}
