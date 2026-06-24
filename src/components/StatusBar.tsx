import { StatusInfo } from "../App";
import { useI18n } from "../i18n/I18nContext";

interface StatusBarProps {
  status: StatusInfo | null;
  isSyncConnected: boolean;
  onToggleSync: () => void;
}

export function StatusBar({ status, isSyncConnected, onToggleSync }: StatusBarProps) {
  const { t } = useI18n();

  if (!status) {
    return (
      <div className="status-bar">
        <div className="status-bar-content">
          <span className="status-dot offline" />
          <span className="status-text">{t.status_starting}</span>
        </div>
      </div>
    );
  }

  const isConnected = isSyncConnected && status.connected_devices > 0;

  return (
    <div className="status-bar">
      <div className="status-bar-content">
        <div className="status-left">
          <span className={`status-dot ${!isSyncConnected ? "paused" : isConnected ? "online" : "offline"}`} />
          <span className="status-text">{status.local_name}</span>
          {!isSyncConnected && <span className="status-paused-badge">{t.status_disconnected}</span>}
        </div>
        <div className="status-right">
          <button
            className={`sync-toggle-btn ${isSyncConnected ? "connected" : "disconnected"}`}
            onClick={onToggleSync}
            title={isSyncConnected ? t.status_toggle_disconnect : t.status_toggle_connect}
          >
            {isSyncConnected ? t.status_btn_disconnect : t.status_btn_connect}
          </button>
          <span className="status-badge">
            {status.connected_devices}/{status.total_devices} {t.status_connected}
          </span>
        </div>
      </div>
      <div className="status-ips">
        {status.local_ips.length > 0 ? (
          status.local_ips.map((ip, i) => (
            <span key={ip} className="status-ip">
              {ip}:{status.local_port}
              {i < status.local_ips.length - 1 ? " | " : ""}
            </span>
          ))
        ) : (
          <span className="status-ip">{t.status_no_interface}</span>
        )}
      </div>
    </div>
  );
}
