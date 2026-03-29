use serde::{Deserialize, Serialize};
use std::fs;
// chronoを使用するため、SystemTimeなどは削除
// use std::time::{SystemTime, UNIX_EPOCH}; 

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BingoConfig {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub hit_scale: f64,
    pub se_enabled: bool,
    pub se_volume: f64,
    pub tts_enabled: bool,
    pub tts_volume: f64,
    pub tts_repeat_count: i32,
}

const CONFIG_PATH: &str = "bingo_config.json";
const SESSIONS_DIR: &str = "sessions";

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BingoSession {
    pub timestamp: String,
    pub hits: Vec<i32>,
}

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
        Ok(BingoConfig {
            x: 22.0, y: 109.0, w: 237.0, h: 239.0, hit_scale: 100.0,
            se_enabled: true, se_volume: 20.0, tts_enabled: true, tts_volume: 40.0, tts_repeat_count: 1,
        })
    }
}

#[tauri::command]
fn save_session(filename: Option<String>, hits: Vec<i32>) -> Result<String, String> {
    if !std::path::Path::new(SESSIONS_DIR).exists() {
        fs::create_dir(SESSIONS_DIR).map_err(|e| e.to_string())?;
    }

    let file_name = match filename {
        Some(f) => f,
        None => {
            let now = chrono::Local::now();
            format!("session_{}.json", now.format("%Y%m%d_%H%M%S"))
        }
    };

    let path = format!("{}/{}", SESSIONS_DIR, file_name);
    let session = BingoSession { timestamp: file_name.clone(), hits };
    let json = serde_json::to_string_pretty(&session).map_err(|e| e.to_string())?;
    
    fs::write(path, json).map_err(|e| e.to_string())?;
    Ok(file_name) 
}

#[tauri::command]
fn get_sessions() -> Result<Vec<String>, String> {
    if !std::path::Path::new(SESSIONS_DIR).exists() { return Ok(vec![]); }
    let paths = fs::read_dir(SESSIONS_DIR).map_err(|e| e.to_string())?;
    let mut files = Vec::new();
    for path in paths {
        if let Ok(entry) = path {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.ends_with(".json") { files.push(name); }
        }
    }
    files.sort_by(|a, b| b.cmp(a));
    Ok(files)
}

#[tauri::command]
fn load_session(filename: String) -> Result<Vec<i32>, String> {
    let path = format!("{}/{}", SESSIONS_DIR, filename);
    let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let session: BingoSession = serde_json::from_str(&content).map_err(|e| e.to_string())?;
    Ok(session.hits)
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![save_settings, load_settings, save_session, get_sessions, load_session])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}