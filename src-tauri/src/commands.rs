use crate::network::NetworkInterface;
use crate::state::{AppState, ClipboardEntry, DeviceInfo, WsMessage};
use std::sync::Arc;
use tauri::{Manager, State};

/// 获取已发现的设备列表
#[tauri::command]
pub async fn get_devices(state: State<'_, Arc<AppState>>) -> Result<Vec<DeviceInfo>, String> {
    let devices = state.devices.lock().await;
    Ok(devices.values().cloned().collect())
}

/// 获取连接状态
#[tauri::command]
pub async fn get_status(state: State<'_, Arc<AppState>>) -> Result<StatusInfo, String> {
    let devices = state.devices.lock().await;
    let connected = devices
        .values()
        .filter(|d| d.status == crate::state::DeviceStatus::Connected)
        .count();

    let interfaces = crate::network::get_physical_interfaces();
    let ips: Vec<String> = interfaces.iter().map(|i| i.ip.clone()).collect();
    let interface_names: Vec<String> = interfaces.iter().map(|i| i.name.clone()).collect();

    Ok(StatusInfo {
        local_ips: ips,
        local_name: state.local_name.clone(),
        local_port: state.local_port,
        interface_names,
        connected_devices: connected,
        total_devices: devices.len(),
    })
}

/// 获取剪贴板历史记录
#[tauri::command]
pub async fn get_clipboard_history(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<ClipboardEntry>, String> {
    let history = state.clipboard_history.lock().await;
    Ok(history.clone())
}

/// 清空剪贴板历史
#[tauri::command]
pub async fn clear_history(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let mut history = state.clipboard_history.lock().await;
    history.clear();
    Ok(())
}

/// 获取所有可用网络接口
#[tauri::command]
pub fn get_interfaces() -> Result<Vec<NetworkInterface>, String> {
    Ok(crate::network::get_all_interfaces())
}

/// 请求下载文件（前端点击下载按钮时调用）
#[tauri::command]
pub async fn request_file_download(
    state: State<'_, Arc<AppState>>,
    transfer_id: String,
) -> Result<String, String> {
    log::info!("request_file_download: transfer_id={}", transfer_id);

    // 获取 pending transfer 信息
    let (sender_id, sender_ip, sender_port) = {
        let mut pending = state.pending_transfers.lock().await;
        match pending.get_mut(&transfer_id) {
            Some(pt) => {
                if pt.status == "downloading" {
                    return Err("该文件正在下载中".to_string());
                }
                if pt.status == "done" {
                    return Err("该文件已下载完成".to_string());
                }
                pt.status = "downloading".to_string();
                (pt.sender_id.clone(), pt.sender_ip.clone(), pt.sender_port)
            }
            None => {
                return Err("未找到对应的文件传输记录".to_string());
            }
        }
    };

    // 更新 clipboard_history 中的下载状态
    {
        let mut history = state.clipboard_history.lock().await;
        for entry in history.iter_mut() {
            if entry.transfer_id.as_deref() == Some(&transfer_id) {
                entry.download_status = Some("downloading".to_string());
                break;
            }
        }
    }

    // 构造 file-request 消息
    let msg = WsMessage {
        id: uuid::Uuid::new_v4().to_string(),
        msg_type: "file-request".to_string(),
        content_type: "file".to_string(),
        data: transfer_id.clone(),
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        sender_id: state.local_id.clone(),
        sender_name: state.local_name.clone(),
    };

    {
        let mut seen = state.seen_messages.lock().await;
        seen.insert(msg.id.clone());
    }

    let msg_json = serde_json::to_string(&msg).map_err(|e| e.to_string())?;

    // 优先使用持久连接发送 file-request
    let state_arc: Arc<AppState> = state.inner().clone();
    let sender_opt = {
        let mut peers = state_arc.peers.lock().await;
        peers.remove(&sender_id)
    };

    if let Some(mut sender) = sender_opt {
        use futures_util::SinkExt;
        match sender
            .send(tokio_tungstenite::tungstenite::Message::Text(
                msg_json.clone().into(),
            ))
            .await
        {
            Ok(_) => {
                let mut peers = state_arc.peers.lock().await;
                peers.insert(sender_id.clone(), sender);
                notify_clipboard_updated(&state_arc).await;
                log::info!("file-request sent via persistent connection to {}", sender_id);
                return Ok("下载请求已发送".to_string());
            }
            Err(e) => {
                log::warn!("Persistent sender failed for file-request: {}, trying fallback", e);
                // 标记离线让 connection_manager 重连
                let mut devices = state_arc.devices.lock().await;
                if let Some(d) = devices.get_mut(&sender_id) {
                    d.status = crate::state::DeviceStatus::Offline;
                }
            }
        }
    }

    // 回退：短连接
    if sender_ip.is_empty() || sender_port == 0 {
        return Err("无法获取发送端地址信息".to_string());
    }

    let addr = format!("ws://{}:{}", sender_ip, sender_port);
    log::info!("Sending file-request via short connection to {}", addr);

    match tokio::time::timeout(
        tokio::time::Duration::from_secs(10),
        tokio_tungstenite::connect_async(&addr),
    )
    .await
    {
        Ok(Ok((mut ws, _))) => {
            use futures_util::SinkExt;
            match ws
                .send(tokio_tungstenite::tungstenite::Message::Text(
                    msg_json.into(),
                ))
                .await
            {
                Ok(_) => {
                    let _ = ws.close(None).await;
                    notify_clipboard_updated(&state_arc).await;
                    log::info!("file-request sent via short connection to {}", addr);
                    Ok("下载请求已发送".to_string())
                }
                Err(e) => {
                    log::error!("Short connection send failed: {}", e);
                    Err(format!("发送下载请求失败: {}", e))
                }
            }
        }
        Ok(Err(e)) => {
            log::error!("Failed to connect to {}: {}", addr, e);
            Err(format!("无法连接到发送端: {}", e))
        }
        Err(_) => {
            log::error!("Connection timeout to {}", addr);
            Err(format!("连接发送端超时: {}", addr))
        }
    }
}

/// 通知前端剪贴板历史已更新
async fn notify_clipboard_updated(state: &AppState) {
    use tauri::Emitter;
    let handle = state.app_handle.lock().await;
    if let Some(app) = handle.as_ref() {
        let _ = app.emit("clipboard-received", serde_json::json!({}));
    }
}

/// 在资源管理器/Finder中打开父目录并选中目标（文件或文件夹）
#[tauri::command]
pub async fn open_file_location(file_path: String) -> Result<(), String> {
    log::info!("open_file_location: {}", file_path);

    let path = std::path::Path::new(&file_path);
    if !path.exists() {
        return Err(format!("文件不存在: {}", file_path));
    }

    #[cfg(target_os = "macos")]
    {
        // -R 总是在父目录中选中目标（无论是文件还是文件夹）
        let status = std::process::Command::new("open")
            .args(["-R", &file_path])
            .status()
            .map_err(|e| format!("打开 Finder 失败: {}", e))?;
        if status.success() {
            return Ok(());
        }
        return Err("打开 Finder 失败".to_string());
    }

    #[cfg(target_os = "windows")]
    {
        // 使用 raw_arg 避免 Rust 对参数加引号（explorer.exe 对引号内的 /select, 解析不正确）
        use std::os::windows::process::CommandExt;
        let raw_arg = format!("/select,\"{}\"", file_path);
        log::info!("explorer raw_arg: {}", raw_arg);
        let status = std::process::Command::new("explorer")
            .raw_arg(&raw_arg)
            .status()
            .map_err(|e| format!("打开资源管理器失败: {}", e))?;
        if status.success() {
            return Ok(());
        }
        return Err("打开资源管理器失败".to_string());
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Err("不支持的操作系统".to_string())
    }
}

#[derive(serde::Serialize)]
pub struct StatusInfo {
    pub local_ips: Vec<String>,
    pub local_name: String,
    pub local_port: u16,
    pub interface_names: Vec<String>,
    pub connected_devices: usize,
    pub total_devices: usize,
}

#[derive(serde::Serialize)]
pub struct SettingsInfo {
    pub download_dir: String,
    pub cache_size: u64,
    pub shortcut: String,
    pub autostart: bool,
}

/// 获取应用设置
#[tauri::command]
pub async fn get_settings(state: State<'_, Arc<AppState>>) -> Result<SettingsInfo, String> {
    let download_dir = state.download_dir.lock().await.clone();
    let cache_size = crate::settings::get_cache_size_bytes();
    let shortcut = state.shortcut.lock().await.clone();
    let autostart = *state.autostart.lock().await;
    Ok(SettingsInfo {
        download_dir,
        cache_size,
        shortcut,
        autostart,
    })
}

/// 设置文件下载目录
#[tauri::command]
pub async fn set_download_dir(
    state: State<'_, Arc<AppState>>,
    path: String,
) -> Result<(), String> {
    // 验证目录是否存在
    let dir = std::path::Path::new(&path);
    if !dir.exists() {
        std::fs::create_dir_all(dir).map_err(|e| format!("创建目录失败: {}", e))?;
    }
    if !dir.is_dir() {
        return Err("指定路径不是目录".to_string());
    }

    // 更新内存中的设置
    {
        let mut download_dir = state.download_dir.lock().await;
        *download_dir = path.clone();
    }

    // 持久化保存
    let shortcut = state.shortcut.lock().await.clone();
    let autostart = *state.autostart.lock().await;
    let settings = crate::settings::AppSettings {
        download_dir: path,
        shortcut,
        autostart,
    };
    crate::settings::save_settings(&settings)?;
    Ok(())
}

/// 清除缓存
#[tauri::command]
pub async fn clear_cache(
    state: State<'_, Arc<AppState>>,
) -> Result<u64, String> {
    // 清空 pending transfers
    {
        let mut pending = state.pending_transfers.lock().await;
        pending.clear();
    }
    crate::settings::clear_cache_files()
}

/// 获取缓存大小
#[tauri::command]
pub fn get_cache_size() -> Result<u64, String> {
    Ok(crate::settings::get_cache_size_bytes())
}

/// 断开同步（暂停广播、监听、连接）
#[tauri::command]
pub async fn disconnect_sync(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    use std::sync::atomic::Ordering;
    state.is_paused.store(true, Ordering::SeqCst);

    // 断开所有现有 peer 连接
    {
        let mut peers = state.peers.lock().await;
        peers.clear();
    }

    // 将所有设备标记为 Offline
    {
        let mut devices = state.devices.lock().await;
        for device in devices.values_mut() {
            device.status = crate::state::DeviceStatus::Offline;
        }
    }

    // 通知前端
    {
        use tauri::Emitter;
        let handle = state.app_handle.lock().await;
        if let Some(app) = handle.as_ref() {
            let devices: Vec<crate::state::DeviceInfo> =
                state.devices.lock().await.values().cloned().collect();
            let _ = app.emit("devices-changed", &devices);
            let _ = app.emit("sync-status-changed", false);
        }
    }

    log::info!("Sync disconnected (paused)");
    Ok(())
}

/// 恢复同步（重新开始广播、监听、连接）
#[tauri::command]
pub async fn connect_sync(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    use std::sync::atomic::Ordering;
    state.is_paused.store(false, Ordering::SeqCst);

    // 通知前端
    {
        use tauri::Emitter;
        let handle = state.app_handle.lock().await;
        if let Some(app) = handle.as_ref() {
            let _ = app.emit("sync-status-changed", true);
        }
    }

    log::info!("Sync reconnected (resumed)");
    Ok(())
}

/// 获取同步状态（是否已连接/未暂停）
#[tauri::command]
pub async fn get_sync_status(state: State<'_, Arc<AppState>>) -> Result<bool, String> {
    use std::sync::atomic::Ordering;
    Ok(!state.is_paused.load(Ordering::SeqCst))
}

/// 设置全局快捷键
#[tauri::command]
pub async fn set_shortcut(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    shortcut: String,
) -> Result<(), String> {
    use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

    // 解除旧快捷键
    let old_shortcut = state.shortcut.lock().await.clone();
    if !old_shortcut.is_empty() {
        let _ = app.global_shortcut().unregister(old_shortcut.as_str());
    }

    // 注册新快捷键
    if !shortcut.is_empty() {
        let app_clone = app.clone();
        app.global_shortcut()
            .on_shortcut(shortcut.as_str(), move |_app, _shortcut, event| {
                if event.state() != ShortcutState::Pressed {
                    return;
                }
                if let Some(window) = app_clone.get_webview_window("main") {
                    if window.is_visible().unwrap_or(false) && window.is_focused().unwrap_or(false) {
                        let _ = window.hide();
                    } else {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
            })
            .map_err(|e| format!("注册快捷键失败: {}", e))?;
    }

    // 保存到内存和文件
    {
        let mut s = state.shortcut.lock().await;
        *s = shortcut.clone();
    }
    let download_dir = state.download_dir.lock().await.clone();
    let autostart = *state.autostart.lock().await;
    let settings = crate::settings::AppSettings {
        download_dir,
        shortcut,
        autostart,
    };
    crate::settings::save_settings(&settings)?;
    Ok(())
}

/// 设置开机自动启动
#[tauri::command]
pub async fn set_autostart(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    enabled: bool,
) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;

    let autostart_manager = app.autolaunch();
    if enabled {
        autostart_manager
            .enable()
            .map_err(|e| format!("启用自动启动失败: {}", e))?;
        log::info!("Autostart enabled");
    } else {
        autostart_manager
            .disable()
            .map_err(|e| format!("禁用自动启动失败: {}", e))?;
        log::info!("Autostart disabled");
    }

    // 保存到内存和文件
    {
        let mut a = state.autostart.lock().await;
        *a = enabled;
    }
    let download_dir = state.download_dir.lock().await.clone();
    let shortcut = state.shortcut.lock().await.clone();
    let settings = crate::settings::AppSettings {
        download_dir,
        shortcut,
        autostart: enabled,
    };
    crate::settings::save_settings(&settings)?;
    Ok(())
}

/// 打开日志文件目录
#[tauri::command]
pub async fn open_log_dir() -> Result<(), String> {
    let dir = crate::logger::log_dir();
    if !dir.exists() {
        std::fs::create_dir_all(&dir).map_err(|e| format!("创建日志目录失败: {}", e))?;
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(dir.to_string_lossy().to_string())
            .status()
            .map_err(|e| format!("打开日志目录失败: {}", e))?;
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(dir.to_string_lossy().to_string())
            .status()
            .map_err(|e| format!("打开日志目录失败: {}", e))?;
    }

    log::info!("Opened log directory: {:?}", dir);
    Ok(())
}

/// 获取当前版本号
#[tauri::command]
pub fn get_version() -> Result<String, String> {
    Ok(env!("CARGO_PKG_VERSION").to_string())
}
