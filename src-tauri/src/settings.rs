use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 应用设置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    /// 文件下载保存目录
    pub download_dir: String,
    /// 全局快捷键（打开剪贴板历史窗口）
    #[serde(default = "default_shortcut")]
    pub shortcut: String,
    /// 开机自动启动
    #[serde(default)]
    pub autostart: bool,
}

fn default_shortcut() -> String {
    "CmdOrCtrl+Shift+V".to_string()
}

impl Default for AppSettings {
    fn default() -> Self {
        let download_dir = dirs::download_dir()
            .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")))
            .to_string_lossy()
            .to_string();
        Self {
            download_dir,
            shortcut: default_shortcut(),
            autostart: false,
        }
    }
}

/// 获取设置文件路径
fn settings_path() -> PathBuf {
    let config_dir = dirs::config_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")));
    config_dir.join("clipsync").join("settings.json")
}

/// 加载设置（文件不存在则返回默认值）
pub fn load_settings() -> AppSettings {
    let path = settings_path();
    if path.exists() {
        match std::fs::read_to_string(&path) {
            Ok(content) => {
                match serde_json::from_str::<AppSettings>(&content) {
                    Ok(settings) => return settings,
                    Err(e) => {
                        log::warn!("Failed to parse settings: {}", e);
                    }
                }
            }
            Err(e) => {
                log::warn!("Failed to read settings file: {}", e);
            }
        }
    }
    AppSettings::default()
}

/// 保存设置到文件
pub fn save_settings(settings: &AppSettings) -> Result<(), String> {
    let path = settings_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建配置目录失败: {}", e))?;
    }
    let json = serde_json::to_string_pretty(settings).map_err(|e| format!("序列化失败: {}", e))?;
    std::fs::write(&path, json).map_err(|e| format!("写入配置文件失败: {}", e))?;
    log::info!("Settings saved to {:?}", path);
    Ok(())
}

/// 获取缓存目录路径
pub fn cache_dir() -> PathBuf {
    std::env::temp_dir().join("clipsync")
}

/// 获取缓存大小（字节）
pub fn get_cache_size_bytes() -> u64 {
    let cache = cache_dir();
    if !cache.exists() {
        return 0;
    }
    let mut total: u64 = 0;
    if let Ok(entries) = std::fs::read_dir(&cache) {
        for entry in entries.flatten() {
            if let Ok(meta) = entry.metadata() {
                if meta.is_file() {
                    total += meta.len();
                }
            }
        }
    }
    total
}

/// 清除缓存目录中的所有文件
pub fn clear_cache_files() -> Result<u64, String> {
    let cache = cache_dir();
    if !cache.exists() {
        return Ok(0);
    }
    let mut cleared: u64 = 0;
    if let Ok(entries) = std::fs::read_dir(&cache) {
        for entry in entries.flatten() {
            if let Ok(meta) = entry.metadata() {
                if meta.is_file() {
                    let size = meta.len();
                    if std::fs::remove_file(entry.path()).is_ok() {
                        cleared += size;
                    }
                }
            }
        }
    }
    log::info!("Cache cleared: {} bytes", cleared);
    Ok(cleared)
}
