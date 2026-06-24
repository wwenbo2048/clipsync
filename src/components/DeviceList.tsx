import { DeviceInfo } from "../App";

interface DeviceListProps {
  devices: DeviceInfo[];
}

export function DeviceList({ devices }: DeviceListProps) {
  if (devices.length === 0) {
    return (
      <div className="empty-state">
        <div className="empty-icon">📡</div>
        <p>暂未发现其他设备</p>
        <p className="empty-hint">请确保其他设备在同一局域网中运行 ClipSync</p>
      </div>
    );
  }

  const statusLabel = (status: DeviceInfo["status"]) => {
    switch (status) {
      case "Connected":
        return "已连接";
      case "Connecting":
        return "连接中...";
      case "Offline":
        return "离线";
    }
  };

  const statusClass = (status: DeviceInfo["status"]) => {
    switch (status) {
      case "Connected":
        return "status-connected";
      case "Connecting":
        return "status-connecting";
      case "Offline":
        return "status-offline";
    }
  };

  return (
    <div className="device-list">
      {devices.map((device) => (
        <div key={device.id} className="device-item">
          <div className="device-icon">
            {device.name.includes("Mac") || device.name.includes("mac") ? "🍎" : "🖥️"}
          </div>
          <div className="device-info">
            <div className="device-name">{device.name}</div>
            <div className="device-ip">
              {device.ip}:{device.port}
            </div>
          </div>
          <div className={`device-status ${statusClass(device.status)}`}>
            {statusLabel(device.status)}
          </div>
        </div>
      ))}
    </div>
  );
}
