#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::AppHandle; // Manager を削除
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};
use log::info; // error を削除（log::error! と直接記述するため）
use tauri_plugin_log::{Target, TargetKind};

// --- データ構造体 ---
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

// --- ヘルパー関数 ---
fn get_base_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|s| s.to_path_buf()))
        .expect("Failed to get executable directory")
}

fn get_adjacent_path(_app: &AppHandle, sub_path: &str) -> PathBuf {
    get_base_dir().join(sub_path)
}

// --- Tauri コマンドの実装 ---

#[tauri::command]
fn log_action(trigger: String, message: String) {
    // 2行構成のログ。save_session と同様のインデントで視認性を統一
    info!("[ACTION] ユーザー操作を受信しました: <{}>", trigger);
    info!("   └ [EVENT] {}", message);
}

#[tauri::command]
fn save_settings(app: AppHandle, config: BingoConfig) -> Result<(), String> {
    let path = get_adjacent_path(&app, "bingo_config.json");
    info!("設定の保存を開始します。パス: {:?}", path);

    let json = serde_json::to_string_pretty(&config).map_err(|e| {
        log::error!("設定のシリアライズに失敗しました: {}", e);
        e.to_string()
    })?;

    fs::write(&path, json).map_err(|e| {
        log::error!("設定ファイルの書き込みに失敗しました ({:?}): {}", path, e);
        e.to_string()
    })?;

    info!("設定の保存が正常に完了しました。");
    Ok(())
}

#[tauri::command]
fn load_settings(app: AppHandle) -> Result<BingoConfig, String> {
    let path = get_adjacent_path(&app, "bingo_config.json");
    info!("設定の読み込みを試行します。パス: {:?}", path);

    if path.exists() {
        let content = fs::read_to_string(&path).map_err(|e| {
            log::error!("設定ファイルの読み込みに失敗しました: {}", e);
            e.to_string()
        })?;
        let config = serde_json::from_str(&content).map_err(|e| {
            log::error!("設定データの解析に失敗しました: {}", e);
            e.to_string()
        })?;
        info!("設定の読み込みに成功しました。");
        Ok(config)
    } else {
        info!("設定ファイルが見つからないため、初期設定値を適用します。");
        Ok(BingoConfig {
            x: 22.0, y: 109.0, w: 237.0, h: 239.0, hit_scale: 100.0,
            se_enabled: true, se_volume: 50.0, tts_enabled: true, tts_volume: 50.0, tts_repeat_count: 1,
        })
    }
}

#[tauri::command]
fn save_session(
    app: AppHandle, 
    filename: Option<String>, 
    hits: Vec<i32>, 
    trigger: String  // フロントエンドから受け取る契機
) -> Result<String, String> {
    
    // --- 1行目: アクションの発生を記録 ---
    info!("[ACTION] ユーザー操作を受信しました: <{}>", trigger);

    let dir = get_adjacent_path(&app, "sessions");
    if !dir.exists() {
        let _ = fs::create_dir_all(&dir);
    }

    let file_name = match filename {
        Some(f) if !f.is_empty() => f,
        _ => chrono::Local::now().format("LIVE_at_%Y年%m月%d日_%H時%M分%S秒.json").to_string(),
    };

    let file_path = dir.join(&file_name);
    let session = BingoSession { timestamp: file_name.clone(), hits: hits.clone() };
    let json = serde_json::to_string_pretty(&session).map_err(|e| e.to_string())?;
    
    fs::write(&file_path, json).map_err(|e| {
        log::error!("   └ [ERROR] 保存失敗: {}", e);
        format!("保存失敗: {}", e)
    })?;

    // --- 2行目: 保存結果を記録（インデントをつけて因果関係を表現） ---
    let last_num = hits.last().cloned().unwrap_or(0);
    info!("   └ [SAVE] セッションを更新: {} (最新: {}, 合計: {}件)", file_name, last_num, hits.len());

    Ok(file_name)
}

