use crate::state::{
    AppState, ClipboardEntry, DeviceInfo, DeviceStatus, FileStartData,
    PendingTransfer, WsMessage,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use futures_util::{SinkExt, StreamExt};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::Emitter;
use tokio::net::TcpListener;
use tokio::time::{interval, sleep, Duration};
use tokio_tungstenite::tungstenite::Message;

const CHUNK_SIZE: usize = 64 * 1024; // 64KB per chunk

/// 启动 WebSocket 传输服务
pub async fn start_transport(state: Arc<AppState>) {
    // WebSocket Server
    let state_server = state.clone();
    tokio::spawn(async move {
        ws_server(state_server).await;
    });

    // 连接管理器
    let state_mgr = state.clone();
    tokio::spawn(async move {
        connection_manager(state_mgr).await;
    });
}

/// WebSocket 服务端
async fn ws_server(state: Arc<AppState>) {
    let listener = match TcpListener::bind(format!("0.0.0.0:{}", state.local_port)).await {
        Ok(l) => l,
        Err(e) => {
            log::error!(
                "Failed to bind WebSocket server on port {}: {}",
                state.local_port,
                e
            );
            return;
        }
    };

    log::info!("WebSocket server listening on port {}", state.local_port);

    loop {
        match listener.accept().await {
            Ok((stream, addr)) => {
                log::debug!("Incoming WebSocket from {}", addr);
                let state = state.clone();
                tokio::spawn(async move {
                    let ws = match tokio_tungstenite::accept_async(stream).await {
                        Ok(ws) => ws,
                        Err(e) => {
                            log::warn!("WebSocket accept failed: {}", e);
                            return;
                        }
                    };

                    let (_sender, mut receiver) = ws.split();
                    let mut peer_id: Option<String> = None;
                    let mut is_file_transfer_only = false;

                    while let Some(msg) = receiver.next().await {
                        match msg {
                            Ok(Message::Text(text)) => {
                                if let Ok(ws_msg) =
                                    serde_json::from_str::<WsMessage>(&text)
                                {
                                    if peer_id.is_none() {
                                        peer_id = Some(ws_msg.sender_id.clone());
                                        log::info!(
                                            "Server: peer connected {} ({})",
                                            ws_msg.sender_name,
                                            ws_msg.sender_id
                                        );

                                        let mut devices = state.devices.lock().await;
                                        if let Some(d) =
                                            devices.get_mut(&ws_msg.sender_id)
                                        {
                                            d.status = DeviceStatus::Connected;
                                        }
                                    }

                                    if matches!(
                                        ws_msg.msg_type.as_str(),
                                        "file-chunk" | "file-end"
                                    ) {
                                        is_file_transfer_only = true;
                                    }

                                    handle_ws_message(&state, &ws_msg).await;
                                }
                            }
                            Ok(Message::Close(_)) => break,
                            Err(e) => {
                                log::warn!("WebSocket recv error: {}", e);
                                break;
                            }
                            _ => {}
                        }
                    }

                    // 断开：仅文件传输连接不改变设备状态
                    if let Some(id) = peer_id {
                        if !is_file_transfer_only {
                            log::info!("Server: peer disconnected {}", id);
                            let mut devices = state.devices.lock().await;
                            if let Some(d) = devices.get_mut(&id) {
                                d.status = DeviceStatus::Offline;
                            }
                            drop(devices);
                            notify_devices_changed(&state).await;
                        }
                    }
                });
            }
            Err(e) => {
                log::warn!("Accept failed: {}", e);
            }
        }
    }
}

/// 连接管理器 - 定期尝试连接离线设备
async fn connection_manager(state: Arc<AppState>) {
    let mut ticker = interval(Duration::from_secs(5));
    let mut connecting: HashSet<String> = HashSet::new();

    loop {
        ticker.tick().await;

        // 如果同步已暂停，跳过连接尝试
        if state.is_paused.load(std::sync::atomic::Ordering::SeqCst) {
            continue;
        }

        let devices = state.devices.lock().await;
        let to_connect: Vec<DeviceInfo> = devices
            .values()
            .filter(|d| {
                !connecting.contains(&d.id)
                    && (d.status == DeviceStatus::Offline
                        || d.status == DeviceStatus::Connecting)
            })
            .cloned()
            .collect();
        drop(devices);

        for device in to_connect {
            connecting.insert(device.id.clone());
            let state = state.clone();
            tokio::spawn(async move {
                connect_to_peer(state, &device).await;
            });
        }
    }
}

/// 主动连接到某个设备
async fn connect_to_peer(state: Arc<AppState>, device: &DeviceInfo) {
    let addr = format!("ws://{}:{}", device.ip, device.port);
    log::debug!("Connecting to {} at {}", device.name, addr);

    {
        let mut devices = state.devices.lock().await;
        if let Some(d) = devices.get_mut(&device.id) {
            d.status = DeviceStatus::Connecting;
        }
    }

    let result = tokio::time::timeout(
        Duration::from_secs(5),
        tokio_tungstenite::connect_async(&addr),
    )
    .await;

    match result {
        Ok(Ok((ws, _))) => {
            log::info!("Client: connected to {}", device.name);
            {
                let mut devices = state.devices.lock().await;
                if let Some(d) = devices.get_mut(&device.id) {
                    d.status = DeviceStatus::Connected;
                }
            }
            notify_devices_changed(&state).await;

            let (sender, mut receiver) = ws.split();

            // 存储 sender 到 peers
            {
                let mut peers = state.peers.lock().await;
                peers.insert(device.id.clone(), sender);
            }

            while let Some(msg) = receiver.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        if let Ok(ws_msg) = serde_json::from_str::<WsMessage>(&text) {
                            handle_ws_message(&state, &ws_msg).await;
                        }
                    }
                    Ok(Message::Close(_)) => break,
                    Err(e) => {
                        log::debug!("Error from {}: {}", device.name, e);
                        break;
                    }
                    _ => {}
                }
            }

            // 清理 peers 映射
            {
                let mut peers = state.peers.lock().await;
                peers.remove(&device.id);
            }

            {
                let mut devices = state.devices.lock().await;
                if let Some(d) = devices.get_mut(&device.id) {
                    d.status = DeviceStatus::Offline;
                }
            }
            notify_devices_changed(&state).await;
        }
        Ok(Err(e)) => {
            log::debug!("Connect failed to {}: {}", device.name, e);
            let mut devices = state.devices.lock().await;
            if let Some(d) = devices.get_mut(&device.id) {
                d.status = DeviceStatus::Offline;
            }
        }
        Err(_) => {
            log::debug!("Connect timeout to {}", device.name);
            let mut devices = state.devices.lock().await;
            if let Some(d) = devices.get_mut(&device.id) {
                d.status = DeviceStatus::Offline;
            }
        }
    }
}

