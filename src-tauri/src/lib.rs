mod clipboard;
mod commands;
mod discovery;
mod logger;
mod network;
pub mod settings;
mod state;
mod transport;
mod tray;

use state::AppState;
use std::sync::Arc;
use tauri::{Manager, WindowEvent};
use tauri_plugin_autostart::MacosLauncher;
use tauri_plugin_global_shortcut::GlobalShortcutExt;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 初始化文件日志（生产环境写入日志文件，开发环境同时输出到控制台）
    logger::init_logger();

    let app_state = Arc::new(AppState::new());

    // 在构建器之前读取快捷键设置，避免在 setup 中使用 block_on
    let initial_shortcut = settings::load_settings().shortcut;

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None,
        ))
        .manage(app_state.clone())
        .setup(move |app| {
            let app_handle = app.handle().clone();
            let state = app.state::<Arc<AppState>>().inner().clone();

            // 保存 AppHandle
            {
                let handle = app_handle.clone();
                let state_for_handle = state.clone();
                tauri::async_runtime::spawn(async move {
                    let mut h = state_for_handle.app_handle.lock().await;
                    *h = Some(handle);
                });
            }

            // 设置系统托盘
            tray::setup_tray(&app_handle).ok();

            // 启动设备发现
            let state_clone = state.clone();
            tauri::async_runtime::spawn(async move {
                discovery::start_discovery(state_clone).await;
            });

            // 启动 WebSocket 传输
            let state_clone = state.clone();
            tauri::async_runtime::spawn(async move {
                transport::start_transport(state_clone).await;
            });

            // 启动剪贴板监听
            let state_clone = state.clone();
            tauri::async_runtime::spawn(async move {
                clipboard::start_clipboard_monitor(state_clone).await;
            });

            // 注册全局快捷键
            {
                let shortcut_str = initial_shortcut;
                if !shortcut_str.is_empty() {
                    let app_for_shortcut = app_handle.clone();
                    tauri::async_runtime::spawn(async move {
                        let app_for_callback = app_for_shortcut.clone();
                        if let Err(e) = app_for_shortcut.global_shortcut().on_shortcut(
                            shortcut_str.as_str(),
                            move |_app, _event, _shortcut| {
                                if let Some(window) = app_for_callback.get_webview_window("main") {
                                    if window.is_visible().unwrap_or(false) {
                                        let _ = window.hide();
                                    } else {
                                        let _ = window.show();
                                        let _ = window.set_focus();
                                    }
                                }
                            },
                        ) {
                            log::warn!("Failed to register global shortcut '{}': {}", shortcut_str, e);
                        } else {
                            log::info!("Global shortcut registered: {}", shortcut_str);
                        }
                    });
                }
            }

            log::info!("ClipSync started: {}", state.local_name);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_devices,
            commands::get_status,
            commands::get_clipboard_history,
            commands::clear_history,
            commands::get_interfaces,
            commands::request_file_download,
            commands::open_file_location,
            commands::get_settings,
            commands::set_download_dir,
            commands::clear_cache,
            commands::get_cache_size,
            commands::disconnect_sync,
            commands::connect_sync,
            commands::get_sync_status,
            commands::set_shortcut,
            commands::set_autostart,
            commands::open_log_dir,
            commands::get_version,
        ])
        .on_window_event(|window, event| {
            match event {
                WindowEvent::CloseRequested { api, .. } => {
                    // 拦截关闭事件，隐藏窗口而非退出
                    let _ = window.hide();
                    api.prevent_close();
                }
                WindowEvent::Destroyed => {
                    // 窗口真正销毁时才退出
                }
                _ => {}
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
