use crate::state::{AppState, FileItemInfo, FileStartData, OutgoingFile, OutgoingFileItem};
use crate::transport::broadcast_file_metadata;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::time::{interval, Duration};

const POLL_INTERVAL_MS: u64 = 200;

#[cfg(target_os = "windows")]
#[link(name = "user32")]
extern "system" {
    fn GetClipboardSequenceNumber() -> u32;
}

/// 获取剪贴板序列号（Windows API）
#[cfg(target_os = "windows")]
fn clipboard_sequence() -> u32 {
    unsafe { GetClipboardSequenceNumber() }
}

#[cfg(not(target_os = "windows"))]
fn clipboard_sequence() -> u32 {
    0
}

/// 获取 macOS NSPasteboard.generalPasteboard.changeCount
/// 通过 Objective-C runtime FFI 直接调用，无需创建 arboard 实例
#[cfg(target_os = "macos")]
fn pasteboard_change_count() -> i64 {
    #[link(name = "objc", kind = "dylib")]
    extern "C" {
        fn objc_getClass(name: *const std::ffi::c_char) -> *mut std::ffi::c_void;
        fn sel_registerName(name: *const std::ffi::c_char) -> *mut std::ffi::c_void;
        fn objc_msgSend(obj: *mut std::ffi::c_void, op: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    }

    unsafe {
        let cls = objc_getClass(c"NSPasteboard".as_ptr());
        if cls.is_null() {
            return -1;
        }
        let sel_gp = sel_registerName(c"generalPasteboard".as_ptr());
        let pb = objc_msgSend(cls, sel_gp);
        if pb.is_null() {
            return -1;
        }
        let sel_cc = sel_registerName(c"changeCount".as_ptr());
        objc_msgSend(pb, sel_cc) as i64
    }
}

/// 将文件路径列表写入系统剪贴板（跨平台）
pub async fn set_clipboard_files(paths: &[String]) {
    #[cfg(target_os = "macos")]
    {
        let file_list = paths
            .iter()
            .map(|p| format!("'{}'", p.replace('\\', "\\\\").replace('\'', "\\'")))
            .collect::<Vec<_>>()
            .join(",");
        let script = format!(
            "ObjC.import('AppKit');\
             var pb=$.NSPasteboard.generalPasteboard;\
             pb.clearContents;\
             var files=[{}];\
             var urls=files.map(function(f){{return $.NSURL.fileURLWithPath(f)}});\
             pb.writeObjects(urls)",
            file_list
        );
        let _ = tokio::time::timeout(
            Duration::from_secs(5),
            tokio::process::Command::new("osascript")
                .args(["-l", "JavaScript", "-e", &script])
                .output(),
        )
        .await;
    }
    #[cfg(target_os = "windows")]
    {
        if let Some(first) = paths.first() {
            let ps_path = first.replace("'", "''");
            #[cfg(target_os = "windows")]
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            let _ = tokio::process::Command::new("powershell")
                .args([
                    "-NoProfile",
                    "-Command",
                    &format!("[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; $OutputEncoding = [System.Text.Encoding]::UTF8; Set-Clipboard -Path '{}'", ps_path),
                ])
                .creation_flags(CREATE_NO_WINDOW)
                .output()
                .await;
        }
    }
}

/// 启动剪贴板监听
pub async fn start_clipboard_monitor(state: Arc<AppState>) {
    tokio::spawn(async move {
        monitor_loop(state).await;
    });
}

/// 检查文件是否来自 clipsync 占位目录
fn is_placeholder_file(path: &str) -> bool {
    // 方式1：直接路径前缀匹配
    let clipsync_dir = std::env::temp_dir()
        .join("clipsync")
        .to_string_lossy()
        .to_string();
    let path_lower = path.to_lowercase().replace('/', "\\");
    let dir_lower = clipsync_dir.to_lowercase().replace('/', "\\");
    if path_lower.starts_with(&dir_lower) {
        return true;
    }

    // 方式2：尝试规范化路径后比较（处理 8.3 短路径等差异）
    if let Ok(canonical_path) = std::fs::canonicalize(path) {
        let canonical_str = canonical_path.to_string_lossy().to_lowercase().replace('/', "\\");
        if canonical_str.starts_with(&dir_lower) {
            return true;
        }
    }

    // 方式3：检查文件是否 0 字节且文件名看起来像传输占位符
    if let Ok(meta) = std::fs::metadata(path) {
        if meta.len() == 0 {
            // 0 字节文件且位于某个 UUID 风格目录下，大概率是占位符
            let p = std::path::Path::new(path);
            if let Some(parent) = p.parent() {
                let parent_name = parent.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                // UUID 格式: xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx
                if parent_name.len() == 36 && parent_name.chars().filter(|c| *c == '-').count() == 4 {
                    return true;
                }
            }
        }
    }

    false
}

/// 使用 PowerShell 检测剪贴板中的文件路径（带超时保护）
#[cfg(target_os = "windows")]
async fn get_clipboard_files() -> Option<Vec<String>> {
    #[cfg(target_os = "windows")]
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    // 添加 3 秒超时保护，防止 PowerShell 挂起阻塞 monitor_loop
    let result = tokio::time::timeout(
        Duration::from_secs(3),
        tokio::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                r#"[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; $OutputEncoding = [System.Text.Encoding]::UTF8; $files = Get-Clipboard -Format FileDropList -ErrorAction SilentlyContinue; if ($files) { $files | ForEach-Object { $_.FullName } } else { "" }"#,
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .output(),
    )
    .await;

    let output = match result {
        Ok(Ok(output)) => output,
        Ok(Err(_)) => return None,
        Err(_) => {
            log::warn!("PowerShell clipboard file detection timed out");
            return None;
        }
    };

    if !output.status.success() {
        log::warn!(
            "PowerShell clipboard detection failed: exit={:?}, stderr={}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout)
        .trim_start_matches('\u{FEFF}') // strip UTF-8 BOM if present
        .trim()
        .to_string();
    log::debug!("PowerShell raw stdout bytes: {:?}", &output.stdout);
    log::debug!("PowerShell decoded paths: {:?}", &stdout);
    if stdout.is_empty() {
        return None;
    }

    let paths: Vec<String> = stdout
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();

    if paths.is_empty() {
        None
    } else {
        Some(paths)
    }
}

/// 使用 osascript (JXA) 检测剪贴板中的文件路径（带超时保护）
#[cfg(target_os = "macos")]
async fn get_clipboard_files() -> Option<Vec<String>> {
    let script = "ObjC.import('AppKit');\
var pb=$.NSPasteboard.generalPasteboard;\
var items=pb.pasteboardItems;\
var r=[];\
for(var i=0;i<items.count;i++){\
var item=items.objectAtIndex(i);\
var urlStr=item.stringForType('public.file-url');\
if(urlStr){var url=$.NSURL.URLWithString(urlStr);r.push(url.path.js)}}\
r.length>0?r.join('\\n'):''";

    // 添加 3 秒超时保护，防止 osascript 挂起阻塞 monitor_loop
    let result = tokio::time::timeout(
        Duration::from_secs(3),
        tokio::process::Command::new("osascript")
            .args(["-l", "JavaScript", "-e", script])
            .output(),
    )
    .await;

    let output = match result {
        Ok(Ok(output)) => output,
        Ok(Err(_)) => return None,
        Err(_) => {
            log::warn!("osascript clipboard file detection timed out");
            return None;
        }
    };

    if !output.status.success() {
        log::debug!("osascript clipboard file detection failed");
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if stdout.is_empty() {
        return None;
    }

    let paths: Vec<String> = stdout
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();

    if paths.is_empty() {
        None
    } else {
        Some(paths)
    }
}

/// 获取文件大小（字节）
fn get_file_size(path: &str) -> Option<u64> {
    std::fs::metadata(path).ok().map(|m| m.len())
}

/// 递归枚举目录下所有文件，返回 (absolute_path, relative_path, file_size)
fn enumerate_directory(dir_path: &Path, base_name: &str) -> Vec<(String, String, u64)> {
    let mut results = Vec::new();
    enumerate_dir_recursive(dir_path, base_name, &mut results);
    results
}

fn enumerate_dir_recursive(
    current_path: &Path,
    relative_prefix: &str,
    results: &mut Vec<(String, String, u64)>,
) {
    let entries = match std::fs::read_dir(current_path) {
        Ok(entries) => entries,
        Err(e) => {
            log::warn!("Failed to read dir {:?}: {}", current_path, e);
            return;
        }
    };

    for entry in entries.flatten() {
        let entry_path = entry.path();
        let file_name = entry
            .file_name()
            .to_string_lossy()
            .to_string();
        let relative = format!("{}/{}", relative_prefix, file_name);

        if entry_path.is_dir() {
            enumerate_dir_recursive(&entry_path, &relative, results);
        } else if entry_path.is_file() {
            if let Ok(meta) = std::fs::metadata(&entry_path) {
                results.push((
                    entry_path.to_string_lossy().to_string(),
                    relative,
                    meta.len(),
                ));
            }
        }
    }
}

/// 从剪贴板文件路径列表构建批量发送信息
/// 返回 (display_name, file_items, outgoing_items, total_size)
fn build_batch_file_info(
    paths: &[String],
) -> Option<(String, Vec<FileItemInfo>, Vec<OutgoingFileItem>, u64)> {
    let mut file_items: Vec<FileItemInfo> = Vec::new();
    let mut outgoing_items: Vec<OutgoingFileItem> = Vec::new();
    let mut total_size: u64 = 0;

    for path_str in paths {
        let path = Path::new(path_str);
        if path.is_dir() {
            // 文件夹：递归枚举
            let dir_name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "folder".to_string());
            let entries = enumerate_directory(path, &dir_name);
            for (abs_path, rel_path, size) in entries {
                file_items.push(FileItemInfo {
                    relative_path: rel_path.clone(),
                    file_size: size,
                });
                outgoing_items.push(OutgoingFileItem {
                    absolute_path: abs_path,
                    relative_path: rel_path,
                    file_size: size,
                });
                total_size += size;
            }
        } else if path.is_file() {
            // 单个文件
            let file_name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "unknown".to_string());
            let size = get_file_size(path_str).unwrap_or(0);
            file_items.push(FileItemInfo {
                relative_path: file_name.clone(),
                file_size: size,
            });
            outgoing_items.push(OutgoingFileItem {
                absolute_path: path_str.clone(),
                relative_path: file_name,
                file_size: size,
            });
            total_size += size;
        }
    }

    if file_items.is_empty() {
        return None;
    }

    // 生成显示名称
    let display_name = if paths.len() == 1 {
        let p = Path::new(&paths[0]);
        if p.is_dir() {
            // 单个文件夹
            p.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "folder".to_string())
        } else {
            // 单个文件
            p.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "file".to_string())
        }
    } else {
        format!("{}个文件", paths.len())
    };

    Some((display_name, file_items, outgoing_items, total_size))
}

