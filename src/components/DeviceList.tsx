import { DeviceInfo } from "../App";
import { useI18n } from "../i18n/I18nContext";

interface DeviceListProps {
  devices: DeviceInfo[];
}

export function DeviceList({ devices }: DeviceListProps) {
  const { t } = useI18n();

  if (devices.length === 0) {
    return (
      <div className="empty-state">
        <div className="empty-icon">📡</div>
        <p>{t.device_no_devices}</p>
        <p className="empty-hint">{t.device_no_devices_hint}</p>
      </div>
    );
  }

  const statusLabel = (status: DeviceInfo["status"]) => {
    switch (status) {
      case "Connected":
        return t.device_status_connected;
      case "Connecting":
        return t.device_status_connecting;
      case "Offline":
        return t.device_status_offline;
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
