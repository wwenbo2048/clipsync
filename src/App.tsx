import { useEffect, useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { StatusBar } from "./components/StatusBar";
import { DeviceList } from "./components/DeviceList";
import { ClipHistory } from "./components/ClipHistory";
import { NetworkSelector } from "./components/NetworkSelector";
import { Settings } from "./components/Settings";
import { useI18n } from "./i18n/I18nContext";

export interface DeviceInfo {
  id: string;
  name: string;
  ip: string;
  port: number;
  status: "Connected" | "Connecting" | "Offline";
  last_seen: number;
}

export interface ClipboardEntry {
  id: string;
  text: string;
  timestamp: number;
  source: string;
  content_type: string;
  file_path: string | null;
  file_size: number | null;
  download_status: string | null;
  transfer_id: string | null;
}

export interface StatusInfo {
  local_ips: string[];
  local_name: string;
  local_port: number;
  interface_names: string[];
  connected_devices: number;
  total_devices: number;
}

export interface NetworkInterface {
  name: string;
  ip: string;
  netmask: string;
  broadcast: string;
  is_loopback: boolean;
  is_virtual: boolean;
}

function App() {
  const { t } = useI18n();
  const [devices, setDevices] = useState<DeviceInfo[]>([]);
  const [history, setHistory] = useState<ClipboardEntry[]>([]);
  const [status, setStatus] = useState<StatusInfo | null>(null);
  const [interfaces, setInterfaces] = useState<NetworkInterface[]>([]);
  const [activeTab, setActiveTab] = useState<"history" | "devices" | "network" | "settings">("history");
  const [isSyncConnected, setIsSyncConnected] = useState(true);

  const refreshStatus = useCallback(async () => {
    try {
      const s = await invoke<StatusInfo>("get_status");
      setStatus(s);
    } catch (e) {
      console.error("Failed to get status:", e);
    }
  }, []);

  const refreshDevices = useCallback(async () => {
    try {
      const d = await invoke<DeviceInfo[]>("get_devices");
      setDevices(d);
    } catch (e) {
      console.error("Failed to get devices:", e);
    }
  }, []);

  const refreshHistory = useCallback(async () => {
    try {
      const h = await invoke<ClipboardEntry[]>("get_clipboard_history");
      setHistory(h);
    } catch (e) {
      console.error("Failed to get history:", e);
    }
  }, []);

  const refreshInterfaces = useCallback(async () => {
    try {
      const i = await invoke<NetworkInterface[]>("get_interfaces");
      setInterfaces(i);
    } catch (e) {
      console.error("Failed to get interfaces:", e);
    }
  }, []);

  useEffect(() => {
    refreshStatus();
    refreshDevices();
    refreshHistory();
    refreshInterfaces();

    // 初始化同步状态
    invoke<boolean>("get_sync_status").then(setIsSyncConnected).catch(console.error);

    const interval = setInterval(() => {
      refreshStatus();
      refreshDevices();
    }, 3000);

    const unlistenDevices = listen<DeviceInfo[]>("devices-changed", (event) => {
      setDevices(event.payload);
      refreshStatus();
    });

    const unlistenClipboard = listen("clipboard-received", () => {
      refreshHistory();
    });

    const unlistenFileDone = listen("file-download-complete", () => {
      refreshHistory();
    });

    const unlistenSyncStatus = listen<boolean>("sync-status-changed", (event) => {
      setIsSyncConnected(event.payload);
    });

    return () => {
      clearInterval(interval);
      unlistenDevices.then((f: () => void) => f());
      unlistenClipboard.then((f: () => void) => f());
      unlistenFileDone.then((f: () => void) => f());
      unlistenSyncStatus.then((f: () => void) => f());
    };
  }, [refreshStatus, refreshDevices, refreshHistory, refreshInterfaces]);

  const handleToggleSync = async () => {
    try {
      if (isSyncConnected) {
        await invoke("disconnect_sync");
        setIsSyncConnected(false);
      } else {
        await invoke("connect_sync");
        setIsSyncConnected(true);
      }
    } catch (e) {
      console.error("Failed to toggle sync:", e);
    }
  };

  const handleClearHistory = async () => {
    try {
      await invoke("clear_history");
      setHistory([]);
    } catch (e) {
      console.error("Failed to clear history:", e);
    }
  };

  const handleCopyToClipboard = async (text: string) => {
    try {
      await navigator.clipboard.writeText(text);
    } catch (e) {
      console.error("Failed to copy:", e);
    }
  };

  const handleDownloadFile = async (transferId: string) => {
    try {
      await invoke("request_file_download", { transferId });
      refreshHistory();
    } catch (e) {
      console.error("Failed to request download:", e);
    }
  };

  const handleOpenFolder = async (filePath: string) => {
    try {
      await invoke("open_file_location", { filePath });
    } catch (e) {
      console.error("Failed to open folder:", e);
    }
  };

  return (
    <div className="app-container">
      <StatusBar status={status} isSyncConnected={isSyncConnected} onToggleSync={handleToggleSync} />

      <div className="tab-bar">
        <button
          className={`tab-btn ${activeTab === "history" ? "active" : ""}`}
          onClick={() => setActiveTab("history")}
        >
          {t.tab_history}
        </button>
        <button
          className={`tab-btn ${activeTab === "devices" ? "active" : ""}`}
          onClick={() => setActiveTab("devices")}
        >
          {t.tab_devices.replace("{count}", String(devices.length))}
        </button>
        <button
          className={`tab-btn ${activeTab === "network" ? "active" : ""}`}
          onClick={() => setActiveTab("network")}
        >
          {t.tab_network}
        </button>
        <button
          className={`tab-btn ${activeTab === "settings" ? "active" : ""}`}
          onClick={() => setActiveTab("settings")}
        >
          {t.tab_settings}
        </button>
      </div>

      <div className="tab-content">
        {activeTab === "history" ? (
          <ClipHistory
            entries={history}
            onCopy={handleCopyToClipboard}
            onClear={handleClearHistory}
            onDownload={handleDownloadFile}
            onOpenFolder={handleOpenFolder}
          />
        ) : activeTab === "devices" ? (
          <DeviceList devices={devices} />
        ) : activeTab === "settings" ? (
          <Settings />
        ) : (
          <NetworkSelector interfaces={interfaces} />
        )}
      </div>
    </div>
  );
}

export default App;
