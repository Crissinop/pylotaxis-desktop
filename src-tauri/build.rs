fn main() {
    // Con un manifest dell'app, Tauri controlla i permessi di OGNI comando dell'app:
    // un comando che nessuna capability concede viene rifiutato, quindi un comando
    // nuovo nasce chiuso finché non lo si aggiunge qui e in capabilities/ (A.7.10). (v0.1.0)
    const APP_COMMANDS: &[&str] = &[
        "app_info",
        "registry_list",
        "category_create",
        "category_rename",
        "category_delete",
        "executable_pick",
        "app_create",
        "app_update",
        "app_delete",
        "app_launch",
    ];

    let attributes = tauri_build::Attributes::new()
        .app_manifest(tauri_build::AppManifest::new().commands(APP_COMMANDS));

    if let Err(error) = tauri_build::try_build(attributes) {
        // Uno script di build può solo fallire in modo esplicito: il messaggio completo
        // (es. un permesso inesistente in una capability) finisce nell'output di cargo.
        panic!("tauri-build non riuscito: {error:#}");
    }
}