// ========================== 统一发送机制 ==========================

/// 向指定设备发送消息：优先使用持久连接，失败则回退短连接
/// 持久连接失败时标记设备为 Offline，让 connection_manager 自动重连
async fn send_to_peer(state: &Arc<AppState>, device: &DeviceInfo, msg_json: &str) {
    // 1. 尝试使用持久连接（connect_to_peer 建立的 client-side sender）
    let sender_opt = {
        let mut peers = state.peers.lock().await;
        peers.remove(&device.id)
    };

    if let Some(mut sender) = sender_opt {
        match sender.send(Message::Text(msg_json.to_string().into())).await {
            Ok(_) => {
                // 发送成功，归还 sender
                let mut peers = state.peers.lock().await;
                peers.insert(device.id.clone(), sender);
                return;
            }
            Err(e) => {
                log::warn!(
                    "Persistent sender failed for {}, marking offline: {}",
                    device.name,
                    e
                );
                // sender 已消费且失败，不归还
                // 标记设备离线，connection_manager 会自动重连
                let mut devices = state.devices.lock().await;
                if let Some(d) = devices.get_mut(&device.id) {
                    d.status = DeviceStatus::Offline;
                }
            }
        }
    }

    // 2. 回退：创建短连接发送
    let addr = format!("ws://{}:{}", device.ip, device.port);
    match tokio::time::timeout(
        Duration::from_secs(5),
        tokio_tungstenite::connect_async(&addr),
    )
    .await
    {
        Ok(Ok((mut ws, _))) => {
            match ws.send(Message::Text(msg_json.to_string().into())).await {
                Ok(_) => {
                    log::debug!("Short-connection send OK to {}", device.name);
                }
                Err(e) => {
                    log::warn!("Short-connection send failed to {}: {}", device.name, e);
                }
            }
            let _ = ws.close(None).await;
        }
        Ok(Err(e)) => {
            log::warn!("Short-connection to {} failed: {}", device.name, e);
        }
        Err(_) => {
            log::warn!("Short-connection to {} timed out", device.name);
        }
    }
}

/// 广播消息到所有在线设备
async fn broadcast_to_all(state: Arc<AppState>, msg_json: String) {
    // 如果同步已暂停，不广播
    if state.is_paused.load(std::sync::atomic::Ordering::SeqCst) {
        log::debug!("Sync paused, skipping broadcast");
        return;
    }

    let devices: Vec<DeviceInfo> = {
        state
            .devices
            .lock()
            .await
            .values()
            .filter(|d| d.status != DeviceStatus::Offline)
            .cloned()
            .collect()
    };

    log::info!(
        "Broadcasting to {} device(s)",
        devices.len()
    );

    for device in devices {
        let state = state.clone();
        let json = msg_json.clone();
        tokio::spawn(async move {
            send_to_peer(&state, &device, &json).await;
        });
    }
}

