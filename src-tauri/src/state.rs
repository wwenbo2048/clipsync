use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::Arc;
use tokio::sync::Mutex;

/// WebSocket Sender 类型别名
pub type WsSender = futures_util::stream::SplitSink<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    tokio_tungstenite::tungstenite::Message,
>;

/// 设备信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub ip: String,
    pub port: u16,
    pub status: DeviceStatus,
    pub last_seen: u64,
}

/// 设备状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DeviceStatus {
    Connected,
    Connecting,
    Offline,
}

/// 剪贴板历史条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardEntry {
    pub id: String,
    pub text: String,
    pub timestamp: u64,
    pub source: String,
    /// 内容类型: "text" 或 "file"
    pub content_type: String,
    /// 文件实际路径（下载完成后或本机发送时填写）
    pub file_path: Option<String>,
    /// 文件大小（字节）
    pub file_size: Option<u64>,
    /// 下载状态: "pending" / "downloading" / "done"（仅文件类型有效）
    pub download_status: Option<String>,
    /// 文件传输ID（关联 PendingTransfer）
    pub transfer_id: Option<String>,
}

impl ClipboardEntry {
    /// 创建文本条目
    pub fn new_text(id: String, text: String, timestamp: u64, source: String) -> Self {
        Self {
            id,
            text,
            timestamp,
            source,
            content_type: "text".to_string(),
            file_path: None,
            file_size: None,
            download_status: None,
            transfer_id: None,
        }
    }

    /// 创建文件条目
    pub fn new_file(
        id: String,
        file_name: String,
        timestamp: u64,
        source: String,
        file_size: u64,
        file_path: Option<String>,
        download_status: Option<String>,
        transfer_id: Option<String>,
    ) -> Self {
        Self {
            id,
            text: file_name,
            timestamp,
            source,
            content_type: "file".to_string(),
            file_path,
            file_size: Some(file_size),
            download_status,
            transfer_id,
        }
    }
}

/// 批量传输中的单个文件信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileItemInfo {
    /// 相对路径（如 "folder/sub/file.txt" 或 "file.txt"）
    pub relative_path: String,
    /// 文件大小（字节）
    pub file_size: u64,
}

/// 待接收的文件传输信息（接收端维护）
#[derive(Debug, Clone)]
pub struct PendingTransfer {
    /// 显示名称（如 "3个文件" 或 "folder_name"）
    pub display_name: String,
    /// 批量中所有文件的信息
    pub files: Vec<FileItemInfo>,
    /// 所有文件总大小
    pub total_size: u64,
    pub sender_id: String,
    pub sender_ip: String,
    pub sender_port: u16,
    /// 临时基础目录路径
    pub base_temp_dir: String,
    /// 已接收字节数（所有文件累计）
    pub received_bytes: u64,
    /// 当前正在接收的文件索引
    pub current_file_index: usize,
    /// 下载状态: pending / downloading / done / error
    pub status: String,
}

/// 待发送的单个文件项（发送端维护）
#[derive(Debug, Clone)]
pub struct OutgoingFileItem {
    /// 本机绝对路径
    pub absolute_path: String,
    /// 传输时的相对路径
    pub relative_path: String,
    /// 文件大小
    pub file_size: u64,
}

/// 待发送的文件批次（发送端维护）
#[derive(Debug, Clone)]
pub struct OutgoingFile {
    /// 所有要发送的文件
    pub files: Vec<OutgoingFileItem>,
    /// 总大小
    pub total_size: u64,
}

/// file-start 消息中 data 字段的结构（支持批量）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileStartData {
    pub transfer_id: String,
    /// 显示名称（如 "3个文件" 或 "folder_name"）
    pub display_name: String,
    /// 批量中所有文件的信息
    pub files: Vec<FileItemInfo>,
    /// 所有文件总大小
    pub total_size: u64,
}

/// WebSocket 消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsMessage {
    pub id: String,
    pub msg_type: String,
    pub content_type: String,
    pub data: String,
    pub timestamp: u64,
    pub sender_id: String,
    pub sender_name: String,
}

