//! 戻す記録。元の cleanyourtv の `tools/recovery.py` と同じ形式・同じファイル名の規則
//! （`~/.cleanyourtv/recovery-<アドレス>-<リストのパスの sha1 先頭 8 桁>.json`、version 1）。

use crate::names::{SCALE_RE, is_component, is_package};
use crate::safety::{CUSTOM_LAUNCHER, STOCK_LAUNCHERS, is_chooser, not_a_launcher};
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use std::collections::{BTreeMap, BTreeSet};
use std::net::IpAddr;
use std::path::{Path, PathBuf};

pub const VERSION: u32 = 1;
pub const SETTINGS: [&str; 3] = [
    "window_animation_scale",
    "transition_animation_scale",
    "animator_duration_scale",
];

/// 記録。フィールドは JSON のキーの順（元の `sort_keys=True` と同じ並び）に置く。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecoveryRecord {
    pub created: String,
    pub home: Option<String>,
    pub host: String,
    pub list: String,
    /// パッケージ名 → "enabled" / "disabled" / "absent"
    pub packages: BTreeMap<String, String>,
    #[serde(default)]
    pub settings: BTreeMap<String, String>,
    pub version: u32,
}

/// 記録を作るときに読んだテレビの状態。
#[derive(Debug, Clone, Default)]
pub struct TvState {
    pub installed: BTreeSet<String>,
    pub disabled: BTreeSet<String>,
    /// `resolve-activity` の答え（コンポーネントの形のときだけ）
    pub home: Option<String>,
    pub scales: BTreeMap<String, String>,
}

pub fn package_state(name: &str, s: &TvState) -> &'static str {
    if !s.installed.contains(name) {
        "absent"
    } else if s.disabled.contains(name) {
        "disabled"
    } else {
        "enabled"
    }
}

/// `host:port` / `[ipv6]:port` を (host, port) に分ける。
pub fn split_host_port(endpoint: &str) -> (String, Option<String>) {
    if let Some(rest) = endpoint.strip_prefix('[') {
        if let Some(end) = rest.find(']') {
            let after = &rest[end + 1..];
            return (
                rest[..end].to_string(),
                after.strip_prefix(':').map(str::to_string),
            );
        }
    } else if endpoint.matches(':').count() == 1 {
        let (h, p) = endpoint.split_once(':').unwrap();
        return (h.to_string(), Some(p.to_string()));
    }
    (endpoint.to_string(), None)
}

/// ネットワーク上のテレビなら IP アドレス（ループバックと名前は None）。
pub fn lan_address(host: &str) -> Option<String> {
    let (name, _) = split_host_port(host);
    match name.parse::<IpAddr>() {
        Ok(ip) if !ip.is_loopback() => Some(name),
        _ => None,
    }
}

/// 記録を置くフォルダ（元と同じ `~/.cleanyourtv/`）。
pub fn record_dir(home: &Path) -> PathBuf {
    home.join(".cleanyourtv")
}

fn list_digest(list_path: &Path) -> String {
    let resolved = std::fs::canonicalize(list_path).unwrap_or_else(|_| list_path.to_path_buf());
    let mut h = Sha1::new();
    h.update(resolved.to_string_lossy().as_bytes());
    let out = h.finalize();
    out.iter().take(4).map(|b| format!("{b:02x}")).collect()
}

fn record_name(dir: &Path, list_path: &Path, tag: &str) -> PathBuf {
    let tag: String = if tag.is_empty() {
        "no-host".into()
    } else {
        tag.chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .collect()
    };
    dir.join(format!("recovery-{tag}-{}.json", list_digest(list_path)))
}

/// この TV とリストの記録の場所。古い名前（ポート付き）の記録があればそれを使う。
/// 新旧どちらの名前でもあるときは、どちらが正しいか決められないのでエラー。
pub fn find_record(dir: &Path, list_path: &Path, host: &str) -> Result<PathBuf, Vec<PathBuf>> {
    let address = lan_address(host);
    let path = record_name(dir, list_path, address.as_deref().unwrap_or(host));
    let Some(address) = address else {
        return Ok(path);
    };
    let port = split_host_port(host).1.unwrap_or_else(|| "5555".into());
    let typed = if address.contains(':') {
        format!("[{address}]:{port}")
    } else {
        format!("{address}:{port}")
    };
    let mut names = vec![path.clone()];
    for n in [
        record_name(dir, list_path, &typed),
        record_name(dir, list_path, host),
    ] {
        if !names.contains(&n) {
            names.push(n);
        }
    }
    let found: Vec<PathBuf> = names.into_iter().filter(|n| n.exists()).collect();
    match found.len() {
        0 => Ok(path),
        1 => Ok(found[0].clone()),
        _ => Err(found),
    }
}