// ========================== 广播函数 ==========================

/// 广播图片剪贴板到所有已连接设备
pub async fn broadcast_clipboard_image(state: Arc<AppState>, png_bytes: Vec<u8>) {
    let png_b64 = BASE64.encode(&png_bytes);

    let msg = WsMessage {
        id: uuid::Uuid::new_v4().to_string(),
        msg_type: "clipboard".to_string(),
        content_type: "image".to_string(),
        data: png_b64,
        timestamp: current_timestamp(),
        sender_id: state.local_id.clone(),
        sender_name: state.local_name.clone(),
    };

    {
        let mut seen = state.seen_messages.lock().await;
        seen.insert(msg.id.clone());
    }

    let entry = ClipboardEntry::new_image(
        msg.id.clone(),
        msg.timestamp,
        "本机".to_string(),
        png_bytes.len() as u64,
    );
    {
        let mut history = state.clipboard_history.lock().await;
        history.insert(0, entry);
        if history.len() > 20 {
            history.truncate(20);
        }
    }

    let msg_json = match serde_json::to_string(&msg) {
        Ok(j) => j,
        Err(_) => return,
    };

    log::info!("Broadcasting image: {} bytes PNG", png_bytes.len());

    broadcast_to_all(state.clone(), msg_json).await;

    notify_clipboard_received(&state, &msg).await;
}

/// 广播文本剪贴板到所有已连接设备
pub async fn broadcast_clipboard(state: Arc<AppState>, text: String) {
    let msg = WsMessage {
        id: uuid::Uuid::new_v4().to_string(),
        msg_type: "clipboard".to_string(),
        content_type: "text".to_string(),
        data: text.clone(),
        timestamp: current_timestamp(),
        sender_id: state.local_id.clone(),
        sender_name: state.local_name.clone(),
    };

    {
        let mut seen = state.seen_messages.lock().await;
        seen.insert(msg.id.clone());
    }

    let entry = ClipboardEntry::new_text(
        msg.id.clone(),
        text.clone(),
        msg.timestamp,
        "本机".to_string(),
    );
    {
        let mut history = state.clipboard_history.lock().await;
        history.insert(0, entry);
        if history.len() > 20 {
            history.truncate(20);
        }
    }

    let msg_json = match serde_json::to_string(&msg) {
        Ok(j) => j,
        Err(_) => return,
    };

    broadcast_to_all(state.clone(), msg_json).await;

    // 通知前端刷新历史列表（让本机文字记录立刻显示）
    // 使用 spawn 避免阻塞 monitor_loop
    tokio::spawn(async move {
        let handle = state.app_handle.lock().await;
        if let Some(app) = handle.as_ref() {
            let _ = app.emit("clipboard-received", &msg);
        }
    });
}

/// 广播文件元信息（file-start）到所有设备（支持批量）
pub async fn broadcast_file_metadata(state: Arc<AppState>, file_data: FileStartData) {
    let data_json = match serde_json::to_string(&file_data) {
        Ok(j) => j,
        Err(e) => {
            log::error!("Failed to serialize file start data: {}", e);
            return;
        }
    };

    let msg = WsMessage {
        id: uuid::Uuid::new_v4().to_string(),
        msg_type: "file-start".to_string(),
        content_type: "file".to_string(),
        data: data_json,
        timestamp: current_timestamp(),
        sender_id: state.local_id.clone(),
        sender_name: state.local_name.clone(),
    };

    {
        let mut seen = state.seen_messages.lock().await;
        seen.insert(msg.id.clone());
    }

    // 本地历史记录（发送端）- 显示为一条
    let entry = ClipboardEntry::new_file(
        msg.id.clone(),
        file_data.display_name.clone(),
        msg.timestamp,
        "本机".to_string(),
        file_data.total_size,
        None,
        None,
        Some(file_data.transfer_id.clone()),
    );
    {
        let mut history = state.clipboard_history.lock().await;
        history.insert(0, entry);
        if history.len() > 20 {
            history.truncate(20);
        }
    }

    let msg_json = match serde_json::to_string(&msg) {
        Ok(j) => j,
        Err(_) => return,
    };

    log::info!(
        "Broadcasting file-start: {} ({} files, {} bytes), transfer_id={}",
        file_data.display_name,
        file_data.files.len(),
        file_data.total_size,
        file_data.transfer_id
    );

    broadcast_to_all(state.clone(), msg_json).await;

    notify_clipboard_received(&state, &msg).await;
}

