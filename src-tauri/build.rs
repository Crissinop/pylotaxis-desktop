// Elenco dei comandi, condiviso con i test del blocco (v0.3.0).
include!("app_commands.rs");

fn main() {
    // Con un manifest dell'app, Tauri controlla i permessi di OGNI comando dell'app:
    // un comando che nessuna capability concede viene rifiutato, quindi un comando
    // nuovo nasce chiuso finché non lo si aggiunge in app_commands.rs e in capabilities/
    // (A.7.10). (v0.1.0)
    //
    // tauri-build dichiara i propri file da osservare, e così cargo smette di rieseguire
    // questo script a ogni modifica del pacchetto: l'elenco va dichiarato a mano, altrimenti
    // un comando aggiunto resterebbe senza permesso fino a una ricompilazione pulita (v0.3.0).
    println!("cargo:rerun-if-changed=app_commands.rs");

    let attributes = tauri_build::Attributes::new()
        .app_manifest(tauri_build::AppManifest::new().commands(APP_COMMANDS));

    if let Err(error) = tauri_build::try_build(attributes) {
        // Uno script di build può solo fallire in modo esplicito: il messaggio completo
        // (es. un permesso inesistente in una capability) finisce nell'output di cargo.
        panic!("tauri-build non riuscito: {error:#}");
    }
}
