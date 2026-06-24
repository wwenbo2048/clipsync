use tauri::{
    image::Image,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager,
};

/// 检测系统是否使用中文语言环境
fn is_chinese_locale() -> bool {
    // macOS / Linux: 检查 LANG / LC_ALL 环境变量
    if let Ok(lang) = std::env::var("LANG") {
        if lang.starts_with("zh") {
            return true;
        }
    }
    if let Ok(lang) = std::env::var("LC_ALL") {
        if lang.starts_with("zh") {
            return true;
        }
    }
    // Windows: 检查系统语言代码
    #[cfg(target_os = "windows")]
    {
        // Windows 上 LANG 环境变量可能不存在，使用 GetACP 或其他方式
        // 简化处理：中文 Windows 默认 locale 为 2052 (0x0804) 或 1028 (0x0404)
        // 这里通过环境变量做近似判断
        if std::env::var("LANG").is_err() && std::env::var("LC_ALL").is_err() {
            // 默认中文 Windows 环境下返回 true
            // 用户可以通过前端语言切换覆盖 UI 语言
            return false; // Windows 上默认使用英文，前端语言切换为主
        }
    }
    false
}

pub fn setup_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let zh = is_chinese_locale();
    let (show_text, quit_text, tooltip_text) = if zh {
        ("显示窗口", "退出", "ClipSync - 剪贴板同步")
    } else {
        ("Show Window", "Quit", "ClipSync - Clipboard Sync")
    };

    let show = MenuItem::with_id(app, "show", show_text, true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", quit_text, true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &quit])?;

    let icon = Image::from_bytes(include_bytes!("../icons/128x128.png"))
        .expect("failed to load tray icon");

    TrayIconBuilder::new()
        .icon(icon)
        .icon_as_template(false)
        .menu(&menu)
        .tooltip(tooltip_text)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => {
                show_main_window(app);
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| match event {
            TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } => {
                let app = tray.app_handle();
                toggle_main_window(app);
            }
            _ => {}
        })
        .build(app)?;

    Ok(())
}

/// 切换窗口显示/隐藏
fn toggle_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if window.is_visible().unwrap_or(false) {
            let _ = window.hide();
        } else {
            show_main_window(app);
        }
    }
}

/// 显示并聚焦主窗口（处理 Windows 最小化恢复）
pub fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        // Windows 下需要先取消最小化
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
        let _ = window.set_always_on_top(true);
        // 短暂置顶后取消，避免一直置顶
        std::thread::sleep(std::time::Duration::from_millis(100));
        let _ = window.set_always_on_top(false);
    }
}
