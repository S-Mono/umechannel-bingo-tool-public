#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, Size, WindowEvent};
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};
use log::info;
use tauri_plugin_log::{Target, TargetKind};

const DEFAULT_NORMAL_BINGO_VIDEO_PATH: &str = "effects/normal_bingo.mp4";
const DEFAULT_SPECIAL_1_VIDEO_PATH: &str = "effects/special_1.mp4";
const DEFAULT_SPECIAL_25_VIDEO_PATH: &str = "effects/special_25.mp4";
const MAIN_WINDOW_DEFAULT_WIDTH: u32 = 450;
const MAIN_WINDOW_DEFAULT_HEIGHT: u32 = 650;

// --- データ構造体 ---
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct BingoConfig {
    pub x: f64, pub y: f64, pub w: f64, pub h: f64, pub hit_scale: f64,
    pub se_enabled: bool, pub se_volume: f64,
    pub tts_enabled: bool, pub tts_volume: f64, pub tts_repeat_count: i32,
    pub main_window_x: Option<i32>,
    pub main_window_y: Option<i32>,
    pub effect_enabled: bool,
    pub effect_monitor_id: String,
    pub normal_bingo_effect_enabled: bool,
    pub special_1_effect_enabled: bool,
    pub special_25_effect_enabled: bool,
    pub normal_bingo_video_path: String,
    pub special_1_video_path: String,
    pub special_25_video_path: String,
}

