import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useI18n } from "../i18n/I18nContext";
import { languageMeta, Language } from "../i18n/translations";

interface SettingsInfo {
  download_dir: string;
  cache_size: number;
  shortcut: string;
  autostart: boolean;
}

export function Settings() {
  const { t, lang, setLang } = useI18n();
  const [settings, setSettings] = useState<SettingsInfo | null>(null);
  const [editPath, setEditPath] = useState("");
  const [editShortcut, setEditShortcut] = useState("");
  const [saving, setSaving] = useState(false);
  const [savingShortcut, setSavingShortcut] = useState(false);
  const [clearing, setClearing] = useState(false);
  const [togglingAutostart, setTogglingAutostart] = useState(false);
  const [langOpen, setLangOpen] = useState(false);
  const [message, setMessage] = useState<{ type: "success" | "error"; text: string } | null>(null);
  const [version, setVersion] = useState("");
  const langListRef = useRef<HTMLDivElement>(null);

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

  useEffect(() => {
    if (langOpen && langListRef.current) {
      langListRef.current.scrollIntoView({ behavior: "smooth", block: "end" });
    }
  }, [langOpen]);

  const showMessage = (type: "success" | "error", text: string) => {
    setMessage({ type, text });
    setTimeout(() => setMessage(null), 3000);
  };

  const handleSavePath = async () => {
    if (!editPath.trim()) {
      showMessage("error", t.settings_path_empty);
      return;
    }
    setSaving(true);
    try {
      await invoke("set_download_dir", { path: editPath.trim() });
      showMessage("success", t.settings_save_success);
      loadSettings();
    } catch (e: any) {
      showMessage("error", e?.toString() || t.settings_save_failed);
    }
    setSaving(false);
  };

  const handleClearCache = async () => {
    setClearing(true);
    try {
      const cleared = await invoke<number>("clear_cache");
      showMessage("success", t.settings_clear_success(formatSize(cleared)));
      loadSettings();
    } catch (e: any) {
      showMessage("error", e?.toString() || t.settings_clear_failed);
    }
    setClearing(false);
  };

  const handleSaveShortcut = async () => {
    setSavingShortcut(true);
    try {
      await invoke("set_shortcut", { shortcut: editShortcut.trim() });
      showMessage("success", t.settings_shortcut_saved);
      loadSettings();
    } catch (e: any) {
      showMessage("error", e?.toString() || t.settings_save_failed);
    }
    setSavingShortcut(false);
  };

  const handleToggleAutostart = async () => {
    if (!settings) return;
    const next = !settings.autostart;
    setTogglingAutostart(true);
    try {
      await invoke("set_autostart", { enabled: next });
      showMessage("success", next ? t.settings_autostart_on : t.settings_autostart_off);
      loadSettings();
    } catch (e: any) {
      showMessage("error", e?.toString() || t.settings_autostart_failed);
    }
    setTogglingAutostart(false);
  };

  const handleOpenLogDir = async () => {
    try {
      await invoke("open_log_dir");
    } catch (e: any) {
      showMessage("error", e?.toString() || t.settings_open_log_failed);
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
        <p>{t.settings_loading}</p>
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
        <div className="settings-label">{t.settings_download_dir}</div>
        <div className="settings-desc">{t.settings_download_dir_desc}</div>
        <div className="settings-input-row">
          <input
            type="text"
            className="settings-input"
            value={editPath}
            onChange={(e) => setEditPath(e.target.value)}
            placeholder={t.settings_download_dir_placeholder}
          />
        </div>
        <button
          className="settings-btn primary"
          onClick={handleSavePath}
          disabled={saving || editPath === settings.download_dir}
        >
          {saving ? t.settings_saving : t.settings_save}
        </button>
      </div>

      <div className="settings-section">
        <div className="settings-label">{t.settings_cache_management}</div>
        <div className="settings-desc">
          {t.settings_cache_size}: <strong>{formatSize(settings.cache_size)}</strong>
        </div>
        <button
          className="settings-btn danger"
          onClick={handleClearCache}
          disabled={clearing || settings.cache_size === 0}
        >
          {clearing ? t.settings_clearing : t.settings_clear_cache}
        </button>
      </div>

      <div className="settings-section">
        <div className="settings-label">{t.settings_shortcut}</div>
        <div className="settings-desc">{t.settings_shortcut_desc}</div>
        <div className="settings-input-row">
          <input
            type="text"
            className="settings-input"
            value={editShortcut}
            onChange={(e) => setEditShortcut(e.target.value)}
            placeholder={t.settings_shortcut_placeholder}
          />
        </div>
        <div className="settings-desc" style={{ marginBottom: 8, opacity: 0.7 }}>
          {t.settings_shortcut_format}
        </div>
        <button
          className="settings-btn primary"
          onClick={handleSaveShortcut}
          disabled={savingShortcut || editShortcut === settings.shortcut}
        >
          {savingShortcut ? t.settings_saving : t.settings_save}
        </button>
      </div>

      <div className="settings-section">
        <div className="settings-label">{t.settings_autostart}</div>
        <div className="settings-desc">{t.settings_autostart_desc}</div>
        <button
          className={`settings-btn ${settings.autostart ? "danger" : "primary"}`}
          onClick={handleToggleAutostart}
          disabled={togglingAutostart}
        >
          {togglingAutostart
            ? t.settings_autostart_setting
            : settings.autostart
            ? t.settings_autostart_disable
            : t.settings_autostart_enable}
        </button>
        {settings.autostart && (
          <div className="settings-desc" style={{ marginTop: 6, opacity: 0.7 }}>
            {t.settings_autostart_enabled}
          </div>
        )}
      </div>

      <div className="settings-section">
        <div className="settings-label">{t.settings_log}</div>
        <div className="settings-desc">{t.settings_log_desc}</div>
        <button
          className="settings-btn primary"
          onClick={handleOpenLogDir}
        >
          {t.settings_open_log}
        </button>
      </div>

      <div className="settings-section settings-version">
        <div className="settings-version-label">{t.settings_version}</div>
        <div className="settings-version-value">v{version || t.settings_version_unknown}</div>
      </div>

      <div className="settings-section">
        <div className="settings-label">{t.lang_section}</div>
        <div className="settings-desc">{t.lang_desc}</div>
        <div className="lang-select-wrapper">
          <button
            className={`lang-select-trigger ${langOpen ? "open" : ""}`}
            onClick={() => setLangOpen((v) => !v)}
          >
            <svg className="lang-globe" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
              <circle cx="12" cy="12" r="10" />
              <line x1="2" y1="12" x2="22" y2="12" />
              <path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z" />
            </svg>
            <span className="lang-current-name">{languageMeta[lang].nativeName}</span>
            <svg className={`lang-chevron ${langOpen ? "up" : ""}`} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
              <polyline points="6 9 12 15 18 9" />
            </svg>
          </button>
          {langOpen && (
            <div className="lang-select-list" ref={langListRef}>
              {(Object.keys(languageMeta) as Language[]).map((code) => (
                <button
                  key={code}
                  className={`lang-option ${code === lang ? "active" : ""}`}
                  onClick={() => {
                    setLang(code);
                    setLangOpen(false);
                  }}
                >
                  <span className="lang-option-name">{languageMeta[code].nativeName}</span>
                  {code === lang && (
                    <svg className="lang-check" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                      <polyline points="20 6 9 17 4 12" />
                    </svg>
                  )}
                </button>
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