// ========================== 消息处理 ==========================

/// 处理收到的 WebSocket 消息
async fn handle_ws_message(state: &AppState, msg: &WsMessage) {
    // 消息去重（msg_id 级别）
    {
        let mut seen = state.seen_messages.lock().await;
        if seen.contains(&msg.id) {
            return;
        }
        seen.insert(msg.id.clone());
        if seen.len() > 1000 {
            let to_remove: Vec<String> = seen.iter().take(500).cloned().collect();
            for id in to_remove {
                seen.remove(&id);
            }
        }
    }

    log::info!(
        "Processing msg: type={}, content={}, from={}",
        msg.msg_type,
        msg.content_type,
        msg.sender_name
    );

    match msg.msg_type.as_str() {
        "clipboard" if msg.content_type == "text" => {
            handle_clipboard_text(state, msg).await;
        }
        "clipboard" if msg.content_type == "image" => {
            handle_clipboard_image(state, msg).await;
        }
        "file-start" => {
            handle_file_start(state, msg).await;
        }
        "file-request" => {
            handle_file_request(state, msg).await;
        }
        "file-chunk" => {
            handle_file_chunk(state, msg).await;
        }
        "file-end" => {
            handle_file_end(state, msg).await;
        }
        _ => {
            log::debug!("Unknown message type: {}", msg.msg_type);
        }
    }
}

/// 处理图片剪贴板消息
async fn handle_clipboard_image(state: &AppState, msg: &WsMessage) {
    // 不处理自己发的消息
    if msg.sender_id == state.local_id {
        return;
    }

    // 解码 base64 → PNG 字节
    let png_bytes = match BASE64.decode(msg.data.as_bytes()) {
        Ok(b) => b,
        Err(e) => {
            log::warn!("Failed to decode image base64: {}", e);
            return;
        }
    };

    // 设置远程图片哈希，防止 monitor 回弹广播
    {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        use std::sync::atomic::Ordering;
        let mut hasher = DefaultHasher::new();
        png_bytes.hash(&mut hasher);
        let hash = hasher.finish();
        state.last_remote_image_hash.store(hash, Ordering::SeqCst);
    }

    // 解码 PNG → RGBA
    let img = match image::load_from_memory(&png_bytes) {
        Ok(img) => img,
        Err(e) => {
            log::warn!("Failed to decode PNG: {}", e);
            return;
        }
    };
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    let pixels = rgba.into_raw();

    // 使用 guard 确保 is_writing_clipboard 始终被重置
    struct WritingGuard<'a>(&'a AppState);
    impl<'a> Drop for WritingGuard<'a> {
        fn drop(&mut self) {
            use std::sync::atomic::Ordering;
            self.0.is_writing_clipboard.store(false, Ordering::SeqCst);
        }
    }

    // 获取剪贴板写入互斥锁，防止并发操作损坏系统剪贴板
    let _write_guard = state.clipboard_write_lock.lock().await;

    {
        use std::sync::atomic::Ordering;
        state.is_writing_clipboard.store(true, Ordering::SeqCst);
    }
    let _flag_guard = WritingGuard(state);

    // 写入图片到剪贴板
    {
        use std::borrow::Cow;
        let img_data = arboard::ImageData {
            width: width as usize,
            height: height as usize,
            bytes: Cow::Owned(pixels),
        };
        if let Ok(mut clipboard) = arboard::Clipboard::new() {
            if let Err(e) = clipboard.set_image(img_data) {
                log::warn!("Failed to write image to clipboard: {}", e);
            }
        }
    }

    sleep(Duration::from_millis(500)).await;

    let entry = ClipboardEntry::new_image(
        msg.id.clone(),
        msg.timestamp,
        msg.sender_name.clone(),
        png_bytes.len() as u64,
    );
    {
        let mut history = state.clipboard_history.lock().await;
        history.insert(0, entry);
        if history.len() > 20 {
            history.truncate(20);
        }
    }

    notify_clipboard_received(state, msg).await;
}

