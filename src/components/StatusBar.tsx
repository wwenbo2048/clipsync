import { StatusInfo } from "../App";

interface StatusBarProps {
  status: StatusInfo | null;
  isSyncConnected: boolean;
  onToggleSync: () => void;
}

export function StatusBar({ status, isSyncConnected, onToggleSync }: StatusBarProps) {
  if (!status) {
    return (
      <div className="status-bar">
        <div className="status-bar-content">
          <span className="status-dot offline" />
          <span className="status-text">正在启动...</span>
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
          {!isSyncConnected && <span className="status-paused-badge">已断开</span>}
        </div>
        <div className="status-right">
          <button
            className={`sync-toggle-btn ${isSyncConnected ? "connected" : "disconnected"}`}
            onClick={onToggleSync}
            title={isSyncConnected ? "断开同步" : "连接同步"}
          >
            {isSyncConnected ? "断开" : "连接"}
          </button>
          <span className="status-badge">
            {status.connected_devices}/{status.total_devices} 已连接
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
          <span className="status-ip">无可用网卡</span>
        )}
      </div>
    </div>
  );
}
