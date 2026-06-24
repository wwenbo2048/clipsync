use crate::network;
use crate::state::{AppState, BroadcastInfo, DeviceInfo, DeviceStatus};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::Emitter;
use tokio::net::UdpSocket;
use tokio::time::{interval, Duration};

const BROADCAST_PORT: u16 = 37020;
const BROADCAST_INTERVAL_SECS: u64 = 2;
const DEVICE_TIMEOUT_SECS: u64 = 10;

/// 启动设备发现服务
pub async fn start_discovery(state: Arc<AppState>) {
    // 启动监听（全局一个 socket）
    let state_listen = state.clone();
    tokio::spawn(async move {
        listen_loop(state_listen).await;
    });

    // 启动广播发送（每个物理网卡一个任务）
    let state_broadcast = state.clone();
    tokio::spawn(async move {
        broadcast_all_interfaces(state_broadcast).await;
    });

    // 启动设备清理
    let state_cleanup = state.clone();
    tokio::spawn(async move {
        cleanup_loop(state_cleanup).await;
    });
}

/// 在每个物理网卡上启动广播发送
async fn broadcast_all_interfaces(state: Arc<AppState>) {
    let mut ticker = interval(Duration::from_secs(BROADCAST_INTERVAL_SECS));

    loop {
        ticker.tick().await;

        // 如果同步已暂停，跳过广播
        if state.is_paused.load(std::sync::atomic::Ordering::SeqCst) {
            continue;
        }

        // 每次广播前重新获取网卡列表（网卡可能动态变化）
        let interfaces = network::get_physical_interfaces();

        if interfaces.is_empty() {
            log::debug!("No physical interfaces found for broadcast");
            continue;
        }

        for iface in &interfaces {
            let info = BroadcastInfo {
                id: state.local_id.clone(),
                name: state.local_name.clone(),
                ip: iface.ip.clone(),
                port: state.local_port,
                timestamp: current_timestamp(),
            };

            if let Ok(data) = serde_json::to_string(&info) {
                // 绑定到具体网卡的 IP，确保广播走正确的网卡
                let bind_addr = format!("{}:0", iface.ip);
                match UdpSocket::bind(&bind_addr).await {
                    Ok(socket) => {
                        if socket.set_broadcast(true).is_ok() {
                            let target = format!("{}:{}", iface.broadcast, BROADCAST_PORT);
                            match socket.send_to(data.as_bytes(), &target).await {
                                Ok(_) => {
                                    log::debug!(
                                        "Broadcast on {} ({}) -> {}",
                                        iface.name,
                                        iface.ip,
                                        target
                                    );
                                }
                                Err(e) => {
                                    log::debug!(
                                        "Broadcast failed on {}: {}",
                                        iface.name,
                                        e
                                    );
                                }
                            }
                        }
                    }
                    Err(e) => {
                        log::debug!("Failed to bind to {}: {}", iface.ip, e);
                    }
                }
            }
        }
    }
}

/// 监听 UDP 广播（绑定 0.0.0.0 接收所有网卡的广播）
async fn listen_loop(state: Arc<AppState>) {
    let socket = match UdpSocket::bind(format!("0.0.0.0:{}", BROADCAST_PORT)).await {
        Ok(s) => s,
        Err(e) => {
            log::error!("Failed to bind UDP listener on port {}: {}. Is another instance running?", BROADCAST_PORT, e);
            return;
        }
    };
    socket.set_broadcast(true).ok();

    log::info!("UDP discovery listener started on port {}", BROADCAST_PORT);
    let mut buf = [0u8; 1024];

    loop {
        match socket.recv_from(&mut buf).await {
            Ok((len, addr)) => {
                // 如果同步已暂停，忽略收到的广播
                if state.is_paused.load(std::sync::atomic::Ordering::SeqCst) {
                    continue;
                }
                if let Ok(data) = std::str::from_utf8(&buf[..len]) {
                    if let Ok(info) = serde_json::from_str::<BroadcastInfo>(data) {
                        // 忽略自己的广播
                        if info.id == state.local_id {
                            continue;
                        }
                        log::debug!(
                            "Discovered device: {} ({}) from {}",
                            info.name,
                            info.ip,
                            addr
                        );
                        handle_discovered_device(&state, &info).await;
                    }
                }
            }
            Err(e) => {
                log::warn!("Failed to receive broadcast: {}", e);
            }
        }
    }
}

/// 处理发现的设备
async fn handle_discovered_device(state: &AppState, info: &BroadcastInfo) {
    let mut devices = state.devices.lock().await;
    let prev_status = devices.get(&info.id).map(|d| d.status.clone());

    devices.insert(
        info.id.clone(),
        DeviceInfo {
            id: info.id.clone(),
            name: info.name.clone(),
            ip: info.ip.clone(),
            port: info.port,
            status: DeviceStatus::Connected,
            last_seen: current_timestamp(),
        },
    );

    let should_notify = prev_status.is_none() || prev_status == Some(DeviceStatus::Offline);

    if should_notify {
        log::info!("New/returned device: {} ({})", info.name, info.ip);
        drop(devices); // 释放锁再通知
        notify_devices_changed(state).await;
    }
}

/// 定期清理超时设备
async fn cleanup_loop(state: Arc<AppState>) {
    let mut ticker = interval(Duration::from_secs(5));

    loop {
        ticker.tick().await;
        let now = current_timestamp();
        let mut devices = state.devices.lock().await;
        let mut changed = false;

        for device in devices.values_mut() {
            if now - device.last_seen > DEVICE_TIMEOUT_SECS
                && device.status != DeviceStatus::Offline
            {
                log::info!("Device timed out: {}", device.name);
                device.status = DeviceStatus::Offline;
                changed = true;
            }
        }

        if changed {
            drop(devices);
            notify_devices_changed(&state).await;
        }
    }
}

/// 通知前端设备列表变化
async fn notify_devices_changed(state: &AppState) {
    let handle = state.app_handle.lock().await;
    if let Some(app) = handle.as_ref() {
        let devices: Vec<DeviceInfo> = state.devices.lock().await.values().cloned().collect();
        let _ = app.emit("devices-changed", &devices);
    }
}

fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}