/// 处理文本剪贴板消息
async fn handle_clipboard_text(state: &AppState, msg: &WsMessage) {
    // 不处理自己发的消息
    if msg.sender_id == state.local_id {
        return;
    }

    // 设置远程文本哈希，防止 monitor 回弹广播
    {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        use std::sync::atomic::Ordering;
        let mut hasher = DefaultHasher::new();
        msg.data.hash(&mut hasher);
        let hash = hasher.finish();
        state.last_remote_text_hash.store(hash, Ordering::SeqCst);
    }

    // 使用 guard 确保 is_writing_clipboard 始终被重置
    struct WritingGuard<'a>(&'a AppState);
    impl<'a> Drop for WritingGuard<'a> {
        fn drop(&mut self) {
            use std::sync::atomic::Ordering;
            self.0.is_writing_clipboard.store(false, Ordering::SeqCst);
        }
    }

    // 获取剪贴板写入互斥锁，防止并发操作损坏系统剪贴板
    let _write_guard = state.clipboard_write_lock.lock().await;

    {
        use std::sync::atomic::Ordering;
        state.is_writing_clipboard.store(true, Ordering::SeqCst);
    }
    let _flag_guard = WritingGuard(state);

    if let Ok(mut clipboard) = arboard::Clipboard::new() {
        if let Err(e) = clipboard.set_text(&msg.data) {
            log::warn!("Failed to write clipboard: {}", e);
        }
    }

    sleep(Duration::from_millis(500)).await;
    // _flag_guard 和 _write_guard 会在函数结束时自动释放

    let entry = ClipboardEntry::new_text(
        msg.id.clone(),
        msg.data.clone(),
        msg.timestamp,
        msg.sender_name.clone(),
    );
    {
        let mut history = state.clipboard_history.lock().await;
        history.insert(0, entry);
        if history.len() > 20 {
            history.truncate(20);
        }
    }

    notify_clipboard_received(state, msg).await;
}

/// 处理 file-start：接收端创建临时目录并记录批量传输信息
async fn handle_file_start(state: &AppState, msg: &WsMessage) {
    // 不处理自己发的消息
    if msg.sender_id == state.local_id {
        log::debug!("Skipping file-start from self");
        return;
    }

    let file_data: FileStartData = match serde_json::from_str(&msg.data) {
        Ok(d) => d,
        Err(e) => {
            log::warn!("Failed to parse file-start data: {}", e);
            return;
        }
    };

    // 去重：检查 transfer_id 是否已存在
    {
        let pending = state.pending_transfers.lock().await;
        if pending.contains_key(&file_data.transfer_id) {
            log::info!(
                "Skipping duplicate file-start: transfer_id={} already exists",
                file_data.transfer_id
            );
            return;
        }
    }

    log::info!(
        "Received file-start: {} ({} files, {} bytes) from {}, transfer_id={}",
        file_data.display_name,
        file_data.files.len(),
        file_data.total_size,
        msg.sender_name,
        file_data.transfer_id
    );

    // 创建临时目录（以 transfer_id 命名，避免冲突）
    let temp_dir = std::env::temp_dir()
        .join("clipsync")
        .join(&file_data.transfer_id);
    let _ = std::fs::create_dir_all(&temp_dir);
    let temp_dir_str = temp_dir.to_string_lossy().to_string();

    // 只有单文件传输时才写入占位到剪贴板（多文件/文件夹不写，避免触发 monitor 重复广播）
    let is_single_file = file_data.files.len() == 1 && !file_data.files[0].relative_path.contains('/');

    if is_single_file {
        let placeholder_file = if let Some(first_file) = file_data.files.first() {
            let p = temp_dir.join(&first_file.relative_path);
            if let Some(parent) = p.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(&p, b"");
            p.to_string_lossy().to_string()
        } else {
            temp_dir_str.clone()
        };

        // 获取剪贴板写入互斥锁
        let _write_guard = state.clipboard_write_lock.lock().await;

        {
            use std::sync::atomic::Ordering;
            state.is_writing_clipboard.store(true, Ordering::SeqCst);
        }

        let _ = crate::clipboard::set_clipboard_files(&[placeholder_file.clone()]).await;

        // 设置占位文件哈希，防止 monitor 重复检测（必须在 is_writing_clipboard=false 之前设置！）
        {
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};
            use std::sync::atomic::Ordering;
            let mut hasher = DefaultHasher::new();
            let lower = placeholder_file.to_lowercase();
            vec![lower].hash(&mut hasher);
            let hash = hasher.finish();
            state.written_placeholder_hash.store(hash, Ordering::SeqCst);
        }

        sleep(Duration::from_millis(500)).await;
        {
            use std::sync::atomic::Ordering;
            state.is_writing_clipboard.store(false, Ordering::SeqCst);
        }
        drop(_write_guard);
    }

    // 添加到历史记录（显示为一条）
    let entry = ClipboardEntry::new_file(
        msg.id.clone(),
        file_data.display_name.clone(),
        msg.timestamp,
        msg.sender_name.clone(),
        file_data.total_size,
        None,
        Some("pending".to_string()),
        Some(file_data.transfer_id.clone()),
    );
    {
        let mut history = state.clipboard_history.lock().await;
        history.insert(0, entry);
        if history.len() > 20 {
            history.truncate(20);
        }
    }

    // 获取发送端设备信息
    let (sender_ip, sender_port) = {
        let devices = state.devices.lock().await;
        devices
            .get(&msg.sender_id)
            .map(|d| (d.ip.clone(), d.port))
            .unwrap_or_default()
    };

    // 注册 pending transfer
    {
        let mut pending = state.pending_transfers.lock().await;
        pending.insert(
            file_data.transfer_id.clone(),
            PendingTransfer {
                display_name: file_data.display_name.clone(),
                files: file_data.files.clone(),
                total_size: file_data.total_size,
                sender_id: msg.sender_id.clone(),
                sender_ip,
                sender_port,
                base_temp_dir: temp_dir_str,
                received_bytes: 0,
                current_file_index: 0,
                status: "pending".to_string(),
            },
        );
    }

    notify_clipboard_received(state, msg).await;
}

