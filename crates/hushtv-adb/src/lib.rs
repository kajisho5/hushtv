//! テレビとの通信。adb_client（Rust 製の ADB）で直接つなぐ。Google の adb は使わない。
//!
//! ネットワークデバッグ（5555 番など、暗号化なし）と、ワイヤレスデバッグ（暗号化あり）に対応。
//! ワイヤレスデバッグの 6 桁コードでのペアリングは pair.rs（Google の adb は使わない）。

use adb_client::{ADBDeviceExt, tcp::ADBTcpDevice};
use hushtv_core::names::{is_package, package_names};
use hushtv_core::record::{SETTINGS, TvState};
use hushtv_core::safety::{HOME_QUERY, Tv, resolved_home};
use std::collections::BTreeSet;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream, ToSocketAddrs, UdpSocket};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

pub mod discover;
pub mod fleet;
pub mod flow;
pub mod launcher;
pub mod pair;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("テレビの住所が読めません: {0}")]
    Address(String),
    #[error(
        "テレビにつながりません（{0}）。テレビの電源・同じ Wi-Fi か・デバッグがオンかを確かめてください"
    )]
    Connect(String),
    #[error("鍵を用意できません: {0}")]
    Key(String),
    #[error("テレビがアプリの一覧を返しませんでした。つながりを確かめて、もう一度試してください")]
    NoPackages,
}

/// ADB の鍵の場所。Google の adb と同じ `~/.android/adbkey`（すでに許可したパソコンなら確認が出ない）。
pub fn default_key_path() -> PathBuf {
    if let Ok(home) = std::env::var("ANDROID_USER_HOME") {
        return PathBuf::from(home).join("android").join("adbkey");
    }
    dirs::home_dir()
        .unwrap_or_default()
        .join(".android")
        .join("adbkey")
}