/// 新しい記録を作る。リストのパッケージに加え、今のホームのパッケージと、入っている純正ホームも記録する。
pub fn new_record(
    created: &str,
    host: &str,
    list_path: &Path,
    packages: &[String],
    s: &TvState,
) -> RecoveryRecord {
    let mut home = s.home.clone().filter(|h| is_component(h));
    if home.as_deref().is_some_and(is_chooser) {
        home = None;
    }
    let mut targets: Vec<String> = packages.to_vec();
    if let Some(h) = &home {
        let hp = h.split('/').next().unwrap().to_string();
        if !targets.contains(&hp) {
            targets.push(hp);
        }
    }
    for st in STOCK_LAUNCHERS {
        if s.installed.contains(st) && !targets.iter().any(|t| t == st) {
            targets.push(st.to_string());
        }
    }
    if home.as_deref().is_some_and(not_a_launcher) {
        home = None;
    }
    let resolved = std::fs::canonicalize(list_path).unwrap_or_else(|_| list_path.to_path_buf());
    RecoveryRecord {
        created: created.to_string(),
        home,
        host: host.to_string(),
        list: resolved.to_string_lossy().to_string(),
        packages: targets
            .iter()
            .map(|t| (t.clone(), package_state(t, s).to_string()))
            .collect(),
        settings: s.scales.clone(),
        version: VERSION,
    }
}

/// 既にある記録に、リストが増やした名前を足す。記録済みの名前は上書きしない（最初の状態を守る）。
/// 「なかった」と記録されたもので、今は入っていて動いているものは "enabled" に直す。
/// 変わったら true。
pub fn extend_record(record: &mut RecoveryRecord, names: &[String], s: &TvState) -> bool {
    let mut changed = false;
    for n in names {
        match record.packages.get(n).map(String::as_str) {
            None => {
                record
                    .packages
                    .insert(n.clone(), package_state(n, s).to_string());
                changed = true;
            }
            Some("absent") if package_state(n, s) == "enabled" => {
                record.packages.insert(n.clone(), "enabled".into());
                changed = true;
            }
            _ => {}
        }
    }
    changed
}

/// 読み込んで、形を確かめる（壊れた記録で戻そうとしない）。
pub fn load(text: &str) -> Result<RecoveryRecord, String> {
    let r: RecoveryRecord = serde_json::from_str(text).map_err(|e| e.to_string())?;
    if r.version != VERSION {
        return Err(format!(
            "記録のバージョンが {} です（{} だけ読めます）",
            r.version, VERSION
        ));
    }
    if let Some(bad) = r.packages.keys().find(|k| !is_package(k)) {
        return Err(format!("記録にパッケージ名ではない名前があります: {bad}"));
    }
    if let Some(h) = &r.home
        && !is_component(h)
    {
        return Err(format!("記録のホームの形が違います: {h}"));
    }
    Ok(r)
}

/// 元の `json.dump(record, indent=2, sort_keys=True)` と同じ形で書き出す。
pub fn to_json(r: &RecoveryRecord) -> String {
    serde_json::to_string_pretty(r).unwrap() + "\n"
}

