use log::{Level, Log, Metadata, Record};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

/// 日志文件最大大小（2MB），超过后自动轮转
const MAX_LOG_SIZE: u64 = 2 * 1024 * 1024;
/// 保留的历史日志文件数量
const MAX_BACKUP_FILES: usize = 3;

/// 简单的文件日志器，支持按大小轮转
pub struct FileLogger {
    log_dir: PathBuf,
    file: Mutex<Option<File>>,
}

impl FileLogger {
    pub fn new(log_dir: PathBuf) -> Self {
        std::fs::create_dir_all(&log_dir).ok();
        Self {
            log_dir,
            file: Mutex::new(None),
        }
    }

    fn log_path(&self) -> PathBuf {
        self.log_dir.join("clipsync.log")
    }

    fn ensure_file(&self) -> std::io::Result<File> {
        let path = self.log_path();

        // 检查是否需要轮转
        if path.exists() {
            if let Ok(meta) = std::fs::metadata(&path) {
                if meta.len() > MAX_LOG_SIZE {
                    self.rotate_logs();
                }
            }
        }

        OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
    }

    fn rotate_logs(&self) {
        let base = self.log_path();

        // 删除最旧的备份
        for i in (1..=MAX_BACKUP_FILES).rev() {
            let src = self.log_dir.join(format!("clipsync.{}.log", i));
            if i == MAX_BACKUP_FILES {
                let _ = std::fs::remove_file(&src);
            } else {
                let dst = self.log_dir.join(format!("clipsync.{}.log", i + 1));
                let _ = std::fs::rename(&src, &dst);
            }
        }

        // 当前日志 -> clipsync.1.log
        if base.exists() {
            let _ = std::fs::rename(&base, self.log_dir.join("clipsync.1.log"));
        }
    }

    fn format_time() -> String {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // 简单的本地时间格式化（使用秒级时间戳转换为可读格式）
        let secs = now;
        let days = secs / 86400;
        let day_secs = secs % 86400;
        let hours = day_secs / 3600;
        let minutes = (day_secs % 3600) / 60;
        let seconds = day_secs % 60;

        // 从 1970-01-01 计算年月日
        let (year, month, day) = days_to_ymd(days);

        format!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
            year, month, day, hours, minutes, seconds
        )
    }
}

impl Log for FileLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Info
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let time = Self::format_time();
        let level = record.level();
        let target = record.target();
        let message = record.args();

        let line = format!("[{}] [{}] {} - {}\n", time, level, target, message);

        if let Ok(mut file_opt) = self.file.lock() {
            // 确保文件打开
            if file_opt.is_none() {
                *file_opt = self.ensure_file().ok();
            }

            let write_ok = if let Some(ref mut file) = *file_opt {
                file.write_all(line.as_bytes()).is_ok()
            } else {
                false
            };

            if !write_ok {
                // 写入失败，尝试重新打开文件
                *file_opt = self.ensure_file().ok();
                if let Some(ref mut file) = *file_opt {
                    let _ = file.write_all(line.as_bytes());
                    let _ = file.flush();
                }
            } else if let Some(ref mut file) = *file_opt {
                let _ = file.flush();
            }
        }
    }

    fn flush(&self) {
        if let Ok(mut file_opt) = self.file.lock() {
            if let Some(ref mut file) = *file_opt {
                let _ = file.flush();
            }
        }
    }
}

/// 获取日志目录路径
pub fn log_dir() -> PathBuf {
    let config_dir = dirs::config_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")));
    config_dir.join("clipsync").join("logs")
}

/// 初始化文件日志，同时输出到控制台（开发模式）
pub fn init_logger() {
    let dir = log_dir();
    let file_logger = FileLogger::new(dir);

    // 设置全局日志器
    log::set_boxed_logger(Box::new(file_logger))
        .expect("Failed to set file logger");

    // 生产环境默认 Info 级别，开发环境可通过 RUST_LOG 覆盖
    if cfg!(debug_assertions) {
        log::set_max_level(log::LevelFilter::Debug);
    } else {
        log::set_max_level(log::LevelFilter::Info);
    }

    log::info!("ClipSync logger initialized, log dir: {:?}", log_dir());
}

// 辅助函数：从 Unix epoch 天数计算年月日
fn days_to_ymd(days: u64) -> (u64, u64, u64) {
    // 简单算法，足够日志时间戳使用
    let mut y = 1970u64;
    let mut remaining = days;

    loop {
        let days_in_year = if is_leap(y) { 366 } else { 365 };
        if remaining < days_in_year {
            break;
        }
        remaining -= days_in_year;
        y += 1;
    }

    let leap = is_leap(y);
    let month_days = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];

    let mut m = 1u64;
    for &d in &month_days {
        if remaining < d {
            break;
        }
        remaining -= d;
        m += 1;
    }

    (y, m, remaining + 1)
}

fn is_leap(y: u64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0)
}
