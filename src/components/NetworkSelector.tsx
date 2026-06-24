import { NetworkInterface } from "../App";
import { useI18n } from "../i18n/I18nContext";

interface NetworkSelectorProps {
  interfaces: NetworkInterface[];
}

export function NetworkSelector({ interfaces }: NetworkSelectorProps) {
  const { t } = useI18n();

  if (interfaces.length === 0) {
    return (
      <div className="empty-state">
        <div className="empty-icon">🌐</div>
        <p>{t.network_no_interfaces}</p>
      </div>
    );
  }

  const physical = interfaces.filter((i) => !i.is_virtual);
  const virtual = interfaces.filter((i) => i.is_virtual);

  return (
    <div className="network-selector">
      <div className="network-info">
        {t.network_auto_broadcast}
      </div>

      {physical.length > 0 && (
        <div className="interface-group">
          <div className="group-label">{t.network_physical}</div>
          {physical.map((iface) => (
            <InterfaceItem key={iface.ip} iface={iface} />
          ))}
        </div>
      )}

      {virtual.length > 0 && (
        <div className="interface-group">
          <div className="group-label">{t.network_virtual}</div>
          {virtual.map((iface) => (
            <InterfaceItem key={iface.ip} iface={iface} dimmed />
          ))}
        </div>
      )}
    </div>
  );
}

function InterfaceItem({
  iface,
  dimmed = false,
}: {
  iface: NetworkInterface;
  dimmed?: boolean;
}) {
  const { t } = useI18n();
  return (
    <div className={`interface-item ${dimmed ? "dimmed" : ""}`}>
      <div className="interface-icon">{dimmed ? "○" : "●"}</div>
      <div className="interface-info">
        <div className="interface-name">{iface.name}</div>
        <div className="interface-ip">
          {iface.ip} / {iface.netmask}
        </div>
        <div className="interface-broadcast">{t.network_broadcast}: {iface.broadcast}</div>
      </div>
    </div>
  );
}