/// 記録どおりに戻すコマンド（1 行 1 コマンド）。`stock` は純正ホームの名前（なければ Google 標準）。
pub fn restore_commands(r: &RecoveryRecord, stock: Option<&str>) -> Vec<String> {
    let mut cmds: Vec<String> = r
        .packages
        .iter()
        .filter(|(_, s)| *s == "enabled")
        .map(|(n, _)| format!("pm enable {n}"))
        .collect();
    let fallback = stock
        .filter(|s| is_package(s))
        .unwrap_or("com.google.android.tvlauncher");
    let fallback_state = r.packages.get(fallback).map(String::as_str);
    let resolve = format!(
        "cmd package set-home-activity --user 0 $(cmd package resolve-activity --brief -a android.intent.action.MAIN -c android.intent.category.HOME {fallback} | tail -n 1)"
    );
    match &r.home {
        Some(h) if h.split('/').next() != Some(CUSTOM_LAUNCHER) => {
            cmds.push(format!("cmd package set-home-activity --user 0 {h}"))
        }
        Some(h) if fallback_state != Some("enabled") => {
            cmds.push(format!("cmd package set-home-activity --user 0 {h}"))
        }
        None if matches!(fallback_state, Some("disabled") | Some("absent")) => {}
        _ => cmds.push(resolve),
    }
    for key in SETTINGS {
        let v = r.settings.get(key).map(|v| v.trim()).unwrap_or("");
        if v == "null" {
            cmds.push(format!("settings delete global {key}"));
        } else if SCALE_RE.is_match(v) {
            cmds.push(format!("settings put global {key} {v}"));
        }
    }
    cmds
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> TvState {
        TvState {
            installed: [
                "com.a.ad",
                "com.a.kept",
                "com.google.android.tvlauncher",
                "com.a.home",
            ]
            .map(String::from)
            .into(),
            disabled: ["com.a.kept"].map(String::from).into(),
            home: Some("com.a.home/.Main".into()),
            scales: SETTINGS
                .iter()
                .map(|k| (k.to_string(), "1.0".into()))
                .collect(),
        }
    }

    #[test]
    fn record_holds_list_home_and_stock_launchers() {
        let r = new_record(
            "2026-11-08T21:00:00+0900",
            "192.168.10.23",
            Path::new("/no/such/list.txt"),
            &["com.a.ad".into(), "com.a.kept".into(), "com.a.gone".into()],
            &state(),
        );
        assert_eq!(r.packages["com.a.ad"], "enabled");
        assert_eq!(r.packages["com.a.kept"], "disabled");
        assert_eq!(r.packages["com.a.gone"], "absent");
        assert_eq!(r.packages["com.a.home"], "enabled");
        assert_eq!(r.packages["com.google.android.tvlauncher"], "enabled");
        assert_eq!(r.home.as_deref(), Some("com.a.home/.Main"));
        // JSON のキーは元と同じアルファベット順
        let json = to_json(&r);
        let keys: Vec<usize> = [
            "\"created\"",
            "\"home\"",
            "\"host\"",
            "\"list\"",
            "\"packages\"",
            "\"settings\"",
            "\"version\"",
        ]
        .iter()
        .map(|k| json.find(k).unwrap())
        .collect();
        assert!(keys.windows(2).all(|w| w[0] < w[1]));
        assert_eq!(load(&json).unwrap(), r);
    }

    #[test]
    fn chooser_and_setup_screen_are_not_recorded_as_home() {
        let mut s = state();
        s.home = Some("android/com.android.internal.app.ResolverActivity".into());
        assert_eq!(new_record("t", "h", Path::new("x"), &[], &s).home, None);
        s.home = Some("com.google.android.tungsten.setupwraith/.RecoveryActivity".into());
        let r = new_record("t", "h", Path::new("x"), &[], &s);
        assert_eq!(r.home, None);
        assert!(
            r.packages
                .contains_key("com.google.android.tungsten.setupwraith")
        );
    }

    #[test]
    fn restore_only_enables_what_was_on() {
        let r = new_record(
            "t",
            "h",
            Path::new("x"),
            &["com.a.ad".into(), "com.a.kept".into()],
            &state(),
        );
        let cmds = restore_commands(&r, None);
        assert!(cmds.contains(&"pm enable com.a.ad".to_string()));
        assert!(!cmds.iter().any(|c| c.contains("com.a.kept")));
        assert!(
            cmds.contains(&"cmd package set-home-activity --user 0 com.a.home/.Main".to_string())
        );
        assert!(cmds.contains(&"settings put global window_animation_scale 1.0".to_string()));
    }

    #[test]
    fn extend_never_overwrites() {
        let mut r = new_record(
            "t",
            "h",
            Path::new("x"),
            &["com.a.ad".into(), "com.a.new".into()],
            &state(),
        );
        let mut later = state();
        later.disabled.insert("com.a.ad".into());
        later.installed.insert("com.a.new".into());
        assert!(extend_record(
            &mut r,
            &["com.a.ad".into(), "com.a.new".into(), "com.a.more".into()],
            &later
        ));
        assert_eq!(r.packages["com.a.ad"], "enabled");
        assert_eq!(r.packages["com.a.new"], "enabled");
        assert_eq!(r.packages["com.a.more"], "absent");
    }

    #[test]
    fn record_names() {
        assert_eq!(
            split_host_port("192.168.10.23:5555"),
            ("192.168.10.23".into(), Some("5555".into()))
        );
        assert_eq!(
            split_host_port("[fe80::1]:5555"),
            ("fe80::1".into(), Some("5555".into()))
        );
        assert_eq!(lan_address("127.0.0.1:15555"), None);
        let dir = Path::new("/tmp/hushtv-test-none");
        let p = find_record(dir, Path::new("/x/debloat.txt"), "192.168.10.23:5555").unwrap();
        assert!(
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("recovery-192.168.10.23-")
        );
    }

    #[test]
    fn rejects_broken_record() {
        assert!(load(r#"{"created":"t","home":null,"host":"h","list":"l","packages":{"x;reboot":"enabled"},"settings":{},"version":1}"#).is_err());
        assert!(load(r#"{"created":"t","home":null,"host":"h","list":"l","packages":{},"settings":{},"version":2}"#).is_err());
    }
}
