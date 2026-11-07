//! 不可信目标的连接策略。FakeIP 是明确例外，不能把它当作普通私网拒绝。
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;
use url::{Host, Url};

pub fn allowed_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let [a, b, c, _] = ip.octets();
            if a == 198 && (b == 18 || b == 19) {
                return true;
            }
            !(a == 0
                || a == 10
                || a == 127
                || a >= 224
                || (a == 100 && (64..=127).contains(&b))
                || (a == 169 && b == 254)
                || (a == 172 && (16..=31).contains(&b))
                || (a == 192 && b == 168)
                || (a == 192 && b == 0 && (c == 0 || c == 2))
                || (a == 192 && b == 88 && c == 99)
                || (a == 198 && b == 51 && c == 100)
                || (a == 203 && b == 0 && c == 113))
        }
        IpAddr::V6(ip) => {
            if let Some(v4) = ip.to_ipv4_mapped() {
                return allowed_ip(IpAddr::V4(v4));
            }
            let s = ip.segments();
            // 仅全球单播；拒绝 ULA、链路本地、回环、组播、NAT64，以及可封装私网 IPv4 的 6to4/Teredo。
            (s[0] & 0xe000) == 0x2000
                && s[0] != 0x2002
                && !(s[0] == 0x2001 && (s[1] < 0x0200 || s[1] == 0x0db8))
                && !(s[0] == 0x3fff && s[1] < 0x1000)
        }
    }
}

pub fn parse_target(raw: &str) -> Result<Url, String> {
    if raw.chars().any(char::is_control) {
        return Err("目标地址含控制字符".into());
    }
    let url = Url::parse(raw).map_err(|_| "目标地址无效")?;
    if !matches!(url.scheme(), "https" | "http")
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err("目标只允许不带认证信息的 HTTP(S) 地址".into());
    }
    match url.host().ok_or("目标缺少主机名")? {
        Host::Ipv4(ip) => check_ip(ip.into())?,
        Host::Ipv6(ip) => check_ip(ip.into())?,
        Host::Domain(name) => {
            let name = name.trim_end_matches('.').to_ascii_lowercase();
            if ["localhost", "local", "internal", "home.arpa"]
                .iter()
                .any(|suffix| name == *suffix || name.ends_with(&format!(".{suffix}")))
            {
                return Err("禁止访问本机或内网主机名".into());
            }
        }
    }
    Ok(url)
}

fn check_ip(ip: IpAddr) -> Result<(), String> {
    if allowed_ip(ip) {
        Ok(())
    } else {
        Err("禁止访问普通内网、回环或保留地址（仅放行 FakeIP 198.18.0.0/15）".into())
    }
}

pub fn check_addresses(addrs: &[SocketAddr]) -> Result<(), String> {
    if addrs.is_empty() {
        return Err("目标没有可用的 DNS 地址".into());
    }
    // 混合公私网答案整体拒绝，不能让连接器回退到私网地址。
    for addr in addrs {
        check_ip(addr.ip())?;
    }
    Ok(())
}

pub async fn resolve_target(url: &Url) -> Result<Vec<SocketAddr>, String> {
    let port = url.port_or_known_default().ok_or("目标端口无效")?;
    let mut addrs = match url.host().ok_or("目标缺少主机名")? {
        Host::Ipv4(ip) => vec![SocketAddr::new(ip.into(), port)],
        Host::Ipv6(ip) => vec![SocketAddr::new(ip.into(), port)],
        Host::Domain(host) => tokio::time::timeout(
            Duration::from_secs(10),
            tokio::net::lookup_host((host, port)),
        )
        .await
        .map_err(|_| "DNS 解析超时")?
        .map_err(|_| "DNS 解析失败")?
        .collect(),
    };
    addrs.sort_unstable();
    addrs.dedup();
    check_addresses(&addrs)?;
    Ok(addrs)
}

pub fn redirect_target(current: &Url, location: &str) -> Result<Url, String> {
    let next = current.join(location).map_err(|_| "重定向地址无效")?;
    let next = parse_target(next.as_str())?;
    if current.scheme() == "https" && next.scheme() != "https" {
        return Err("禁止 HTTPS 降级重定向".into());
    }
    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_and_fakeip_are_allowed() {
        for ip in [
            "1.1.1.1",
            "8.8.8.8",
            "198.18.0.0",
            "198.19.255.255",
            "2606:4700:4700::1111",
            "::ffff:198.18.0.1",
        ] {
            assert!(allowed_ip(ip.parse().unwrap()), "{ip}");
        }
    }

    #[test]
    fn private_and_special_addresses_are_blocked() {
        for ip in [
            "0.0.0.0",
            "10.0.0.1",
            "100.64.0.1",
            "100.127.255.254",
            "127.0.0.1",
            "169.254.169.254",
            "172.16.0.1",
            "172.31.255.254",
            "192.168.1.1",
            "192.0.0.1",
            "192.0.2.1",
            "198.51.100.1",
            "203.0.113.1",
            "224.0.0.1",
            "255.255.255.255",
            "::",
            "::1",
            "fc00::1",
            "fe80::1",
            "ff02::1",
            "::ffff:127.0.0.1",
            "::ffff:192.168.1.1",
            "64:ff9b::c0a8:101",
            "2002:c0a8:101::1",
            "2001::1",
            "2001:db8::1",
            "3fff::1",
        ] {
            assert!(!allowed_ip(ip.parse().unwrap()), "{ip}");
        }
    }

    #[test]
    fn alternate_spellings_and_unsafe_urls_are_blocked() {
        for raw in [
            "http://127.1/",
            "http://2130706433/",
            "http://0x7f000001/",
            "http://[::ffff:192.168.0.1]/",
            "http://localhost./",
            "http://foo.local/",
            "file:///etc/passwd",
            "https://user:secret@example.com/",
            "https://example.com/\n",
        ] {
            assert!(parse_target(raw).is_err(), "{raw}");
        }
        assert!(parse_target("http://198.18.0.1/video").is_ok());
    }

    #[test]
    fn mixed_dns_and_redirect_bypasses_are_blocked() {
        let addrs = [
            "1.1.1.1:443".parse().unwrap(),
            "192.168.1.1:443".parse().unwrap(),
        ];
        assert!(check_addresses(&addrs).is_err());
        assert!(check_addresses(&[]).is_err());
        let current = Url::parse("https://example.com/media").unwrap();
        assert!(redirect_target(&current, "//127.0.0.1/private").is_err());
        assert!(redirect_target(&current, "http://example.com/media").is_err());
        assert!(redirect_target(&current, "https://198.19.0.1/media").is_ok());
        assert_eq!(
            redirect_target(&current, "/next").unwrap().as_str(),
            "https://example.com/next"
        );
    }
}