#[tauri::command]
fn get_sessions(app: AppHandle) -> Result<Vec<String>, String> {
    let dir = get_adjacent_path(&app, "sessions");
    
    // 正常系の「スキャン開始ログ」を削除（ここが冗長の原因）
    if !dir.exists() { return Ok(vec![]); }
    
    let paths = fs::read_dir(&dir).map_err(|e| {
        log::error!("スキャン失敗: {}", e);
        e.to_string()
    })?;

    let mut files: Vec<String> = paths
        .filter_map(|p| p.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|f| f.ends_with(".json"))
        .collect();

    files.sort_by(|a, b| b.cmp(a));
    // 「〇〇件検出しました」も削除し、必要な時だけログを見るようにする
    Ok(files)
}

#[tauri::command]
fn load_session(app: AppHandle, filename: String) -> Result<Vec<i32>, String> {
    info!("セッションの復元を開始します。対象ファイル: {}", filename);
    let mut path = get_adjacent_path(&app, "sessions");
    path.push(&filename);
    
    let content = fs::read_to_string(&path).map_err(|e| {
        log::error!("セッションファイルの読み込みに失敗しました ({:?}): {}", path, e);
        e.to_string()
    })?;

    let session: BingoSession = serde_json::from_str(&content).map_err(|e| {
        log::error!("セッションデータの解析に失敗しました: {}", e);
        e.to_string()
    })?;
    
    info!("セッションの復元に成功しました。ヒット番号: {:?}", session.hits);
    Ok(session.hits)
}

#[tauri::command]
fn exit_app(app: AppHandle) {
    info!("ユーザーによるアプリケーション終了リクエストを受信しました。");
    app.exit(0);
}

// --- メイン関数 ---

fn main() {
    let base_dir = get_base_dir();
    let session_id = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
    let log_filename = format!("session_{}", session_id);

    // 先行書き込みテスト
    let test_file = base_dir.join(".write_test");
    let has_write_permission = match fs::write(&test_file, "test") {
        Ok(_) => {
            let _ = fs::remove_file(test_file);
            true
        }
        Err(_) => false,
    };

    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init());

    // ロギング設定の構築
    let mut log_builder = tauri_plugin_log::Builder::new()
        .targets([
            Target::new(TargetKind::Stdout),
            Target::new(TargetKind::Webview),
        ]);

    if has_write_permission {
        log_builder = log_builder.target(Target::new(TargetKind::LogDir { 
            file_name: Some(log_filename) 
        }));
    }

    builder = builder.plugin(log_builder.build());

    builder
        .setup(move |app| {
            let app_handle = app.handle();
            
            if !has_write_permission {
                log::error!("CRITICAL: 起動ディレクトリへの書き込み権限がありません。アプリを停止します。");
                let handle = app_handle.clone();
                app.dialog()
                    .message("アプリケーション起動場所の書き込み権限がありません。\nデスクトップやドキュメントフォルダにアプリを配置して再実行してください。")
                    .title("権限エラー")
                    .kind(MessageDialogKind::Error)
                    .show(move |_| { handle.exit(1); });
                return Err("Permission denied".into());
            }

            let log_dir = base_dir.join("logs");
            if !log_dir.exists() { 
                let _ = fs::create_dir_all(&log_dir); 
            }
            if let Ok(entries) = fs::read_dir(&log_dir) {
                let mut log_files: Vec<_> = entries.filter_map(|e| e.ok()).collect();
                log_files.sort_by_key(|e| e.metadata().and_then(|m| m.modified()).ok());
                if log_files.len() > 50 {
                    let delete_count = log_files.len() - 50;
                    info!("古いログファイルを {} 件削除します。", delete_count);
                    for i in 0..delete_count { 
                        let _ = fs::remove_file(log_files[i].path()); 
                    }
                }
            }

            info!("Application started in portable mode. (Session ID: {})", session_id);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            save_settings, load_settings, save_session, get_sessions, load_session, exit_app, log_action
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}