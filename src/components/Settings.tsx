import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";

interface SettingsInfo {
  download_dir: string;
  cache_size: number;
  shortcut: string;
  autostart: boolean;
}

export function Settings() {
  const [settings, setSettings] = useState<SettingsInfo | null>(null);
  const [editPath, setEditPath] = useState("");
  const [editShortcut, setEditShortcut] = useState("");
  const [saving, setSaving] = useState(false);
  const [savingShortcut, setSavingShortcut] = useState(false);
  const [clearing, setClearing] = useState(false);
  const [togglingAutostart, setTogglingAutostart] = useState(false);
  const [message, setMessage] = useState<{ type: "success" | "error"; text: string } | null>(null);
  const [version, setVersion] = useState("");

  const loadSettings = useCallback(async () => {
    try {
      const s = await invoke<SettingsInfo>("get_settings");
      setSettings(s);
      setEditPath(s.download_dir);
      setEditShortcut(s.shortcut);
    } catch (e) {
      console.error("Failed to load settings:", e);
    }
    try {
      const v = await invoke<string>("get_version");
      setVersion(v);
    } catch (e) {
      console.error("Failed to load version:", e);
    }
  }, []);

  useEffect(() => {
    loadSettings();
  }, [loadSettings]);

  const showMessage = (type: "success" | "error", text: string) => {
    setMessage({ type, text });
    setTimeout(() => setMessage(null), 3000);
  };

  const handleSavePath = async () => {
    if (!editPath.trim()) {
      showMessage("error", "路径不能为空");
      return;
    }
    setSaving(true);
    try {
      await invoke("set_download_dir", { path: editPath.trim() });
      showMessage("success", "保存成功");
      loadSettings();
    } catch (e: any) {
      showMessage("error", e?.toString() || "保存失败");
    }
    setSaving(false);
  };

  const handleClearCache = async () => {
    setClearing(true);
    try {
      const cleared = await invoke<number>("clear_cache");
      showMessage("success", `已清除 ${formatSize(cleared)} 缓存`);
      loadSettings();
    } catch (e: any) {
      showMessage("error", e?.toString() || "清除失败");
    }
    setClearing(false);
  };

  const handleSaveShortcut = async () => {
    setSavingShortcut(true);
    try {
      await invoke("set_shortcut", { shortcut: editShortcut.trim() });
      showMessage("success", "快捷键已保存");
      loadSettings();
    } catch (e: any) {
      showMessage("error", e?.toString() || "保存失败");
    }
    setSavingShortcut(false);
  };

  const handleToggleAutostart = async () => {
    if (!settings) return;
    const next = !settings.autostart;
    setTogglingAutostart(true);
    try {
      await invoke("set_autostart", { enabled: next });
      showMessage("success", next ? "已开启开机自动启动" : "已关闭开机自动启动");
      loadSettings();
    } catch (e: any) {
      showMessage("error", e?.toString() || "设置失败");
    }
    setTogglingAutostart(false);
  };

  const handleOpenLogDir = async () => {
    try {
      await invoke("open_log_dir");
    } catch (e: any) {
      showMessage("error", e?.toString() || "打开日志目录失败");
    }
  };

  const formatSize = (bytes: number) => {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
    return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
  };

  if (!settings) {
    return (
      <div className="empty-state">
        <p>加载设置中...</p>
      </div>
    );
  }

  return (
    <div className="settings-panel">
      {message && (
        <div className={`settings-message ${message.type}`}>
          {message.text}
        </div>
      )}

      <div className="settings-section">
        <div className="settings-label">文件下载位置</div>
        <div className="settings-desc">接收的文件将保存到此目录</div>
        <div className="settings-input-row">
          <input
            type="text"
            className="settings-input"
            value={editPath}
            onChange={(e) => setEditPath(e.target.value)}
            placeholder="输入下载目录路径"
          />
        </div>
        <button
          className="settings-btn primary"
          onClick={handleSavePath}
          disabled={saving || editPath === settings.download_dir}
        >
          {saving ? "保存中..." : "保存"}
        </button>
      </div>

      <div className="settings-section">
        <div className="settings-label">缓存管理</div>
        <div className="settings-desc">
          临时缓存大小: <strong>{formatSize(settings.cache_size)}</strong>
        </div>
        <button
          className="settings-btn danger"
          onClick={handleClearCache}
          disabled={clearing || settings.cache_size === 0}
        >
          {clearing ? "清除中..." : "清除缓存"}
        </button>
      </div>

      <div className="settings-section">
        <div className="settings-label">全局快捷键</div>
        <div className="settings-desc">按下快捷键快速打开/隐藏剪贴板历史窗口</div>
        <div className="settings-input-row">
          <input
            type="text"
            className="settings-input"
            value={editShortcut}
            onChange={(e) => setEditShortcut(e.target.value)}
            placeholder="例如: CmdOrCtrl+Shift+V"
          />
        </div>
        <div className="settings-desc" style={{ marginBottom: 8, opacity: 0.7 }}>
          支持: Ctrl, Shift, Alt, Super, CmdOrCtrl + 字母/数字/F1-F12
        </div>
        <button
          className="settings-btn primary"
          onClick={handleSaveShortcut}
          disabled={savingShortcut || editShortcut === settings.shortcut}
        >
          {savingShortcut ? "保存中..." : "保存"}
        </button>
      </div>

      <div className="settings-section">
        <div className="settings-label">开机自动启动</div>
        <div className="settings-desc">启动电脑后自动运行 ClipSync，在后台提供剪贴板同步服务</div>
        <button
          className={`settings-btn ${settings.autostart ? "danger" : "primary"}`}
          onClick={handleToggleAutostart}
          disabled={togglingAutostart}
        >
          {togglingAutostart
            ? "设置中..."
            : settings.autostart
            ? "关闭自动启动"
            : "开启自动启动"}
        </button>
        {settings.autostart && (
          <div className="settings-desc" style={{ marginTop: 6, opacity: 0.7 }}>
            ✓ 当前已开启
          </div>
        )}
      </div>

      <div className="settings-section">
        <div className="settings-label">日志管理</div>
        <div className="settings-desc">运行日志用于排查剪贴板同步异常问题</div>
        <button
          className="settings-btn primary"
          onClick={handleOpenLogDir}
        >
          打开日志目录
        </button>
      </div>

      <div className="settings-section settings-version">
        <div className="settings-version-label">当前版本</div>
        <div className="settings-version-value">v{version || "未知"}</div>
      </div>
    </div>
  );
}