impl Default for BingoConfig {
    fn default() -> Self {
        Self {
            x: 22.0,
            y: 109.0,
            w: 237.0,
            h: 239.0,
            hit_scale: 100.0,
            se_enabled: true,
            se_volume: 50.0,
            tts_enabled: true,
            tts_volume: 50.0,
            tts_repeat_count: 1,
            main_window_x: None,
            main_window_y: None,
            effect_enabled: true,
            effect_monitor_id: String::new(),
            normal_bingo_effect_enabled: true,
            special_1_effect_enabled: false,
            special_25_effect_enabled: false,
            normal_bingo_video_path: DEFAULT_NORMAL_BINGO_VIDEO_PATH.to_string(),
            special_1_video_path: DEFAULT_SPECIAL_1_VIDEO_PATH.to_string(),
            special_25_video_path: DEFAULT_SPECIAL_25_VIDEO_PATH.to_string(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BingoSession {
    pub timestamp: String,
    pub hits: Vec<i32>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct MonitorInfo {
    id: String,
    label: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
enum EffectType {
    #[serde(rename = "NORMAL_BINGO")]
    NormalBingo,
    #[serde(rename = "SPECIAL_1", alias = "SPECIAL1")]
    Special1,
    #[serde(rename = "SPECIAL_25", alias = "SPECIAL25")]
    Special25,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct EffectPayload {
    effect_type: EffectType,
    video_path: String,
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

fn get_config_path(app: &AppHandle) -> PathBuf {
    get_adjacent_path(app, "bingo_config.json")
}

fn build_monitor_id(monitor: &tauri::Monitor) -> String {
    format!(
        "{}__{}_{}__{}_{}__{:.2}",
        monitor.name().cloned().unwrap_or_else(|| "monitor".to_string()),
        monitor.position().x,
        monitor.position().y,
        monitor.size().width,
        monitor.size().height,
        monitor.scale_factor()
    )
}

fn resolve_video_path(raw_path: &str) -> PathBuf {
    let candidate = PathBuf::from(raw_path);
    if candidate.is_absolute() {
        candidate
    } else {
        get_base_dir().join(candidate)
    }
}

fn save_config_file(path: &PathBuf, config: &BingoConfig) -> Result<(), String> {
    let json = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    fs::write(path, json).map_err(|e| e.to_string())
}

fn merge_json_defaults(target: &mut Value, defaults: &Value) -> bool {
    if target.is_null() {
        *target = defaults.clone();
        return true;
    }

    match (target, defaults) {
        (Value::Object(target_map), Value::Object(default_map)) => {
            let mut changed = false;
            for (key, default_value) in default_map {
                match target_map.get_mut(key) {
                    Some(existing_value) => {
                        changed |= merge_json_defaults(existing_value, default_value);
                    }
                    None => {
                        target_map.insert(key.clone(), default_value.clone());
                        changed = true;
                    }
                }
            }
            changed
        }
        (current, default_value) => {
            if std::mem::discriminant(current) != std::mem::discriminant(default_value) {
                *current = default_value.clone();
                true
            } else {
                false
            }
        }
    }
}

fn load_or_initialize_config(app: &AppHandle) -> Result<(BingoConfig, bool), String> {
    let path = get_config_path(app);
    let default_config = BingoConfig::default();
    let default_value = serde_json::to_value(&default_config).map_err(|e| e.to_string())?;

    if !path.exists() {
        save_config_file(&path, &default_config)?;
        return Ok((default_config, true));
    }

    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let mut normalized_value = match serde_json::from_str::<Value>(&content) {
        Ok(value) if value.is_object() => value,
        Ok(_) => {
            log::warn!("設定ファイルの形式が不正なため、既定値で再構築します。パス: {:?}", path);
            default_value.clone()
        }
        Err(error) => {
            log::warn!("設定ファイルの解析に失敗したため、既定値で再構築します。{}", error);
            default_value.clone()
        }
    };

    let mut was_updated = merge_json_defaults(&mut normalized_value, &default_value);
    let config = match serde_json::from_value::<BingoConfig>(normalized_value.clone()) {
        Ok(config) => config,
        Err(error) => {
            log::warn!("設定ファイルに不正な値が含まれるため、既定値で再構築します。{}", error);
            was_updated = true;
            default_config.clone()
        }
    };

    if was_updated {
        save_config_file(&path, &config)?;
    }

    Ok((config, was_updated))
}

fn find_monitor(app: &AppHandle, monitor_id: &str) -> Result<Option<tauri::Monitor>, String> {
    let monitors = app.available_monitors().map_err(|e| e.to_string())?;
    Ok(monitors.into_iter().find(|monitor| build_monitor_id(monitor) == monitor_id))
}

fn monitor_contains_point(monitor: &tauri::Monitor, x: i32, y: i32) -> bool {
    let monitor_x = monitor.position().x;
    let monitor_y = monitor.position().y;
    let monitor_right = monitor_x + monitor.size().width as i32;
    let monitor_bottom = monitor_y + monitor.size().height as i32;

    x >= monitor_x && x < monitor_right && y >= monitor_y && y < monitor_bottom
}

fn clamp_window_position_to_monitor(
    monitor: &tauri::Monitor,
    desired_x: i32,
    desired_y: i32,
    window_width: u32,
    window_height: u32,
) -> PhysicalPosition<i32> {
    let monitor_x = monitor.position().x;
    let monitor_y = monitor.position().y;
    let max_x = (monitor_x + monitor.size().width as i32 - window_width as i32).max(monitor_x);
    let max_y = (monitor_y + monitor.size().height as i32 - window_height as i32).max(monitor_y);

    PhysicalPosition::new(desired_x.clamp(monitor_x, max_x), desired_y.clamp(monitor_y, max_y))
}

fn resolve_main_window_position(
    app: &AppHandle,
    config: &BingoConfig,
    window_width: u32,
    window_height: u32,
) -> Result<Option<PhysicalPosition<i32>>, String> {
    let (Some(saved_x), Some(saved_y)) = (config.main_window_x, config.main_window_y) else {
        return Ok(None);
    };

    let monitors = app.available_monitors().map_err(|e| e.to_string())?;
    if monitors.is_empty() {
        return Ok(Some(PhysicalPosition::new(saved_x, saved_y)));
    }

    let primary_monitor_id = app
        .primary_monitor()
        .map_err(|e| e.to_string())?
        .map(|monitor| build_monitor_id(&monitor));

    let target_monitor = monitors
        .iter()
        .find(|monitor| monitor_contains_point(monitor, saved_x, saved_y))
        .or_else(|| {
            primary_monitor_id
                .as_ref()
                .and_then(|primary_id| monitors.iter().find(|monitor| build_monitor_id(monitor) == *primary_id))
        })
        .or_else(|| monitors.first());

    let Some(target_monitor) = target_monitor else {
        return Ok(Some(PhysicalPosition::new(saved_x, saved_y)));
    };

    Ok(Some(clamp_window_position_to_monitor(
        target_monitor,
        saved_x,
        saved_y,
        window_width,
        window_height,
    )))
}

fn save_main_window_position(app: &AppHandle, x: i32, y: i32) -> Result<(), String> {
    let path = get_config_path(app);
    let (mut config, _) = load_or_initialize_config(app)?;

    if config.main_window_x == Some(x) && config.main_window_y == Some(y) {
        return Ok(());
    }

    config.main_window_x = Some(x);
    config.main_window_y = Some(y);
    save_config_file(&path, &config)
}

fn persist_main_window_position(window: &tauri::Window) {
    if window.label() != "main" {
        return;
    }

    match window.outer_position() {
        Ok(position) => {
            if let Err(error) = save_main_window_position(&window.app_handle(), position.x, position.y) {
                log::warn!("メインウィンドウ位置の保存に失敗しました: {}", error);
            }
        }
        Err(error) => {
            log::warn!("メインウィンドウ位置の取得に失敗しました: {}", error);
        }
    }
}

fn resolve_effect_video_path(config: &BingoConfig, effect_type: &EffectType, require_enabled: bool) -> Option<PathBuf> {
    if require_enabled && !config.effect_enabled {
        return None;
    }

    let requested_enabled = match effect_type {
        EffectType::NormalBingo => config.normal_bingo_effect_enabled,
        EffectType::Special1 => config.special_1_effect_enabled,
        EffectType::Special25 => config.special_25_effect_enabled,
    };

    if require_enabled && !requested_enabled {
        return None;
    }

    let primary_candidate = match effect_type {
        EffectType::NormalBingo => resolve_video_path(&config.normal_bingo_video_path),
        EffectType::Special1 => resolve_video_path(&config.special_1_video_path),
        EffectType::Special25 => resolve_video_path(&config.special_25_video_path),
    };

    if primary_candidate.is_file() {
        return Some(primary_candidate);
    }

    if !matches!(effect_type, EffectType::NormalBingo) {
        let fallback = resolve_video_path(&config.normal_bingo_video_path);
        if fallback.is_file() {
            info!("専用エフェクト動画が見つからないため、通常ビンゴ動画へフォールバックします。対象: {:?}", effect_type);
            return Some(fallback);
        }
    }

    None
}

fn resolve_target_monitor(app: &AppHandle, config: &BingoConfig) -> Result<tauri::Monitor, String> {
    if config.effect_monitor_id.is_empty() {
        return app
            .primary_monitor()
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "プライマリモニタを取得できません。".to_string());
    }

    match find_monitor(app, &config.effect_monitor_id)? {
        Some(monitor) => Ok(monitor),
        None => {
            log::warn!("保存済みモニタが見つからないため、プライマリモニタへフォールバックします。");
            app.primary_monitor()
                .map_err(|e| e.to_string())?
                .ok_or_else(|| "プライマリモニタを取得できません。".to_string())
        }
    }
}

fn sync_effect_window_state(app: &AppHandle, config: &BingoConfig, visible: bool) -> Result<(), String> {
    let effect_window = app
        .get_webview_window("effect")
        .ok_or_else(|| "effect ウィンドウが見つかりません。".to_string())?;

    let target_monitor = resolve_target_monitor(app, config)?;

    effect_window
        .set_ignore_cursor_events(true)
        .map_err(|e| e.to_string())?;

    if !visible {
        let _ = effect_window.set_fullscreen(false);
        effect_window.hide().map_err(|e| e.to_string())?;
        return Ok(());
    }

    let _ = effect_window.set_fullscreen(false);
    effect_window
        .set_position(PhysicalPosition::new(target_monitor.position().x, target_monitor.position().y))
        .map_err(|e| e.to_string())?;
    effect_window
        .set_size(Size::Physical(PhysicalSize::new(
            target_monitor.size().width,
            target_monitor.size().height,
        )))
        .map_err(|e| e.to_string())?;

    effect_window.show().map_err(|e| e.to_string())?;
    effect_window.set_fullscreen(true).map_err(|e| e.to_string())?;

    Ok(())
}

fn play_effect_with_config(
    app: &AppHandle,
    config: &BingoConfig,
    effect_type: EffectType,
    require_enabled: bool,
) -> Result<bool, String> {
    let Some(video_path) = resolve_effect_video_path(config, &effect_type, require_enabled) else {
        let message = if require_enabled {
            "再生対象のエフェクト動画が見つからないため、演出をスキップします。"
        } else {
            "プレビュー対象の動画が見つからないため、再生をスキップします。"
        };
        info!("{}", message);
        return Ok(false);
    };

    sync_effect_window_state(app, config, true)?;

    let payload = EffectPayload {
        effect_type,
        video_path: video_path.to_string_lossy().into_owned(),
    };

    app.emit_to("effect", "play-bingo-effect", payload)
        .map_err(|e| e.to_string())?;

    Ok(true)
}

#[tauri::command]
fn sync_effect_window(app: AppHandle, config: BingoConfig, visible: bool) -> Result<(), String> {
    sync_effect_window_state(&app, &config, visible)
}

// --- Tauri コマンドの実装 ---

#[tauri::command]
fn log_action(trigger: String, message: String) {
    info!("[ACTION] ユーザー操作を受信しました: <{}>", trigger);
    info!("   └ [EVENT] {}", message);
}

#[tauri::command]
fn save_settings(app: AppHandle, config: BingoConfig) -> Result<(), String> {
    let path = get_config_path(&app);
    info!("設定の保存を開始します。パス: {:?}", path);

    save_config_file(&path, &config).map_err(|e| {
        log::error!("設定ファイルの書き込みに失敗しました ({:?}): {}", path, e);
        e
    })?;

    info!("設定の保存が正常に完了しました。");
    Ok(())
}

#[tauri::command]
fn load_settings(app: AppHandle) -> Result<BingoConfig, String> {
    let path = get_config_path(&app);
    info!("設定の読み込みを試行します。パス: {:?}", path);
    let (config, was_updated) = load_or_initialize_config(&app).map_err(|e| {
        log::error!("設定の読み込みに失敗しました: {}", e);
        e
    })?;

    if was_updated {
        info!("設定ファイルを検証し、不足項目を補完しました。");
    } else {
        info!("設定の読み込みに成功しました。");
    }

    Ok(config)
}

#[tauri::command]
fn save_session(
    app: AppHandle, 
    filename: Option<String>, 
    hits: Vec<i32>, 
    trigger: String 
) -> Result<String, String> {
    info!("[ACTION] ユーザー操作を受信しました: <{}>", trigger);
    let dir = get_adjacent_path(&app, "sessions");
    if !dir.exists() { let _ = fs::create_dir_all(&dir); }

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

    let last_num = hits.last().cloned().unwrap_or(0);
    info!("   └ [SAVE] セッションを更新: {} (最新: {}, 合計: {}件)", file_name, last_num, hits.len());
    Ok(file_name)
}

#[tauri::command]
fn get_sessions(app: AppHandle) -> Result<Vec<String>, String> {
    let dir = get_adjacent_path(&app, "sessions");
    if !dir.exists() { return Ok(vec![]); }
    let paths = fs::read_dir(&dir).map_err(|e| { log::error!("スキャン失敗: {}", e); e.to_string() })?;
    let mut files: Vec<String> = paths.filter_map(|p| p.ok()).map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|f| f.ends_with(".json")).collect();
    files.sort_by(|a, b| b.cmp(a));
    Ok(files)
}

#[tauri::command]
fn load_session(app: AppHandle, filename: String) -> Result<Vec<i32>, String> {
    info!("セッションの復元を開始します。対象ファイル: {}", filename);
    let mut path = get_adjacent_path(&app, "sessions");
    path.push(&filename);
    let content = fs::read_to_string(&path).map_err(|e| { log::error!("読み込み失敗: {}", e); e.to_string() })?;
    let session: BingoSession = serde_json::from_str(&content).map_err(|e| { log::error!("解析失敗: {}", e); e.to_string() })?;
    info!("セッションの復元に成功しました。ヒット番号: {:?}", session.hits);
    Ok(session.hits)
}

#[tauri::command]
fn list_effect_monitors(app: AppHandle) -> Result<Vec<MonitorInfo>, String> {
    let monitors = app.available_monitors().map_err(|e| e.to_string())?;
    let primary_id = app
        .primary_monitor()
        .map_err(|e| e.to_string())?
        .map(|monitor| build_monitor_id(&monitor));

    let items = monitors
        .into_iter()
        .enumerate()
        .map(|(index, monitor)| {
            let id = build_monitor_id(&monitor);
            let is_primary = primary_id.as_ref() == Some(&id);
            let name = monitor
                .name()
                .cloned()
                .unwrap_or_else(|| format!("Monitor {}", index + 1));
            let label = format!(
                "{}{} / {}x{} / {:.0}% / x={} y={}",
                if is_primary { "[Primary] " } else { "" },
                name,
                monitor.size().width,
                monitor.size().height,
                monitor.scale_factor() * 100.0,
                monitor.position().x,
                monitor.position().y
            );

            MonitorInfo { id, label }
        })
        .collect();

    Ok(items)
}

#[tauri::command]
fn play_bingo_effect(app: AppHandle, config: BingoConfig, effect_type: EffectType) -> Result<bool, String> {
    play_effect_with_config(&app, &config, effect_type, true)
}

#[tauri::command]
fn preview_bingo_effect(app: AppHandle, config: BingoConfig, effect_type: EffectType) -> Result<bool, String> {
    play_effect_with_config(&app, &config, effect_type, false)
}

#[tauri::command]
fn hide_effect_window(app: AppHandle) -> Result<(), String> {
    if let Some(effect_window) = app.get_webview_window("effect") {
        let _ = effect_window.set_ignore_cursor_events(true);
        let _ = effect_window.set_fullscreen(false);
        effect_window.hide().map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[tauri::command]
fn show_effect_window(app: AppHandle) -> Result<(), String> {
    let effect_window = app
        .get_webview_window("effect")
        .ok_or_else(|| "effect ウィンドウが見つかりません。".to_string())?;

    effect_window.show().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn exit_app(app: AppHandle) {
    info!("アプリケーションを終了します。");
    app.exit(0);
}

// --- メイン関数 ---

fn main() {
    let base_dir = get_base_dir();
    let log_dir = base_dir.join("logs"); // ログディレクトリの定義
    
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

    builder = builder.on_window_event(|window, event| {
        match event {
            WindowEvent::Moved(_) => {
                persist_main_window_position(window);
            }
            WindowEvent::CloseRequested { api, .. } => {
                if window.label() == "main" {
                    persist_main_window_position(window);
                }

                // メイン設定画面と effect 画面は閉じずに隠す
                if window.label() == "main" || window.label() == "effect" {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
            _ => {}
        }
    });

    // ロギング設定の構築
    let mut log_builder = tauri_plugin_log::Builder::new()
        // タイムゾーン設定（最新のAPI形式）
        .timezone_strategy(tauri_plugin_log::TimezoneStrategy::UseLocal)
        .targets([
            Target::new(TargetKind::Stdout),
            Target::new(TargetKind::Webview),
        ]);

    if has_write_permission {
        // 【修正】Folder ターゲットの型不整合を解消
        log_builder = log_builder.target(Target::new(TargetKind::Folder { 
            path: log_dir.clone(), 
            // String 型を Some() で包んで Option<String> に変換
            file_name: Some(log_filename.clone()) 
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

            // ログディレクトリの準備
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

            let (config, _) = load_or_initialize_config(&app_handle)?;
            if let Some(main_window) = app.get_webview_window("main") {
                let window_size = main_window
                    .outer_size()
                    .unwrap_or(PhysicalSize::new(MAIN_WINDOW_DEFAULT_WIDTH, MAIN_WINDOW_DEFAULT_HEIGHT));

                if let Some(position) = resolve_main_window_position(
                    &app_handle,
                    &config,
                    window_size.width,
                    window_size.height,
                )? {
                    main_window.set_position(position).map_err(|e| e.to_string())?;
                }
            }

            info!("Application started in portable mode. (Session ID: {})", session_id);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            save_settings,
            load_settings,
            save_session,
            get_sessions,
            load_session,
            list_effect_monitors,
            sync_effect_window,
            play_bingo_effect,
            preview_bingo_effect,
            show_effect_window,
            hide_effect_window,
            exit_app,
            log_action
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}