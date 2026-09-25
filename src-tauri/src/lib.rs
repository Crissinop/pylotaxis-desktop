//! Guscio Tauri: registra i comandi e avvia la finestra.
//! Le regole vivono nel crate `domain`; qui si valida, si delega e si traduce (A.7.3). (v0.1.0)

mod commands;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Unica eccezione alla regola "niente expect" (A.7.5): se il runtime non si avvia
    // non esiste un comportamento degradato possibile, e il messaggio è l'unica diagnosi.
    #[allow(clippy::expect_used)]
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![commands::app_info])
        .run(tauri::generate_context!())
        .expect("avvio del runtime Tauri non riuscito");
}
