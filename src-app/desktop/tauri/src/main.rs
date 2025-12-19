// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Get config file from CONFIG_FILE env var (like the server)
    let config_file = std::env::var("CONFIG_FILE").ok();

    ziee_chat_desktop::run(config_file).expect("Failed to run desktop app");
}