/// UDP 广播信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BroadcastInfo {
    pub id: String,
    pub name: String,
    pub ip: String,
    pub port: u16,
    pub timestamp: u64,
}

/// 全局应用状态
pub struct AppState {
    /// 已发现的设备列表 (device_id -> DeviceInfo)
    pub devices: Arc<Mutex<HashMap<String, DeviceInfo>>>,
    /// 剪贴板历史记录
    pub clipboard_history: Arc<Mutex<Vec<ClipboardEntry>>>,
    /// 已处理的消息ID (用于去重)
    pub seen_messages: Arc<Mutex<HashSet<String>>>,
    /// 本机设备ID
    pub local_id: String,
    /// 本机设备名称
    pub local_name: String,
    /// 本机WebSocket端口
    pub local_port: u16,
    /// 是否正在写入剪贴板 (防止回环)
    pub is_writing_clipboard: Arc<AtomicBool>,
    /// 已写入剪贴板的占位文件路径哈希（防止 monitor 重复检测）
    pub written_placeholder_hash: Arc<AtomicU64>,
    /// 从远程收到的文本哈希（防止 monitor 回弹广播）
    pub last_remote_text_hash: Arc<AtomicU64>,
    /// Tauri AppHandle (用于发送事件到前端)
    pub app_handle: Arc<Mutex<Option<tauri::AppHandle>>>,
    /// 待接收的文件传输 (transfer_id -> PendingTransfer)
    pub pending_transfers: Arc<Mutex<HashMap<String, PendingTransfer>>>,
    /// 待发送的文件 (transfer_id -> OutgoingFile)
    pub outgoing_files: Arc<Mutex<HashMap<String, OutgoingFile>>>,
    /// 已建立的 peer WebSocket sender (device_id -> WsSender)
    pub peers: Arc<Mutex<HashMap<String, WsSender>>>,
    /// 文件下载保存目录
    pub download_dir: Arc<Mutex<String>>,
    /// 是否暂停同步（断开连接状态）
    pub is_paused: Arc<AtomicBool>,
    /// 当前全局快捷键
    pub shortcut: Arc<Mutex<String>>,
    /// 开机自动启动
    pub autostart: Arc<Mutex<bool>>,
    /// 剪贴板写入互斥锁（防止并发写入损坏系统剪贴板）
    pub clipboard_write_lock: Arc<tokio::sync::Mutex<()>>,
}

impl AppState {
    pub fn new() -> Self {
        let local_id = uuid::Uuid::new_v4().to_string();
        let local_name = hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "Unknown".to_string());

        // 打印所有物理网卡信息
        let interfaces = crate::network::get_physical_interfaces();
        log::info!("Physical network interfaces ({}):", interfaces.len());
        for iface in &interfaces {
            log::info!(
                "  {} - {} (broadcast: {})",
                iface.name,
                iface.ip,
                iface.broadcast
            );
        }

        // 加载设置
        let settings = crate::settings::load_settings();
        log::info!("Download dir: {}", settings.download_dir);

        Self {
            devices: Arc::new(Mutex::new(HashMap::new())),
            clipboard_history: Arc::new(Mutex::new(Vec::new())),
            seen_messages: Arc::new(Mutex::new(HashSet::new())),
            local_id,
            local_name,
            local_port: 37021,
            is_writing_clipboard: Arc::new(AtomicBool::new(false)),
            written_placeholder_hash: Arc::new(AtomicU64::new(0)),
            last_remote_text_hash: Arc::new(AtomicU64::new(0)),
            app_handle: Arc::new(Mutex::new(None)),
            pending_transfers: Arc::new(Mutex::new(HashMap::new())),
            outgoing_files: Arc::new(Mutex::new(HashMap::new())),
            peers: Arc::new(Mutex::new(HashMap::new())),
            download_dir: Arc::new(Mutex::new(settings.download_dir)),
            is_paused: Arc::new(AtomicBool::new(false)),
            shortcut: Arc::new(Mutex::new(settings.shortcut)),
            autostart: Arc::new(Mutex::new(settings.autostart)),
            clipboard_write_lock: Arc::new(tokio::sync::Mutex::new(())),
        }
    }
}
