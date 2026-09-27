include!("app_commands.rs");

/// Manifest di Windows dell'app: dichiara Common Controls v6, di cui ha bisogno il plugin
/// dialog (`TaskDialogIndirect`). È la copia esatta di quello predefinito di tauri-build 2.6.3,
/// così l'app si comporta come prima. (v0.6.1)
const WINDOWS_MANIFEST: &str = "windows-app-manifest.xml";

fn main() {
    println!("cargo:rerun-if-changed=app_commands.rs");

    let mut attributes = tauri_build::Attributes::new()
        .app_manifest(tauri_build::AppManifest::new().commands(APP_COMMANDS));

    // tauri-build incorpora il manifest come risorsa solo negli eseguibili (embed-resource
    // emette `rustc-link-arg-bins`): i binari dei test ne restano senza, Windows carica
    // Common Controls 5.82, che non esporta `TaskDialogIndirect`, e il test si ferma con
    // STATUS_ENTRYPOINT_NOT_FOUND. Con il linker MSVC il manifest si incorpora dal linker per
    // tutti i target, test compresi, e tauri-build smette di aggiungerne un secondo. Con gli
    // altri linker resta il comportamento predefinito. (v0.6.1)
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        attributes = attributes
            .windows_attributes(tauri_build::WindowsAttributes::new_without_app_manifest());
        embed_windows_manifest();
    }

    if let Err(error) = tauri_build::try_build(attributes) {
        panic!("tauri-build non riuscito: {error:#}");
    }
}

/// Un file letto dal linker si dichiara a mano: tauri-build fissa i propri
/// `rerun-if-changed` e cargo non rieseguirebbe questo script (lezione 37).
fn embed_windows_manifest() {
    let Ok(dir) = std::env::var("CARGO_MANIFEST_DIR") else {
        panic!("CARGO_MANIFEST_DIR non impostata da cargo");
    };
    let manifest = std::path::Path::new(&dir).join(WINDOWS_MANIFEST);
    println!("cargo:rerun-if-changed={}", manifest.display());
    println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
    println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
}
