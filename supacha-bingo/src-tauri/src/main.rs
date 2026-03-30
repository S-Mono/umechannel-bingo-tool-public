use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BingoConfig {
    pub x: f64, pub y: f64, pub w: f64, pub h: f64, pub hit_scale: f64,
    pub se_enabled: bool, pub se_volume: f64,
    pub tts_enabled: bool, pub tts_volume: f64, pub tts_repeat_count: i32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BingoSession {
    pub timestamp: String,
    pub hits: Vec<i32>,
}

// ヘルパー関数: 安全な保存先ディレクトリを取得
fn get_app_data_path(app: &AppHandle) -> Result<PathBuf, String> {
    app.path().app_local_data_dir().map_err(|e| e.to_string())
}

#[tauri::command]
fn save_settings(app: AppHandle, config: BingoConfig) -> Result<(), String> {
    let mut path = get_app_data_path(&app)?;
    fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    path.push("bingo_config.json");
    
    let json = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;
    fs::write(path, json).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn load_settings(app: AppHandle) -> Result<BingoConfig, String> {
    let mut path = get_app_data_path(&app)?;
    path.push("bingo_config.json");

    if path.exists() {
        let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
        serde_json::from_str(&content).map_err(|e| e.to_string())
    } else {
        Ok(BingoConfig {
            x: 22.0, y: 109.0, w: 237.0, h: 239.0, hit_scale: 100.0,
            se_enabled: true, se_volume: 20.0, tts_enabled: true, tts_volume: 40.0, tts_repeat_count: 1,
        })
    }
}

#[tauri::command]
fn save_session(app: AppHandle, filename: Option<String>, hits: Vec<i32>) -> Result<String, String> {
    let mut dir = get_app_data_path(&app)?;
    dir.push("sessions");
    fs::create_dir_all(&dir).map_err(|e| format!("Dir creation failed: {}", e))?;

    let file_name = match filename {
        Some(f) if !f.is_empty() => f,
        _ => {
            let now = chrono::Local::now();
            format!("LIVE_at_{}.json", now.format("%Y年%m月%d日_%H時%M分%S秒"))
        }
    };

    let file_path = dir.join(&file_name);
    let session = BingoSession { timestamp: file_name.clone(), hits };
    let json = serde_json::to_string_pretty(&session).map_err(|e| e.to_string())?;
    
    fs::write(file_path, json).map_err(|e| format!("File write failed: {}", e))?;
    Ok(file_name)
}

#[tauri::command]
fn get_sessions(app: AppHandle) -> Result<Vec<String>, String> {
    let mut dir = get_app_data_path(&app)?;
    dir.push("sessions");
    
    if !dir.exists() { return Ok(vec![]); }
    
    let paths = fs::read_dir(dir).map_err(|e| e.to_string())?;
    let mut files: Vec<String> = paths
        .filter_map(|p| p.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|f| f.ends_with(".json"))
        .collect();
    files.sort_by(|a, b| b.cmp(a));
    Ok(files)
}

#[tauri::command]
fn load_session(app: AppHandle, filename: String) -> Result<Vec<i32>, String> {
    let mut path = get_app_data_path(&app)?;
    path.push("sessions");
    path.push(filename);
    
    let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let session: BingoSession = serde_json::from_str(&content).map_err(|e| e.to_string())?;
    Ok(session.hits)
}

#[tauri::command]
fn exit_app(app: tauri::AppHandle) {
    // 0 は正常終了を意味します
    app.exit(0);
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            save_settings, 
            load_settings,
            save_session,
            get_sessions,
            load_session,
            exit_app
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                // mainウィンドウ（設定画面）の場合だけ挙動を変更
                if window.label() == "main" {
                    // 標準の「閉じる（破棄）」処理をキャンセル
                    api.prevent_close();
                    // 代わりに「非表示」にする
                    window.hide().unwrap();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}