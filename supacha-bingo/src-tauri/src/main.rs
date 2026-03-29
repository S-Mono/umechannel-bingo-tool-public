// // Prevents additional console window on Windows in release, DO NOT REMOVE!!
// #![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// fn main() {
//     supacha_bingo_lib::run()
// }

use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BingoConfig {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

const CONFIG_PATH: &str = "bingo_config.json";

#[tauri::command]
fn save_settings(config: BingoConfig) -> Result<(), String> {
    let json = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;
    fs::write(CONFIG_PATH, json).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn load_settings() -> Result<BingoConfig, String> {
    if std::path::Path::new(CONFIG_PATH).exists() {
        let content = fs::read_to_string(CONFIG_PATH).map_err(|e| e.to_string())?;
        let config: BingoConfig = serde_json::from_str(&content).map_err(|e| e.to_string())?;
        Ok(config)
    } else {
        // 初期値（282x368の画面に合わせたpx指定の目安）
        Ok(BingoConfig { x: 22.0, y: 103.0, w: 237.0, h: 239.0 })
    }
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![save_settings, load_settings])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}