//! This PC's network addresses, for the LAN web view's links (0.2's webcmd.rs).

/// Private IPv4 addresses of connected adapters: physical ones with a gateway first, virtual
/// adapters (Hyper-V, VMware, VPN, TUN…) last.
pub fn local_addresses() -> Vec<(String, String, bool)> {
    use windows::Win32::NetworkManagement::IpHelper::*;
    use windows::Win32::NetworkManagement::Ndis::IfOperStatusUp;
    use windows::Win32::Networking::WinSock::{AF_INET, SOCKADDR_IN};
    let flags = GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST | GAA_FLAG_SKIP_DNS_SERVER | GAA_FLAG_INCLUDE_GATEWAYS;
    let mut size = 16 * 1024u32;
    let mut buf: Vec<u64> = Vec::new();
    for _ in 0..3 {
        buf = vec![0u64; size as usize / 8 + 1];
        let r = unsafe { GetAdaptersAddresses(AF_INET.0 as u32, flags, None, Some(buf.as_mut_ptr() as *mut IP_ADAPTER_ADDRESSES_LH), &mut size) };
        if r == 0 {
            break;
        }
        if r != 111 {
            return Vec::new(); // 111 = ERROR_BUFFER_OVERFLOW: retry with `size`
        }
    }
    let mut out: Vec<(String, String, bool, bool)> = Vec::new();
    let mut p = buf.as_ptr() as *const IP_ADAPTER_ADDRESSES_LH;
    while !p.is_null() {
        let a = unsafe { &*p };
        p = a.Next;
        if a.OperStatus != IfOperStatusUp || a.IfType == IF_TYPE_SOFTWARE_LOOPBACK {
            continue;
        }
        let name = unsafe { a.FriendlyName.to_string() }.unwrap_or_default();
        let desc = unsafe { a.Description.to_string() }.unwrap_or_default();
        let hay = format!("{name} {desc}").to_lowercase();
        const VIRTUAL: [&str; 12] = ["virtual", "vmware", "virtualbox", "hyper-v", "vethernet", "tap-", "tun", "wintun", "wireguard", "clash", "zerotier", "tailscale"];
        let virt = VIRTUAL.iter().any(|k| hay.contains(k));
        let gateway = !a.FirstGatewayAddress.is_null();
        let mut u = a.FirstUnicastAddress;
        while !u.is_null() {
            let ua = unsafe { &*u };
            u = ua.Next;
            let sa = ua.Address.lpSockaddr;
            if sa.is_null() || unsafe { (*sa).sa_family } != AF_INET {
                continue;
            }
            let b = unsafe { (*(sa as *const SOCKADDR_IN)).sin_addr.S_un.S_un_b };
            let ip = std::net::Ipv4Addr::new(b.s_b1, b.s_b2, b.s_b3, b.s_b4);
            if ip.is_private() {
                out.push((ip.to_string(), name.clone(), virt, gateway));
            }
        }
    }
    out.sort_by_key(|(_, _, virt, gw)| (*virt, !*gw));
    out.dedup_by(|a, b| a.0 == b.0);
    out.into_iter().map(|(ip, label, virt, _)| (ip, label, virt)).collect()
}
