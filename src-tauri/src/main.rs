#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if std::env::args().any(|a| a == "--borrar-credenciales") {
        glassdeck_lib::borrar_credenciales();
        return;
    }
    glassdeck_lib::run()
}
