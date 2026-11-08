//! LAN の中のテレビを探す。mDNS（機器が自分の名前を知らせる仕組み）を聞くだけで、どこにもログインしない。

use mdns_sd::{ServiceDaemon, ServiceEvent};
use std::collections::BTreeMap;
use std::net::IpAddr;
use std::time::{Duration, Instant};

/// Google TV と多くの Android TV は、デバッグがオフでもこれで名前を知らせる。
pub const NAME_SERVICE: &str = "_androidtvremote2._tcp.local.";
/// ワイヤレスデバッグの接続用。
pub const CONNECT_SERVICE: &str = "_adb-tls-connect._tcp.local.";
/// ワイヤレスデバッグのペアリング用（ペア設定の画面を開いている間だけ出る）。
pub const PAIR_SERVICE: &str = "_adb-tls-pairing._tcp.local.";

/// 見つかったテレビ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundTv {
    pub address: IpAddr,
    /// テレビが知らせた名前
    pub name: Option<String>,
    /// ワイヤレスデバッグの接続ポート
    pub tls_port: Option<u16>,
    /// ペアリング用のポート
    pub pair_port: Option<u16>,
    /// 5555 番が開いているか
    pub legacy_open: bool,
}

impl FoundTv {
    fn new(address: IpAddr) -> Self {
        Self {
            address,
            name: None,
            tls_port: None,
            pair_port: None,
            legacy_open: false,
        }
    }
}

fn instance_name(fullname: &str, service: &str) -> String {
    fullname
        .strip_suffix(service)
        .unwrap_or(fullname)
        .trim_end_matches('.')
        .to_string()
}

/// `wait` の間 mDNS を聞いて、見つかったテレビを住所ごとにまとめる。5555 番も確かめる。
pub fn scan(wait: Duration) -> Vec<FoundTv> {
    let mut found: BTreeMap<IpAddr, FoundTv> = BTreeMap::new();
    if let Ok(daemon) = ServiceDaemon::new() {
        let receivers: Vec<_> = [NAME_SERVICE, CONNECT_SERVICE, PAIR_SERVICE]
            .iter()
            .filter_map(|s| daemon.browse(s).ok().map(|r| (*s, r)))
            .collect();
        let end = Instant::now() + wait;
        while Instant::now() < end {
            for (service, rx) in &receivers {
                while let Ok(event) = rx.try_recv() {
                    if let ServiceEvent::ServiceResolved(info) = event {
                        for addr in info.get_addresses() {
                            let ip = addr.to_ip_addr();
                            if ip.is_ipv6() {
                                continue;
                            }
                            let tv = found.entry(ip).or_insert_with(|| FoundTv::new(ip));
                            let port = info.get_port();
                            match *service {
                                NAME_SERVICE => {
                                    tv.name = Some(instance_name(info.get_fullname(), service))
                                }
                                CONNECT_SERVICE => tv.tls_port = Some(port),
                                _ => tv.pair_port = Some(port),
                            }
                        }
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let _ = daemon.shutdown();
    }
    for ip in crate::sweep(5555, Duration::from_millis(400)) {
        found
            .entry(IpAddr::V4(ip))
            .or_insert_with(|| FoundTv::new(IpAddr::V4(ip)))
            .legacy_open = true;
    }
    for tv in found.values_mut() {
        if !tv.legacy_open {
            tv.legacy_open = crate::port_open(
                std::net::SocketAddr::new(tv.address, 5555),
                Duration::from_millis(400),
            );
        }
    }
    found.into_values().collect()
}