/// 对文件路径列表做哈希，用于去重
fn hash_paths(paths: &[String]) -> u64 {
    let mut hasher = DefaultHasher::new();
    let lower: Vec<String> = paths.iter().map(|p| p.to_lowercase()).collect();
    lower.hash(&mut hasher);
    hasher.finish()
}

/// 带超时保护的剪贴板文本读取（防止 arboard 挂起卡死 monitor_loop）
async fn safe_get_clipboard_text() -> Result<Option<String>, String> {
    let result = tokio::time::timeout(
        Duration::from_secs(3),
        tokio::task::spawn_blocking(|| {
            match arboard::Clipboard::new() {
                Ok(mut clipboard) => {
                    match clipboard.get_text() {
                        Ok(text) => Ok(Some(text)),
                        Err(arboard::Error::ContentNotAvailable) => Ok(None),
                        Err(e) => Err(format!("{}", e)),
                    }
                }
                Err(e) => Err(format!("Clipboard::new failed: {}", e)),
            }
        }),
    )
    .await;

    match result {
        Ok(Ok(inner)) => inner,
        Ok(Err(join_err)) => Err(format!("spawn_blocking failed: {}", join_err)),
        Err(_) => Err("Clipboard read timed out (3s)".to_string()),
    }
}

/// 轮询监听剪贴板变化
async fn monitor_loop(state: Arc<AppState>) {
    let mut last_text = String::new();
    let mut last_file_hash: u64 = 0;
    let mut last_seq: u32 = clipboard_sequence();
    #[cfg(target_os = "macos")]
    let mut last_change_count: i64 = pasteboard_change_count();
    let mut ticker = interval(Duration::from_millis(POLL_INTERVAL_MS));
    // 连续失败计数器，防止永久重试不可读内容
    let mut fail_count: u32 = 0;
    const MAX_RETRIES: u32 = 5;
    // is_writing_clipboard 超时保护：记录开始写入的时间戳
    let mut writing_since: Option<std::time::Instant> = None;
    // 循环健康状态追踪
    let mut loop_healthy_count: u64 = 0;
    const WRITING_TIMEOUT_SECS: u64 = 10;

    log::info!("Clipboard monitor started");

    loop {
        ticker.tick().await;

        // 如果同步已暂停，跳过本轮
        if state.is_paused.load(Ordering::SeqCst) {
            continue;
        }

        // is_writing_clipboard 超时保护：如果写入标志超过 WRITING_TIMEOUT_SECS 秒，强制重置
        if state.is_writing_clipboard.load(Ordering::SeqCst) {
            match writing_since {
                Some(start) if start.elapsed().as_secs() > WRITING_TIMEOUT_SECS => {
                    log::warn!(
                        "is_writing_clipboard stuck for >{}s, force resetting",
                        WRITING_TIMEOUT_SECS
                    );
                    state.is_writing_clipboard.store(false, Ordering::SeqCst);
                    writing_since = None;
                }
                None => {
                    writing_since = Some(std::time::Instant::now());
                }
                _ => {
                    // 仍在超时窗口内，正常跳过
                    continue;
                }
            }
        } else {
            writing_since = None;
        }

        // 如果正在写入剪贴板，跳过本轮
        if state.is_writing_clipboard.load(Ordering::SeqCst) {
            continue;
        }

        // 定期健康日志（每 300 次循环约 1 分钟输出一次）
        loop_healthy_count += 1;
        if loop_healthy_count % 300 == 0 {
            log::info!(
                "Clipboard monitor healthy: {} ticks, last_change_count={}, last_text_len={}",
                loop_healthy_count,
                {
                    #[cfg(target_os = "macos")]
                    { last_change_count }
                    #[cfg(not(target_os = "macos"))]
                    { 0i64 }
                },
                last_text.len()
            );
        }

        // 检测剪贴板是否变化
        // Windows: 使用剪贴板序列号（高效）
        // macOS: 使用 NSPasteboard.changeCount（轻量级 ObjC FFI，不读取内容）
        // 注意：不立即更新 last_seq/last_change_count，等内容成功处理后再更新
        let (clipboard_changed, current_seq_val) = if cfg!(target_os = "windows") {
            let current_seq = clipboard_sequence();
            let changed = current_seq != last_seq;
            (changed, current_seq as i64)
        } else {
            #[cfg(target_os = "macos")]
            {
                let current_count = pasteboard_change_count();
                if current_count < 0 {
                    log::warn!("pasteboard_change_count returned {}, skipping", current_count);
                    continue;
                }
                let changed = current_count != last_change_count;
                (changed, current_count)
            }
            #[cfg(not(target_os = "macos"))]
            {
                // 其他平台 fallback
                if let Ok(mut clipboard) = arboard::Clipboard::new() {
                    match clipboard.get_text() {
                        Ok(text) => (text != last_text, 0i64),
                        Err(_) => (last_file_hash == 0, 0i64),
                    }
                } else {
                    (false, 0i64)
                }
            }
        };

        if !clipboard_changed {
            continue;
        }

        log::debug!(
            "Clipboard changed detected: seq_val={}, fail_count={}",
            current_seq_val,
            fail_count
        );

        // 剪贴板变化了，尝试处理内容
        let mut processed = false;

        // 优先检测文件
        if let Some(file_paths) = get_clipboard_files().await {
            let file_hash = hash_paths(&file_paths);

            // 检查是否是已写入的占位文件（B电脑收到file-start后写入剪贴板的）
            let placeholder_hash =
                state.written_placeholder_hash.load(Ordering::SeqCst);
            if placeholder_hash != 0 && file_hash == placeholder_hash {
                log::info!("Skipping placeholder file (hash match)");
                last_file_hash = file_hash;
                processed = true;
            } else {
                // 过滤掉 clipsync 目录下的占位文件
                let real_paths: Vec<String> = file_paths
                    .iter()
                    .filter(|p| !is_placeholder_file(p))
                    .cloned()
                    .collect();

                if real_paths.is_empty() {
                    last_file_hash = file_hash;
                    processed = true;
                } else if file_hash != last_file_hash {
                    last_file_hash = file_hash;
                    state
                        .written_placeholder_hash
                        .store(0, Ordering::SeqCst);

                    log::info!("Files detected: {} path(s)", real_paths.len());

                    if let Some((display_name, file_items, outgoing_items, total_size)) =
                        build_batch_file_info(&real_paths)
                    {
                        let transfer_id = uuid::Uuid::new_v4().to_string();

                        {
                            let mut outgoing =
                                state.outgoing_files.lock().await;
                            outgoing.insert(
                                transfer_id.clone(),
                                OutgoingFile {
                                    files: outgoing_items,
                                    total_size,
                                },
                            );
                        }

                        let file_start_data = FileStartData {
                            transfer_id: transfer_id.clone(),
                            display_name,
                            files: file_items,
                            total_size,
                        };
                        broadcast_file_metadata(
                            state.clone(),
                            file_start_data,
                        )
                        .await;
                    }
                    // 同步 last_text，防止下一轮 arboard 返回文件名时被当作文本广播
                    if let Ok(Some(t)) = safe_get_clipboard_text().await {
                        last_text = t;
                    }
                    processed = true;
                } else {
                    // 文件哈希未变（重复检测）
                    processed = true;
                }
            }
        }

        // 没有文件则检测文本
        if !processed {
            match safe_get_clipboard_text().await {
                Ok(Some(text)) => {
                    if !text.is_empty() && text != last_text {
                        // 检查是否是远程收到的文本（防止回弹广播）
                        let is_remote = {
                            let mut hasher = DefaultHasher::new();
                            text.hash(&mut hasher);
                            let text_hash = hasher.finish();
                            let remote_hash = state
                                .last_remote_text_hash
                                .load(Ordering::SeqCst);
                            remote_hash != 0 && text_hash == remote_hash
                        };

                        if is_remote {
                            last_text = text;
                            processed = true;
                        } else {
                            last_text = text.clone();
                            last_file_hash = 0;
                            state
                                .written_placeholder_hash
                                .store(0, Ordering::SeqCst);
                            state
                                .last_remote_text_hash
                                .store(0, Ordering::SeqCst);
                            let preview: String = text.chars().take(30).collect();
                            log::info!(
                                "Clipboard text: {}...",
                                preview
                            );
                            crate::transport::broadcast_clipboard(
                                state.clone(),
                                text,
                            )
                            .await;
                            processed = true;
                        }
                    } else {
                        // 文本未变或为空，可能是非文本内容（图片等）
                        processed = true;
                    }
                }
                Ok(None) => {
                    // ContentNotAvailable - 非文本内容
                    processed = true;
                }
                Err(e) => {
                    log::warn!("Failed to read clipboard text: {}", e);
                    // 读取失败，不标记 processed，下次重试
                }
            }
        }

        // 只有成功处理后才更新序列号，否则下次重试
        if processed {
            fail_count = 0;
            if cfg!(target_os = "windows") {
                last_seq = current_seq_val as u32;
            } else {
                #[cfg(target_os = "macos")]
                {
                    last_change_count = current_seq_val;
                }
            }
        } else {
            fail_count += 1;
            if fail_count >= MAX_RETRIES {
                // 超过最大重试次数，放弃本次变化，避免无限循环
                log::warn!("Clipboard read failed {} times, skipping this change", fail_count);
                fail_count = 0;
                if cfg!(target_os = "windows") {
                    last_seq = current_seq_val as u32;
                } else {
                    #[cfg(target_os = "macos")]
                    {
                        last_change_count = current_seq_val;
                    }
                }
            } else {
                log::debug!("Clipboard read failed, will retry (attempt {}/{})", fail_count, MAX_RETRIES);
            }
        }
    }
}