/// 处理 file-request：发送端读取所有文件并按序分片传输
async fn handle_file_request(state: &AppState, msg: &WsMessage) {
    let transfer_id = msg.data.clone();

    log::info!(
        "Received file-request for transfer {} from {}",
        transfer_id,
        msg.sender_name
    );

    let file_info = {
        let outgoing = state.outgoing_files.lock().await;
        outgoing.get(&transfer_id).cloned()
    };

    let file_info = match file_info {
        Some(info) => info,
        None => {
            log::warn!("No outgoing file found for transfer {}", transfer_id);
            return;
        }
    };

    let peer_id = msg.sender_id.clone();
    let peer_device = {
        let devices = state.devices.lock().await;
        devices.get(&peer_id).cloned()
    };

    let peer_addr = match peer_device {
        Some(d) => format!("ws://{}:{}", d.ip, d.port),
        None => {
            log::warn!("Peer {} not found in devices", peer_id);
            return;
        }
    };

    let transfer_id_clone = transfer_id.clone();
    let local_id = state.local_id.clone();
    let local_name = state.local_name.clone();

    // 在单独任务中执行批量文件传输
    tokio::spawn(async move {
        let ws = match tokio::time::timeout(
            Duration::from_secs(10),
            tokio_tungstenite::connect_async(&peer_addr),
        )
        .await
        {
            Ok(Ok((ws, _))) => ws,
            _ => {
                log::error!("Failed to connect to peer for file transfer");
                return;
            }
        };

        let (mut sender, _receiver) = ws.split();
        use tokio::io::AsyncReadExt;

        log::info!(
            "Starting batch file transfer: {} files ({} bytes) to {}",
            file_info.files.len(),
            file_info.total_size,
            peer_addr
        );

        // 按序发送每个文件
        for (file_index, file_item) in file_info.files.iter().enumerate() {
            let mut file = match tokio::fs::File::open(&file_item.absolute_path).await {
                Ok(f) => f,
                Err(e) => {
                    log::error!("Failed to open file {}: {}", file_item.absolute_path, e);
                    continue;
                }
            };

            let mut buffer = vec![0u8; CHUNK_SIZE];
            let mut offset: u64 = 0;

            log::info!(
                "  Sending file [{}/{}]: {} ({} bytes)",
                file_index + 1,
                file_info.files.len(),
                file_item.relative_path,
                file_item.file_size
            );

            loop {
                let n = match file.read(&mut buffer).await {
                    Ok(0) => break,
                    Ok(n) => n,
                    Err(e) => {
                        log::error!("Failed to read file: {}", e);
                        break;
                    }
                };

                let chunk_b64 = BASE64.encode(&buffer[..n]);
                let chunk_msg = WsMessage {
                    id: uuid::Uuid::new_v4().to_string(),
                    msg_type: "file-chunk".to_string(),
                    content_type: "file".to_string(),
                    data: serde_json::json!({
                        "transfer_id": transfer_id_clone,
                        "file_index": file_index,
                        "offset": offset,
                        "data": chunk_b64,
                    })
                    .to_string(),
                    timestamp: current_timestamp(),
                    sender_id: local_id.clone(),
                    sender_name: local_name.clone(),
                };

                let json = serde_json::to_string(&chunk_msg).unwrap();
                if sender.send(Message::Text(json.into())).await.is_err() {
                    log::error!("Failed to send chunk to peer");
                    return;
                }

                offset += n as u64;
            }
        }

        // 发送 file-end（整个批次完成）
        let end_msg = WsMessage {
            id: uuid::Uuid::new_v4().to_string(),
            msg_type: "file-end".to_string(),
            content_type: "file".to_string(),
            data: serde_json::json!({
                "transfer_id": transfer_id_clone,
            })
            .to_string(),
            timestamp: current_timestamp(),
            sender_id: local_id.clone(),
            sender_name: local_name.clone(),
        };

        let json = serde_json::to_string(&end_msg).unwrap();
        let _ = sender.send(Message::Text(json.into())).await;
        let _ = sender.close().await;

        log::info!(
            "Batch file transfer complete: {} files ({} bytes)",
            file_info.files.len(),
            file_info.total_size
        );
    });
}

