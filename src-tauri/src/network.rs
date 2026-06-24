use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;

/// 网络接口信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInterface {
    pub name: String,
    pub ip: String,
    pub netmask: String,
    pub broadcast: String,
    pub is_loopback: bool,
    pub is_virtual: bool,
}

/// 获取所有有效的网络接口（IPv4，非回环）
pub fn get_all_interfaces() -> Vec<NetworkInterface> {
    let mut result = Vec::new();

    match get_if_addrs::get_if_addrs() {
        Ok(ifaces) => {
            for iface in ifaces {
                let (ip, netmask) = match iface.addr {
                    get_if_addrs::IfAddr::V4(ref v4) => (v4.ip, v4.netmask),
                    _ => continue,
                };

                // 跳过回环和 0.0.0.0
                if ip.is_loopback() || ip.is_unspecified() {
                    continue;
                }

                let name = iface.name.clone();

                // 计算广播地址
                let broadcast = compute_broadcast(ip, netmask);

                // 检测虚拟网卡
                let name_lower = name.to_lowercase();
                let is_virtual = name_lower.contains("vmware")
                    || name_lower.contains("virtualbox")
                    || name_lower.contains("vbox")
                    || name_lower.contains("hyper-v")
                    || name_lower.contains("veth")
                    || name_lower.contains("docker")
                    || name_lower.contains("vpn")
                    || name_lower.contains("tun")
                    || name_lower.contains("tap")
                    || name_lower.contains("tailscale")
                    || name_lower.contains("wsl")
                    || name_lower.contains("hamachi")
                    || name_lower.contains("zerotier");

                result.push(NetworkInterface {
                    name,
                    ip: ip.to_string(),
                    netmask: netmask.to_string(),
                    broadcast: broadcast.to_string(),
                    is_loopback: ip.is_loopback(),
                    is_virtual,
                });
            }
        }
        Err(e) => {
            log::warn!("Failed to enumerate network interfaces: {}", e);
        }
    }

    // 物理网卡排前面
    result.sort_by(|a, b| {
        a.is_virtual
            .cmp(&b.is_virtual)
            .then_with(|| a.name.cmp(&b.name))
    });

    result
}

/// 获取所有物理网卡（用于广播）
pub fn get_physical_interfaces() -> Vec<NetworkInterface> {
    get_all_interfaces()
        .into_iter()
        .filter(|i| !i.is_virtual)
        .collect()
}

/// 计算子网广播地址
fn compute_broadcast(ip: Ipv4Addr, netmask: Ipv4Addr) -> Ipv4Addr {
    let ip_bits = u32::from(ip);
    let mask_bits = u32::from(netmask);
    let broadcast_bits = ip_bits | !mask_bits;
    Ipv4Addr::from(broadcast_bits)
}
