import { NetworkInterface } from "../App";

interface NetworkSelectorProps {
  interfaces: NetworkInterface[];
}

export function NetworkSelector({ interfaces }: NetworkSelectorProps) {
  if (interfaces.length === 0) {
    return (
      <div className="empty-state">
        <div className="empty-icon">🌐</div>
        <p>未发现可用网络接口</p>
      </div>
    );
  }

  const physical = interfaces.filter((i) => !i.is_virtual);
  const virtual = interfaces.filter((i) => i.is_virtual);

  return (
    <div className="network-selector">
      <div className="network-info">
        ClipSync 会自动在所有物理网卡上广播，无需手动选择
      </div>

      {physical.length > 0 && (
        <div className="interface-group">
          <div className="group-label">物理网卡（自动广播）</div>
          {physical.map((iface) => (
            <InterfaceItem key={iface.ip} iface={iface} />
          ))}
        </div>
      )}

      {virtual.length > 0 && (
        <div className="interface-group">
          <div className="group-label">虚拟网卡（已跳过）</div>
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
  return (
    <div className={`interface-item ${dimmed ? "dimmed" : ""}`}>
      <div className="interface-icon">{dimmed ? "○" : "●"}</div>
      <div className="interface-info">
        <div className="interface-name">{iface.name}</div>
        <div className="interface-ip">
          {iface.ip} / {iface.netmask}
        </div>
        <div className="interface-broadcast">广播: {iface.broadcast}</div>
      </div>
    </div>
  );
}