/// 鍵がなければ作って保存する（作らないと、つなぐたびにテレビで「許可」を求められる）。
pub fn ensure_key(path: &Path) -> Result<(), Error> {
    if path.exists() {
        return Ok(());
    }
    use rsa::pkcs8::{EncodePrivateKey, LineEnding};
    let mut rng = rsa::rand_core::OsRng;
    let key = rsa::RsaPrivateKey::new(&mut rng, 2048).map_err(|e| Error::Key(e.to_string()))?;
    let pem = key
        .to_pkcs8_pem(LineEnding::LF)
        .map_err(|e| Error::Key(e.to_string()))?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| Error::Key(e.to_string()))?;
    }
    std::fs::write(path, pem.as_bytes()).map_err(|e| Error::Key(e.to_string()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

/// `192.168.10.23` / `192.168.10.23:5555` / `[fe80::1]:5555` を住所にする（ポートの既定は 5555）。
pub fn parse_endpoint(host: &str) -> Result<SocketAddr, Error> {
    let (name, port) = hushtv_core::record::split_host_port(host);
    let port: u16 = match port {
        Some(p) => p.parse().map_err(|_| Error::Address(host.into()))?,
        None => 5555,
    };
    (name.as_str(), port)
        .to_socket_addrs()
        .map_err(|_| Error::Address(host.into()))?
        .next()
        .ok_or_else(|| Error::Address(host.into()))
}

/// つながったテレビ。
pub struct Device {
    inner: ADBTcpDevice,
    pub host: String,
}

impl std::fmt::Debug for Device {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Device").field("host", &self.host).finish()
    }
}

impl Device {
    /// つなぐ。初めてのパソコンなら、テレビに「USB デバッグを許可しますか？」が出る。
    pub fn connect(host: &str, key: &Path) -> Result<Self, Error> {
        let addr = parse_endpoint(host)?;
        TcpStream::connect_timeout(&addr, Duration::from_secs(3))
            .map_err(|e| Error::Connect(e.to_string()))?;
        ensure_key(key)?;
        let inner = ADBTcpDevice::new_with_custom_private_key(addr, key)
            .map_err(|e| Error::Connect(e.to_string()))?;
        Ok(Self {
            inner,
            host: host.to_string(),
        })
    }

    /// シェルのコマンドを 1 つ実行して、出力（標準出力のあとに標準エラー）を返す。
    pub fn shell(&mut self, command: &str) -> Option<String> {
        let mut out = Vec::new();
        let mut err = Vec::new();
        self.inner
            .shell_command(&command, Some(&mut out), Some(&mut err))
            .ok()?;
        out.extend_from_slice(&err);
        Some(String::from_utf8_lossy(&out).replace('\r', ""))
    }

    /// 機種名（`ro.product.model`）。
    pub fn model(&mut self) -> Option<String> {
        self.shell("getprop ro.product.model")
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }

    /// Android のバージョン。
    pub fn android_version(&mut self) -> Option<String> {
        self.shell("getprop ro.build.version.release")
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }

    /// ユーザー 0 の、入っているアプリと止まっているアプリ（元の recovery.py の query_packages と同じ）。
    pub fn packages(&mut self) -> Result<(BTreeSet<String>, BTreeSet<String>), Error> {
        let mut scope = " --user 0";
        let mut installed =
            package_names(&self.shell("pm list packages --user 0").unwrap_or_default());
        if installed.is_empty() {
            scope = "";
            installed = package_names(&self.shell("pm list packages").unwrap_or_default());
        }
        if installed.is_empty() {
            return Err(Error::NoPackages);
        }
        let disabled = package_names(
            &self
                .shell(&format!("pm list packages -d{scope}"))
                .unwrap_or_default(),
        );
        Ok((installed, disabled))
    }

    /// 記録や計画に使う、今のテレビの状態。
    pub fn state(&mut self) -> Result<TvState, Error> {
        let (installed, disabled) = self.packages()?;
        let home = self.shell(HOME_QUERY).as_deref().and_then(resolved_home);
        let scales = SETTINGS
            .iter()
            .map(|k| {
                (
                    k.to_string(),
                    self.shell(&format!("settings get global {k}"))
                        .unwrap_or_default()
                        .trim()
                        .to_string(),
                )
            })
            .collect();
        Ok(TvState {
            installed,
            disabled,
            home,
            scales,
        })
    }

    /// 止める。1 つずつ送り、最後にテレビへ問い合わせて、まだ動いているものを返す（読み戻し）。
    pub fn disable(
        &mut self,
        packages: &[String],
        mut progress: impl FnMut(usize, &str),
    ) -> Vec<String> {
        for (i, p) in packages.iter().enumerate() {
            if !is_package(p) {
                continue;
            }
            progress(i, p);
            let _ = self.shell(&format!("pm disable-user --user 0 {p}"));
        }
        let enabled = package_names(
            &self
                .shell("pm list packages -e --user 0")
                .unwrap_or_default(),
        );
        packages
            .iter()
            .filter(|p| enabled.contains(*p))
            .cloned()
            .collect()
    }

    /// ホームボタンを押してから、画面を撮る（PNG）。著作権保護の画面などは黒くなることがある。
    pub fn home_screenshot(&mut self, wait: Duration) -> Option<Vec<u8>> {
        let _ = self.shell("input keyevent KEYCODE_HOME");
        std::thread::sleep(wait);
        let buf = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let writer = SharedBuf(buf.clone());
        self.inner
            .exec("screencap -p", &mut std::io::empty(), Box::new(writer))
            .ok()?;
        let bytes = buf.lock().ok()?.clone();
        bytes.starts_with(b"\x89PNG").then_some(bytes)
    }

    /// コマンドを順に送る（元に戻すとき）。
    /// テレビにファイルを送る。
    pub fn push(&mut self, data: &[u8], path: &str) -> Result<(), String> {
        let mut r = std::io::Cursor::new(data);
        self.inner.push(&mut r, &path).map_err(|e| e.to_string())
    }

    pub fn run_all(&mut self, commands: &[String]) -> Vec<(String, String)> {
        commands
            .iter()
            .map(|c| {
                (
                    c.clone(),
                    self.shell(c).unwrap_or_else(|| "（通信エラー）".into()),
                )
            })
            .collect()
    }
}

impl Tv for Device {
    fn sh(&mut self, command: &str) -> Option<String> {
        self.shell(command)
    }
}

struct SharedBuf(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);
impl std::io::Write for SharedBuf {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .map_err(|_| std::io::Error::other("lock"))?
            .extend_from_slice(data);
        Ok(data.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// テレビの 5555 番が開いているか（つながるかどうかだけ。ログインはしない）。
pub fn port_open(addr: SocketAddr, timeout: Duration) -> bool {
    TcpStream::connect_timeout(&addr, timeout).is_ok()
}

/// このパソコンの LAN 側の IPv4 アドレス（外へは何も送らない。経路を調べるだけ）。
pub fn local_ipv4() -> Option<Ipv4Addr> {
    let s = UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect("192.0.2.1:9").ok()?;
    match s.local_addr().ok()?.ip() {
        IpAddr::V4(v4) if v4.is_private() => Some(v4),
        _ => None,
    }
}

/// 同じ /24 の中で、指定のポートが開いている住所を探す。
pub fn sweep(port: u16, timeout: Duration) -> Vec<Ipv4Addr> {
    let Some(me) = local_ipv4() else {
        return vec![];
    };
    let [a, b, c, _] = me.octets();
    let (tx, rx) = mpsc::channel();
    let started = Instant::now();
    let handles: Vec<_> = (1..=254u8)
        .map(|d| {
            let tx = tx.clone();
            std::thread::spawn(move || {
                let ip = Ipv4Addr::new(a, b, c, d);
                if ip != me && port_open(SocketAddr::new(IpAddr::V4(ip), port), timeout) {
                    let _ = tx.send(ip);
                }
            })
        })
        .collect();
    drop(tx);
    for h in handles {
        let _ = h.join();
    }
    let _ = started;
    let mut found: Vec<Ipv4Addr> = rx.into_iter().collect();
    found.sort();
    found
}
