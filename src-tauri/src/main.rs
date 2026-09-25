// Niente finestra della console su Windows nelle build di release. Da non rimuovere.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    app_lib::run();
}