/// 处理 file-chunk：接收端根据 file_index 将 chunk 数据写入对应文件
async fn handle_file_chunk(state: &AppState, msg: &WsMessage) {
    let chunk_data: serde_json::Value = match serde_json::from_str(&msg.data) {
        Ok(d) => d,
        Err(e) => {
            log::warn!("Failed to parse file-chunk data: {}", e);
            return;
        }
    };

    let transfer_id = match chunk_data["transfer_id"].as_str() {
        Some(id) => id.to_string(),
        None => return,
    };

    let file_index = chunk_data["file_index"].as_u64().unwrap_or(0) as usize;

    let chunk_b64 = match chunk_data["data"].as_str() {
        Some(d) => d,
        None => return,
    };

    let chunk_bytes = match BASE64.decode(chunk_b64) {
        Ok(b) => b,
        Err(e) => {
            log::warn!("Failed to decode chunk base64: {}", e);
            return;
        }
    };

    // 获取目标文件路径
    let target_path = {
        let mut pending = state.pending_transfers.lock().await;
        if let Some(pt) = pending.get_mut(&transfer_id) {
            pt.received_bytes += chunk_bytes.len() as u64;
            pt.status = "downloading".to_string();
            pt.current_file_index = file_index;

            if file_index < pt.files.len() {
                let relative_path = &pt.files[file_index].relative_path;
                let base_dir = std::path::Path::new(&pt.base_temp_dir);
                base_dir.join(relative_path).to_string_lossy().to_string()
            } else {
                log::warn!("file_index {} out of range", file_index);
                return;
            }
        } else {
            return;
        }
    };

    // 确保父目录存在
    if let Some(parent) = std::path::Path::new(&target_path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    use std::io::Write;
    match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&target_path)
    {
        Ok(mut file) => {
            if let Err(e) = file.write_all(&chunk_bytes) {
                log::error!("Failed to write chunk: {}", e);
            }
        }
        Err(e) => {
            log::error!("Failed to open file for chunk writing: {}", e);
        }
    }
}

