export type Language = "zh" | "en" | "ja" | "ko" | "de" | "fr" | "es" | "pt" | "ru";

/** 语言元信息（用于下拉选择器和 locale 映射） */
export const languageMeta: Record<Language, { nativeName: string; locale: string }> = {
  zh: { nativeName: "中文", locale: "zh-CN" },
  en: { nativeName: "English", locale: "en-US" },
  ja: { nativeName: "日本語", locale: "ja-JP" },
  ko: { nativeName: "한국어", locale: "ko-KR" },
  de: { nativeName: "Deutsch", locale: "de-DE" },
  fr: { nativeName: "Français", locale: "fr-FR" },
  es: { nativeName: "Español", locale: "es-ES" },
  pt: { nativeName: "Português", locale: "pt-BR" },
  ru: { nativeName: "Русский", locale: "ru-RU" },
};

export interface Translation {
  // App tabs
  tab_history: string;
  tab_devices: string;
  tab_network: string;
  tab_settings: string;

  // StatusBar
  status_starting: string;
  status_disconnected: string;
  status_connected: string;
  status_no_interface: string;
  status_toggle_connect: string;
  status_toggle_disconnect: string;
  status_btn_connect: string;
  status_btn_disconnect: string;

  // DeviceList
  device_no_devices: string;
  device_no_devices_hint: string;
  device_status_connected: string;
  device_status_connecting: string;
  device_status_offline: string;

  // ClipHistory
  history_empty: string;
  history_empty_hint: string;
  history_count: (n: number) => string;
  history_clear: string;
  history_download: string;
  history_downloading: string;
  history_open_folder: string;
  history_local: string;
  history_copy: string;
  history_image: string;

  // NetworkSelector
  network_no_interfaces: string;
  network_auto_broadcast: string;
  network_physical: string;
  network_virtual: string;
  network_broadcast: string;

  // Settings
  settings_loading: string;
  settings_download_dir: string;
  settings_download_dir_desc: string;
  settings_download_dir_placeholder: string;
  settings_save: string;
  settings_saving: string;
  settings_cache_management: string;
  settings_cache_size: string;
  settings_clear_cache: string;
  settings_clearing: string;
  settings_shortcut: string;
  settings_shortcut_desc: string;
  settings_shortcut_placeholder: string;
  settings_shortcut_format: string;
  settings_autostart: string;
  settings_autostart_desc: string;
  settings_autostart_enable: string;
  settings_autostart_disable: string;
  settings_autostart_setting: string;
  settings_autostart_enabled: string;
  settings_log: string;
  settings_log_desc: string;
  settings_open_log: string;
  settings_version: string;
  settings_version_unknown: string;
  settings_path_empty: string;
  settings_save_success: string;
  settings_save_failed: string;
  settings_clear_success: (size: string) => string;
  settings_clear_failed: string;
  settings_shortcut_saved: string;
  settings_autostart_on: string;
  settings_autostart_off: string;
  settings_autostart_failed: string;
  settings_open_log_failed: string;

  // Language switcher
  lang_section: string;
  lang_desc: string;
}

// Re-export all language packs
export { zh } from "./lang/zh";
export { en } from "./lang/en";
export { ja } from "./lang/ja";
export { ko } from "./lang/ko";
export { de } from "./lang/de";
export { fr } from "./lang/fr";
export { es } from "./lang/es";
export { pt } from "./lang/pt";
export { ru } from "./lang/ru";

import { zh } from "./lang/zh";
import { en } from "./lang/en";
import { ja } from "./lang/ja";
import { ko } from "./lang/ko";
import { de } from "./lang/de";
import { fr } from "./lang/fr";
import { es } from "./lang/es";
import { pt } from "./lang/pt";
import { ru } from "./lang/ru";

export const translations: Record<Language, Translation> = {
  zh, en, ja, ko, de, fr, es, pt, ru,
};
