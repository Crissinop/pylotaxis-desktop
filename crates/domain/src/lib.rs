//! Dominio del portale: regole e persistenza, senza dipendenze da Tauri.
//!
//! Tutto ciò che decide vive qui e si prova con `cargo test`, senza finestra né
//! webview; il guscio in `src-tauri` si limita a esporlo (A.7.3). (v0.1.0)

pub mod db;
mod error;
mod hex;
pub mod launch;
pub mod lock;
pub mod registry;
pub mod secrets;
pub mod shortcut;

pub use db::{Database, DatabaseKey, KeyPlan, plan_key};
pub use error::Error;

/// Nome del prodotto. È uno dei punti elencati nella Scheda (A.2): un cambio di
/// nome passa di qui, e `scripts/check-name.mjs` verifica che coincida con
/// `productName` e con la costante TypeScript. (v0.1.0)
pub const APP_NAME: &str = "Pylotaxis";