/// 处理 file-end：接收端标记传输完成，移动文件到下载目录，更新剪贴板
async fn handle_file_end(state: &AppState, msg: &WsMessage) {
    let end_data: serde_json::Value = match serde_json::from_str(&msg.data) {
        Ok(d) => d,
        Err(e) => {
            log::warn!("Failed to parse file-end data: {}", e);
            return;
        }
    };

    let transfer_id = match end_data["transfer_id"].as_str() {
        Some(id) => id.to_string(),
        None => return,
    };

    log::info!("Batch file transfer complete for {}", transfer_id);

    let (base_temp_dir, files, total_size, display_name) = {
        let mut pending = state.pending_transfers.lock().await;
        if let Some(pt) = pending.get_mut(&transfer_id) {
            pt.status = "done".to_string();
            (
                pt.base_temp_dir.clone(),
                pt.files.clone(),
                pt.total_size,
                pt.display_name.clone(),
            )
        } else {
            return;
        }
    };

    // 将所有文件移动到用户配置的下载目录，保持相对路径结构
    let final_dir = {
        let download_dir = state.download_dir.lock().await;
        download_dir.clone()
    };
    let dest_base = std::path::Path::new(&final_dir);
    let _ = std::fs::create_dir_all(dest_base);

    let temp_base = std::path::Path::new(&base_temp_dir);
    let mut final_paths: Vec<String> = Vec::new();

    for file_item in &files {
        // 标准化路径分隔符（macOS 用 /，Windows 需要 \）
        let normalized_rel = if cfg!(target_os = "windows") {
            file_item.relative_path.replace('/', "\\")
        } else {
            file_item.relative_path.clone()
        };
        let src_path = temp_base.join(&file_item.relative_path);
        let mut dest_path = dest_base.join(&normalized_rel);

        // 确保目标父目录存在
        if let Some(parent) = dest_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        // 同名文件处理（仅对单文件批次添加数字后缀）
        if dest_path.exists() && files.len() == 1 {
            let stem = dest_path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            let ext_str = dest_path
                .extension()
                .map(|e| format!(".{}", e.to_string_lossy()))
                .unwrap_or_default();
            let parent_dir = dest_path.parent().unwrap_or(dest_base);
            for i in 1..100 {
                let candidate = parent_dir.join(format!("{}({}){}", stem, i, ext_str));
                if !candidate.exists() {
                    dest_path = candidate;
                    break;
                }
            }
        }

        // 移动文件（同卷 rename，跨卷 copy+delete）
        if src_path.exists() {
            match std::fs::rename(&src_path, &dest_path) {
                Ok(_) => {}
                Err(_) => {
                    if let Err(e) = std::fs::copy(&src_path, &dest_path) {
                        log::error!("Failed to copy file {:?} -> {:?}: {}", src_path, dest_path, e);
                        continue;
                    } else {
                        let _ = std::fs::remove_file(&src_path);
                    }
                }
            }
            final_paths.push(dest_path.to_string_lossy().to_string());
        } else {
            log::warn!("Temp file not found: {:?}", src_path);
        }
    }

    // 清理临时目录
    let _ = std::fs::remove_dir_all(&base_temp_dir);

    log::info!("Files saved to: {} ({} files)", final_dir, final_paths.len());

    // 确定保存路径：文件夹传输时指向顶层文件夹，单文件时指向文件
    let is_multi_or_folder = files.len() > 1 || files.first().map(|f| f.relative_path.contains('/')).unwrap_or(false);
    let saved_location = if is_multi_or_folder {
        // 多文件或文件夹传输：获取顶层文件夹路径
        if let Some(first_file) = files.first() {
            let top_component = first_file.relative_path.split('/').next().unwrap_or("");
            if top_component.is_empty() {
                final_dir.clone()
            } else {
                let top_path = dest_base.join(if cfg!(target_os = "windows") {
                    top_component.replace('/', "\\")
                } else {
                    top_component.to_string()
                });
                top_path.to_string_lossy().to_string()
            }
        } else {
            final_dir.clone()
        }
    } else {
        // 单文件：指向文件本身
        final_paths.first().cloned().unwrap_or(final_dir.clone())
    };

    // 只有单文件时才写入剪贴板（多文件/文件夹 Windows 上 Set-Clipboard 不支持多路径，且容易触发重复广播）
    if !is_multi_or_folder && final_paths.len() == 1 {
        let clipboard_paths = final_paths.clone();

        // 获取剪贴板写入互斥锁
        let _write_guard = state.clipboard_write_lock.lock().await;

        {
            use std::sync::atomic::Ordering;
            state.is_writing_clipboard.store(true, Ordering::SeqCst);
        }

        let _ = crate::clipboard::set_clipboard_files(&clipboard_paths).await;

        // 设置哈希防止 monitor 重复检测（必须在 is_writing_clipboard=false 之前设置！）
        {
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};
            use std::sync::atomic::Ordering;
            let mut hasher = DefaultHasher::new();
            let lower: Vec<String> = clipboard_paths.iter().map(|p| p.to_lowercase()).collect();
            lower.hash(&mut hasher);
            let hash = hasher.finish();
            state.written_placeholder_hash.store(hash, Ordering::SeqCst);
        }

        sleep(Duration::from_millis(500)).await;
        {
            use std::sync::atomic::Ordering;
            state.is_writing_clipboard.store(false, Ordering::SeqCst);
        }
    }

    // 更新 clipboard_history 中的状态和文件路径
    let first_path = Some(saved_location);
    {
        let mut history = state.clipboard_history.lock().await;
        for entry in history.iter_mut() {
            if entry.transfer_id.as_deref() == Some(transfer_id.as_str()) {
                entry.download_status = Some("done".to_string());
                entry.file_path = first_path.clone();
                break;
            }
        }
    }

    let handle = state.app_handle.lock().await;
    if let Some(app) = handle.as_ref() {
        let _ = app.emit(
            "file-download-complete",
            serde_json::json!({
                "transfer_id": transfer_id,
                "display_name": display_name,
                "file_count": files.len(),
                "total_size": total_size,
                "final_dir": final_dir,
            }),
        );
    }
}

// ========================== 通知函数 ==========================

async fn notify_clipboard_received(state: &AppState, msg: &WsMessage) {
    // 使用 tokio::spawn 异步发送事件，避免 app_handle 锁竞争阻塞其他任务
    let app_handle = state.app_handle.clone();
    let msg_clone = msg.clone();
    tokio::spawn(async move {
        let handle = app_handle.lock().await;
        if let Some(app) = handle.as_ref() {
            use tauri::Emitter;
            let _ = app.emit("clipboard-received", &msg_clone);
        }
    });
}

async fn notify_devices_changed(state: &AppState) {
    // 使用 tokio::spawn 异步发送事件，避免同时持有 app_handle 和 devices 锁导致死锁
    let app_handle = state.app_handle.clone();
    let devices_arc = state.devices.clone();
    tokio::spawn(async move {
        let handle = app_handle.lock().await;
        if let Some(app) = handle.as_ref() {
            use tauri::Emitter;
            let devices: Vec<DeviceInfo> = devices_arc.lock().await.values().cloned().collect();
            let _ = app.emit("devices-changed", &devices);
        }
    });
}

fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}
