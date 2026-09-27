// Elenco unico dei comandi dell'app (v0.3.0).
//
// Lo include build.rs, che genera un permesso per ognuno, e lo includono i test del blocco,
// che provano su ognuno che da bloccata venga rifiutato. Un comando nuovo va aggiunto qui,
// in `generate_handler!` (src/lib.rs) e in capabilities/default.json.
// Solo commenti `//`: il file si include con `include!`, anche fuori da un modulo.

pub const APP_COMMANDS: &[&str] = &[
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
    "lock_status",
    "lock_now",
    "unlock_pin",
    "unlock_hello",
    "hello_availability",
    "security_set_pin",
    "security_disable",
    "security_set_hello",
    "security_set_idle",
    "secret_create",
    "secret_update",
    "secret_replace",
    "secret_delete",
    "secret_copy",
    // Rapidità (v0.5.0): scorciatoia, tray, palette.
    "shortcut_status",
    "shortcut_set",
    "tray_setup",
    "main_window_show",
    "palette_ready",
    // Organizzazione (v0.6.0): gruppi di avvio, stato delle app, avvio con Windows.
    "group_create",
    "group_update",
    "group_delete",
    "group_launch",
    "health_status",
    "autostart_status",
    "autostart_set",
];
