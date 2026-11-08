//! 安全装置。元の cleanyourtv の `tools/disable.sh` の検査を、同じ順番・同じ問い合わせで移したもの。
//!
//! どの検査も「確かめられなかったら止めない側に倒す」。テレビへの問い合わせは [`Tv`] を通すので、
//! テストでは答えを差し替えて確かめられる。

use crate::names::{PACKAGE_RE, is_package};
use regex::Regex;
use serde::Serialize;
use std::collections::BTreeSet;
use std::sync::LazyLock;

/// テレビのシェルにコマンドを送って答えを受け取るもの。通信に失敗したら `None`。
pub trait Tv {
    fn sh(&mut self, command: &str) -> Option<String>;
}

/// Android の中核とテレビの制御部品。止めるとテレビが起動しなくなる事例がある（元の #884, #1578, #2557, #2803）。
pub static CORE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(android|android\.ext\.(services|shared)|com\.android\.sdksandbox|com\.android\.location\.fused|com\.android\.systemui|com\.android\.shell|com\.android\.phone|com\.android\.settings|com\.android\.tv\.settings|com\.(google\.)?android\.(permissioncontroller|packageinstaller|ext\.services|ext\.shared|modulemetadata)|com\.google\.android\.overlay\.modules.*|com\.android\.providers\..*|com\.cltv\.mal|com\.mediatek\.(tis|tv\.service|tv\.agent))$").unwrap()
});
/// 中核の形でも止めてよいもの（どのテレビ機能も使わない連絡先などのプロバイダ）。
pub static CORE_OK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^com\.android\.providers\.(calendar|contacts|userdictionary|blockednumber)$")
        .unwrap()
});

pub const SETUPWRAITH: &str = "com.google.android.tungsten.setupwraith";
/// 元の cleanyourtv のホーム画面アプリ。
pub const CUSTOM_LAUNCHER: &str = "com.gathin.tvhome";
pub const STOCK_LAUNCHERS: [&str; 4] = [
    "com.google.android.tvlauncher",
    "com.google.android.apps.tv.launcherx",
    "com.google.android.leanbacklauncher",
    "com.mitv.tvhome",
];
/// ホームに答えるがホーム画面ではないもの（初期設定画面と設定アプリ）。
pub const NOT_LAUNCHERS: [&str; 4] = [
    SETUPWRAITH,
    "com.android.tv.settings",
    "com.android.settings",
    "com.android.provision",
];
static FALLBACK_HOME_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"/\S*FallbackHome$").unwrap());
static FACTORY_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)factorytest|factory_test|factorymode").unwrap());

pub const HOME_QUERY: &str = "cmd package resolve-activity --brief -a android.intent.action.MAIN -c android.intent.category.HOME";
pub const CLAIM_QUERY: &str = "cmd package query-activities --brief -a android.intent.action.MAIN -c android.intent.category.HOME";

/// 止めずに残した理由。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    /// Android の中核・テレビの制御部品
    Core,
    /// 保存済み Wi-Fi の持ち主が初期設定画面（止めると Wi-Fi が消える）
    SetupwraithOwnsWifi,
    /// Wi-Fi の持ち主を確かめられなかった
    SetupwraithUnchecked,
    /// 今のホームで、ほかにホームになれるアプリがない
    HomeOwnerAlone,
    /// 今のホームで、止めると初期設定画面がホームを取ってしまう
    HomeOwnerSetupwraith,
    /// ホームの既定がなく、止めるとホームになれるものがなくなる
    HomeNoDefault,
    /// ホームを確かめられず、純正のホーム画面だった
    HomeUnknownStock,
    /// HDMI などの入力の土台（止めると「信号なし」のままになる）
    TvInputHardware,
    /// 入力の土台かどうか、このファームでは読めなかった
    TvInputUnchecked,
    /// テレビの診断出力が止まっていて読めなかった（再起動が必要）
    TvInputHung,
    /// 入力切替を開くアプリ
    InputHandler,
    /// 設定を開くアプリ
    SettingsHandler,
}

impl Reason {
    pub fn text(self, lang: Lang) -> &'static str {
        use Reason::*;
        match (self, lang) {
            (Core, Lang::Ja) => {
                "Android の中核かテレビの制御部品です。止めると起動しなくなる事例があります"
            }
            (Core, Lang::En) => "Core Android or TV middleware; the TV may not boot without it",
            (SetupwraithOwnsWifi, Lang::Ja) => {
                "保存済み Wi-Fi の持ち主です。止めると Wi-Fi が消えます（設定で Wi-Fi を一度削除して入り直すと止められます）"
            }
            (SetupwraithOwnsWifi, Lang::En) => {
                "It owns the saved Wi-Fi network; disabling it deletes that network (forget and re-join it in Settings first)"
            }
            (SetupwraithUnchecked, Lang::Ja) => "Wi-Fi の持ち主を確かめられませんでした",
            (SetupwraithUnchecked, Lang::En) => {
                "Could not check which app owns the saved Wi-Fi networks"
            }
            (HomeOwnerAlone, Lang::Ja) => {
                "今のホーム画面です。ほかにホームになれるアプリがありません（先に新しいホーム画面を入れてください）"
            }
            (HomeOwnerAlone, Lang::En) => {
                "It owns Home and nothing else could answer Home (install the new launcher first)"
            }
            (HomeOwnerSetupwraith, Lang::Ja) => {
                "今のホーム画面です。止めると初期設定画面がホームを取り、画面が真っ黒になる事例があります"
            }
            (HomeOwnerSetupwraith, Lang::En) => {
                "It owns Home, and the setup wizard would take Home instead (black screen)"
            }
            (HomeNoDefault, Lang::Ja) => {
                "ホームの既定がなく、止めるとホームになれるアプリがなくなります"
            }
            (HomeNoDefault, Lang::En) => "Home has no default, and nothing else could answer Home",
            (HomeUnknownStock, Lang::Ja) => "ホームを確かめられず、これは純正のホーム画面です",
            (HomeUnknownStock, Lang::En) => {
                "Could not ask which app owns Home, and this is a stock launcher"
            }
            (TvInputHardware, Lang::Ja) => {
                "HDMI などの入力の土台です。止めると「信号なし」のままになる事例があります"
            }
            (TvInputHardware, Lang::En) => {
                "It drives the HDMI/AV/tuner inputs; disabled, they can stay on \"No Signal\""
            }
            (TvInputUnchecked, Lang::Ja) => {
                "入力の土台かどうか、このファームの答えでは読めませんでした"
            }
            (TvInputUnchecked, Lang::En) => {
                "This firmware's answer cannot be read for the input-driver check"
            }
            (TvInputHung, Lang::Ja) => {
                "テレビの診断出力が止まっています。テレビを再起動してからやり直してください"
            }
            (TvInputHung, Lang::En) => {
                "The TV's dumps are switched off; reboot the TV and try again"
            }
            (InputHandler, Lang::Ja) => "入力切替やチャンネルを開くアプリです",
            (InputHandler, Lang::En) => "The TV opens its inputs and channels with it",
            (SettingsHandler, Lang::Ja) => {
                "設定を開くアプリです。止めると設定（と開発者向けオプション）が開けなくなります"
            }
            (SettingsHandler, Lang::En) => {
                "The TV opens Settings with it (and the way back to Developer options)"
            }
        }
    }
}

/// 表示の言語。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Lang {
    Ja,
    En,
}

impl Lang {
    /// OS の言語設定（LANG など）から決める。
    pub fn from_env() -> Self {
        for key in ["HUSHTV_LANG", "LC_ALL", "LC_MESSAGES", "LANG"] {
            if let Ok(v) = std::env::var(key)
                && !v.is_empty()
            {
                return if v.to_lowercase().starts_with("ja") {
                    Lang::Ja
                } else {
                    Lang::En
                };
            }
        }
        Lang::Ja
    }
}

/// 検査の途中で分かった、利用者に伝えること。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Note {
    /// ホームの既定がない（選択画面）
    HomeChooser,
    /// ホームの持ち主が分からなかった
    HomeUnknown,
    /// 入力の土台の検査ができなかった
    TvInputCheckFailed,
    /// 入力・設定を開くアプリの問い合わせに何も返ってこなかった
    HandlersEmpty,
    /// 入力・設定を開くアプリの検査ができなかった
    HandlersCheckFailed,
    /// 5555 番が「許可」なしで誰でもつながる設定
    AdbInsecure,
}

impl Note {
    pub fn text(self, lang: Lang) -> &'static str {
        use Note::*;
        match (self, lang) {
            (HomeChooser, Lang::Ja) => {
                "ホームの既定がないため、ホームに答えるアプリの一覧で判断しました。リストに今のホーム画面が入っていないか確かめてください"
            }
            (HomeChooser, Lang::En) => {
                "Home has no default, so the Home check went by the apps that answer Home. Check the list does not hold your launcher"
            }
            (HomeUnknown, Lang::Ja) => {
                "どのアプリがホームか分からなかったため、ホームに答えるアプリの一覧で判断しました。リストに今のホーム画面が入っていないか確かめてください"
            }
            (HomeUnknown, Lang::En) => {
                "Could not tell which app owns Home, so the check went by the apps that answer Home. Check the list does not hold your launcher"
            }
            (TvInputCheckFailed, Lang::Ja) => {
                "HDMI などの入力を動かすアプリかどうか確かめられませんでした。終わったら全部の入力を映して確認してください"
            }
            (TvInputCheckFailed, Lang::En) => {
                "Could not check which packages drive the inputs. Check every input after this run"
            }
            (HandlersEmpty, Lang::Ja) => {
                "入力切替と設定を開くアプリを問い合わせても答えがありませんでした。終わったら入力と設定が開くか確かめてください"
            }
            (HandlersEmpty, Lang::En) => {
                "The query for input and Settings handlers answered nothing. Open an input and Settings after this run"
            }
            (HandlersCheckFailed, Lang::Ja) => {
                "入力切替と設定を開くアプリを確かめられませんでした。終わったら入力と設定が開くか確かめてください"
            }
            (HandlersCheckFailed, Lang::En) => {
                "Could not check the input and Settings handlers. Open an input and Settings after this run"
            }
            (AdbInsecure, Lang::Ja) => {
                "このテレビは、同じネットワークの機器なら「許可」なしでつながる設定です。終わったら必ずデバッグを切ってテレビを再起動してください"
            }
            (AdbInsecure, Lang::En) => {
                "This TV accepts network ADB from any device without the Allow prompt. Turn debugging off and reboot when done"
            }
        }
    }
}

/// 安全装置の上書き（元の FORCE_HOME などの環境変数と同じ）。既定は全部オフ。
#[derive(Debug, Clone, Copy, Default, Serialize)]
pub struct Overrides {
    pub force_setupwraith: bool,
    pub force_home: bool,
    pub tvinput_allow: bool,
    pub handlers_allow: bool,
}

/// 検査の結果。
#[derive(Debug, Clone, Default, Serialize)]
pub struct Guarded {
    /// 検査を通って、止めてよいもの（リストの順）
    pub send: Vec<String>,
    /// 止めずに残したもの
    pub skipped: Vec<(String, Reason)>,
    pub notes: Vec<Note>,
}

impl Guarded {
    fn skip(&mut self, pkg: &str, why: Reason) {
        if let Some(i) = self.send.iter().position(|p| p == pkg) {
            self.send.remove(i);
            self.skipped.push((pkg.to_string(), why));
        }
    }
    fn has(&self, pkg: &str) -> bool {
        self.send.iter().any(|p| p == pkg)
    }
}

/// 中核の部品か。
pub fn is_core(pkg: &str) -> bool {
    CORE_RE.is_match(pkg) && !CORE_OK.is_match(pkg)
}

/// ホームの選択画面か（パッケージ名が `android` など、パッケージの形でない）。
pub fn is_chooser(home: &str) -> bool {
    !home.is_empty() && !is_package(home.split('/').next().unwrap_or(""))
}

/// ホームに答えても、戻す先にしてはいけないもの（初期設定画面・仮のホーム・工場テスト画面）。
pub fn not_a_launcher(home: &str) -> bool {
    if home.is_empty() {
        return false;
    }
    let pkg = home.split('/').next().unwrap_or("");
    NOT_LAUNCHERS.contains(&pkg) || FALLBACK_HOME_RE.is_match(home) || FACTORY_RE.is_match(pkg)
}

/// `resolve-activity` の答えの最後の行（コンポーネントの形のときだけ）。
pub fn resolved_home(answer: &str) -> Option<String> {
    let last = answer.lines().last()?.trim();
    crate::names::is_component(last).then(|| last.to_string())
}

static HOME_PROCESS_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"mHomeProcess:.*[: ]([A-Za-z0-9_.]+)/[A-Za-z0-9_.]+").unwrap());

/// `dumpsys activity` の mHomeProcess からホームのパッケージ（Android 7 より古いファーム用）。
pub fn home_process(dump: &str) -> Option<String> {
    dump.lines().find_map(|l| {
        HOME_PROCESS_RE
            .captures(l.trim_end_matches('\r'))
            .map(|c| c[1].to_string())
    })
}

static CLAIM_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^ *([A-Za-z0-9_.]+/[A-Za-z0-9_.$]+)").unwrap());

/// `query-activities` の答えから、ホームに答えるコンポーネント（重複なし）。空なら `None`。
pub fn home_claims(answer: &str) -> Option<BTreeSet<String>> {
    let set: BTreeSet<String> = answer
        .lines()
        .filter_map(|l| {
            CLAIM_RE
                .captures(l.trim_end_matches('\r'))
                .map(|c| c[1].to_string())
        })
        .collect();
    (!set.is_empty()).then_some(set)
}

/// ホームに答えるコンポーネントのうち、ホーム画面アプリのパッケージ。
pub fn launchers_in(claims: &BTreeSet<String>) -> BTreeSet<String> {
    claims
        .iter()
        .filter(|c| !FALLBACK_HOME_RE.is_match(c))
        .map(|c| c.split('/').next().unwrap_or("").to_string())
        .filter(|p| !FACTORY_RE.is_match(p) && !NOT_LAUNCHERS.contains(&p.as_str()))
        .collect()
}

/// 入力の土台かどうかを、テレビの中で 1 回で調べるシェル（元の disable.sh と同じ）。
/// 名前は全部パッケージ名の形を確かめてから入れる。
pub fn tvinput_script(names: &[String]) -> String {
    let names: Vec<&str> = names
        .iter()
        .map(String::as_str)
        .filter(|n| is_package(n))
        .collect();
    format!(
        "s=$(getprop ro.build.version.sdk); for p in {} ; do d=$(dumpsys package $p 2>/dev/null); case $d in *'requested permissions:'*) printf '%s\\n' \"$d\" | sed -n -E '/requested permissions:/,/(install|runtime) permissions:|User [0-9]+:/p' | grep -q TV_INPUT_HARDWARE && echo $p;; *'grantedPermissions:'*) printf '%s\\n' \"$d\" | sed -n -E '/grantedPermissions:/,/(install|runtime) permissions:|User [0-9]+:/p' | grep -q TV_INPUT_HARDWARE && echo $p;; *'Service Resolver Table:'*) printf '%s\\n' \"$d\" | grep -q 'android.media.tv.TvInputService' && echo $p;; *'Unable to find'*) :;; *'hung system process'*) echo \"HUNG $p\";; *\"Package [$p]\"*) [ \"${{s:-0}}\" -ge 23 ] 2>/dev/null || echo \"NOCHK $p\";; *) echo \"NOCHK $p\";; esac; done; true",
        names.join(" ")
    )
}

pub const HANDLERS_QUERY: &str = "cmd package query-activities --brief -a android.intent.action.VIEW -t vnd.android.cursor.item/channel 2>/dev/null; echo ==; cmd package query-activities --brief -a android.settings.SETTINGS 2>/dev/null; true";

static HANDLER_NAME_RE: LazyLock<Regex> = LazyLock::new(|| {
    let pkg = PACKAGE_RE
        .as_str()
        .trim_start_matches('^')
        .trim_end_matches('$');
    Regex::new(&format!(r"^\s*({pkg})/")).unwrap()
});

/// 入力切替・設定を開くアプリ（`==` の行で前後に分ける）。
pub fn handlers(answer: &str) -> (BTreeSet<String>, BTreeSet<String>) {
    let mut input = BTreeSet::new();
    let mut settings = BTreeSet::new();
    let mut after = false;
    for line in answer.lines().map(|l| l.trim_end_matches('\r')) {
        if line == "==" && !after {
            after = true;
            continue;
        }
        if let Some(c) = HANDLER_NAME_RE.captures(line) {
            if after {
                settings.insert(c[1].to_string())
            } else {
                input.insert(c[1].to_string())
            };
        }
    }
    (input, settings)
}

/// 全部の検査を、元の disable.sh と同じ順番で行う。`packages` はリストの止める行（形は確認済み）。
pub fn run(packages: &[String], tv: &mut dyn Tv, ov: Overrides) -> Guarded {
    let mut g = Guarded {
        send: packages.to_vec(),
        ..Default::default()
    };

    // 1. 中核の部品（上書きできない）
    for p in packages.iter().filter(|p| is_core(p)) {
        g.skip(p, Reason::Core);
    }

    // 2. 初期設定画面と Wi-Fi
    if g.has(SETUPWRAITH) && !ov.force_setupwraith {
        let wifi = tv.sh("dumpsys wifi").unwrap_or_default();
        let owners: Vec<&str> = wifi
            .match_indices("cname=")
            .map(|(i, _)| {
                let rest = &wifi[i..];
                rest.split(char::is_whitespace).next().unwrap_or("")
            })
            .collect();
        if owners.is_empty() {
            g.skip(SETUPWRAITH, Reason::SetupwraithUnchecked);
        } else if owners.iter().any(|o| o.contains("setupwraith")) {
            g.skip(SETUPWRAITH, Reason::SetupwraithOwnsWifi);
        }
    }

    // 3. 「許可」なしでつながる設定か（あとで知らせる）
    if let Some(secure) = tv.sh("getprop ro.adb.secure") {
        let v = secure.trim();
        if v == "0" || v.is_empty() {
            g.notes.push(Note::AdbInsecure);
        }
    }

    // 4. ホーム画面を守る
    if !ov.force_home && !g.send.is_empty() {
        guard_home(&mut g, tv);
    }

    // 5. HDMI などの入力の土台
    if !g.send.is_empty() && !ov.tvinput_allow {
        match tv.sh(&tvinput_script(&g.send)) {
            None => g.notes.push(Note::TvInputCheckFailed),
            Some(out) => {
                let mut hung = vec![];
                let mut nochk = vec![];
                let mut hw = vec![];
                for line in out.lines().map(|l| l.trim()) {
                    if let Some(p) = line.strip_prefix("HUNG ") {
                        hung.push(p.to_string());
                    } else if let Some(p) = line.strip_prefix("NOCHK ") {
                        nochk.push(p.to_string());
                    } else {
                        hw.extend(line.split_whitespace().map(str::to_string));
                    }
                }
                for p in hung {
                    g.skip(&p, Reason::TvInputHung)
                }
                for p in nochk {
                    g.skip(&p, Reason::TvInputUnchecked)
                }
                for p in hw {
                    g.skip(&p, Reason::TvInputHardware)
                }
            }
        }
    }

    // 6. 入力切替・設定を開くアプリ
    if !g.send.is_empty() && !ov.handlers_allow {
        match tv.sh(HANDLERS_QUERY) {
            None => g.notes.push(Note::HandlersCheckFailed),
            Some(out) => {
                let (input, settings) = handlers(&out);
                if settings.is_empty() {
                    g.notes.push(Note::HandlersEmpty);
                }
                for p in input {
                    g.skip(&p, Reason::InputHandler)
                }
                for p in settings {
                    g.skip(&p, Reason::SettingsHandler)
                }
            }
        }
    }
    g
}

fn guard_home(g: &mut Guarded, tv: &mut dyn Tv) {
    let resolved = tv.sh(HOME_QUERY).unwrap_or_default();
    let mut home = resolved
        .lines()
        .last()
        .unwrap_or("")
        .trim()
        .split('/')
        .next()
        .unwrap_or("")
        .to_string();
    let chooser = home == "android";
    if !is_package(&home) {
        home = tv
            .sh("dumpsys activity")
            .as_deref()
            .and_then(home_process)
            .filter(|p| is_package(p))
            .unwrap_or_default();
    }
    let in_list = |g: &Guarded, p: &str| g.send.iter().any(|x| x == p);
    if !home.is_empty() {
        if in_list(g, &home) {
            let claims = tv
                .sh(CLAIM_QUERY)
                .as_deref()
                .and_then(home_claims)
                .unwrap_or_default();
            let others: Vec<String> = launchers_in(&claims)
                .into_iter()
                .filter(|p| *p != home && !in_list(g, p))
                .collect();
            let setup_home = claims
                .iter()
                .any(|c| c.starts_with(&format!("{SETUPWRAITH}/")))
                && !in_list(g, SETUPWRAITH);
            if others.is_empty() {
                g.skip(&home, Reason::HomeOwnerAlone);
            } else if setup_home {
                g.skip(&home, Reason::HomeOwnerSetupwraith);
            }
        }
    } else {
        match tv.sh(CLAIM_QUERY).as_deref().and_then(home_claims) {
            Some(claims) => {
                let launchers = launchers_in(&claims);
                let listed: Vec<String> = launchers
                    .iter()
                    .filter(|p| in_list(g, p))
                    .cloned()
                    .collect();
                let others = launchers.iter().any(|p| !in_list(g, p));
                if !listed.is_empty() && !others {
                    for p in listed {
                        g.skip(&p, Reason::HomeNoDefault)
                    }
                }
            }
            None => {
                for p in STOCK_LAUNCHERS {
                    g.skip(p, Reason::HomeUnknownStock)
                }
            }
        }
        if !g.send.is_empty() {
            g.notes.push(if chooser {
                Note::HomeChooser
            } else {
                Note::HomeUnknown
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// 決めた答えを返す架空のテレビ。知らないコマンドは通信失敗（None）。
    struct FakeTv(HashMap<String, String>);
    impl Tv for FakeTv {
        fn sh(&mut self, command: &str) -> Option<String> {
            if command.starts_with("s=$(getprop ro.build.version.sdk)") {
                return self.0.get("TVINPUT").cloned();
            }
            self.0.get(command).cloned()
        }
    }
    fn tv(pairs: &[(&str, &str)]) -> FakeTv {
        FakeTv(
            pairs
                .iter()
                .map(|(a, b)| (a.to_string(), b.to_string()))
                .collect(),
        )
    }
    fn pk(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }
    const BASE: [(&str, &str); 4] = [
        ("getprop ro.adb.secure", "1"),
        ("TVINPUT", ""),
        (
            HANDLERS_QUERY,
            "==\ncom.android.tv.settings/.MainSettings\n",
        ),
        (HOME_QUERY, "priority=0\ncom.example.stockhome/.Main\n"),
    ];

    #[test]
    fn core_packages_are_never_sent() {
        let mut t = tv(&BASE);
        let g = run(
            &pk(&[
                "com.example.ad",
                "com.android.systemui",
                "com.android.providers.calendar",
                "com.google.android.overlay.modules.x",
            ]),
            &mut t,
            Overrides::default(),
        );
        assert_eq!(g.send, ["com.example.ad", "com.android.providers.calendar"]);
        assert!(g.skipped.iter().all(|(_, r)| *r == Reason::Core));
        // 上書きしても中核は送らない
        let g = run(
            &pk(&["android"]),
            &mut tv(&BASE),
            Overrides {
                force_home: true,
                tvinput_allow: true,
                handlers_allow: true,
                force_setupwraith: true,
            },
        );
        assert!(g.send.is_empty());
    }

    #[test]
    fn setupwraith_kept_when_it_owns_wifi_or_unknown() {
        let mut t = tv(&BASE);
        t.0.insert(
            "dumpsys wifi".into(),
            "Network{ cname=com.google.android.tungsten.setupwraith ssid=x }".into(),
        );
        let g = run(&pk(&[SETUPWRAITH]), &mut t, Overrides::default());
        assert_eq!(
            g.skipped,
            [(SETUPWRAITH.to_string(), Reason::SetupwraithOwnsWifi)]
        );
        let g = run(&pk(&[SETUPWRAITH]), &mut tv(&BASE), Overrides::default());
        assert_eq!(
            g.skipped,
            [(SETUPWRAITH.to_string(), Reason::SetupwraithUnchecked)]
        );
        let mut t = tv(&BASE);
        t.0.insert(
            "dumpsys wifi".into(),
            "cname=android.uid.system:1000".into(),
        );
        assert!(
            run(&pk(&[SETUPWRAITH]), &mut t, Overrides::default())
                .skipped
                .is_empty()
        );
    }

    #[test]
    fn home_owner_kept_when_alone() {
        let mut t = tv(&BASE);
        t.0.insert(
            CLAIM_QUERY.into(),
            "com.example.stockhome/.Main\ncom.android.tv.settings/.FallbackHome\n".into(),
        );
        let g = run(
            &pk(&["com.example.stockhome", "com.example.ad"]),
            &mut t,
            Overrides::default(),
        );
        assert_eq!(g.send, ["com.example.ad"]);
        assert_eq!(g.skipped[0].1, Reason::HomeOwnerAlone);
    }

    #[test]
    fn home_owner_disabled_when_another_launcher_exists() {
        let mut t = tv(&BASE);
        t.0.insert(
            CLAIM_QUERY.into(),
            "com.example.stockhome/.Main\ncom.gathin.tvhome/.Home\n".into(),
        );
        let g = run(
            &pk(&["com.example.stockhome"]),
            &mut t,
            Overrides::default(),
        );
        assert_eq!(g.send, ["com.example.stockhome"]);
    }

    #[test]
    fn home_owner_kept_when_setupwraith_would_take_home() {
        let mut t = tv(&BASE);
        t.0.insert(CLAIM_QUERY.into(), "com.example.stockhome/.Main\ncom.gathin.tvhome/.Home\ncom.google.android.tungsten.setupwraith/.RecoveryActivity\n".into());
        let g = run(
            &pk(&["com.example.stockhome"]),
            &mut t,
            Overrides::default(),
        );
        assert_eq!(g.skipped[0].1, Reason::HomeOwnerSetupwraith);
    }

    #[test]
    fn unknown_home_keeps_stock_launchers_and_says_so() {
        let mut t = tv(&BASE);
        t.0.insert(HOME_QUERY.into(), "/system/bin/sh: cmd: not found".into());
        let g = run(
            &pk(&["com.google.android.tvlauncher", "com.example.ad"]),
            &mut t,
            Overrides::default(),
        );
        assert_eq!(g.send, ["com.example.ad"]);
        assert_eq!(g.skipped[0].1, Reason::HomeUnknownStock);
        assert!(g.notes.contains(&Note::HomeUnknown));
    }

    #[test]
    fn old_firmware_home_from_dumpsys() {
        let mut t = tv(&BASE);
        t.0.insert(HOME_QUERY.into(), "".into());
        t.0.insert(
            "dumpsys activity".into(),
            "  mHomeProcess: ProcessRecord{52f4a5c8 2f4b:com.example.launcher/u0a56}".into(),
        );
        t.0.insert(CLAIM_QUERY.into(), "".into());
        let g = run(&pk(&["com.example.launcher"]), &mut t, Overrides::default());
        assert_eq!(g.skipped[0].1, Reason::HomeOwnerAlone);
    }

    #[test]
    fn chooser_with_listed_launchers_only() {
        let mut t = tv(&BASE);
        t.0.insert(
            HOME_QUERY.into(),
            "android/com.android.internal.app.ResolverActivity".into(),
        );
        t.0.insert(
            CLAIM_QUERY.into(),
            "com.a.home/.A\ncom.b.home/.B\ncom.duokan.factorytest/.FactoryLauncher\n".into(),
        );
        let g = run(
            &pk(&["com.a.home", "com.b.home", "com.example.ad"]),
            &mut t,
            Overrides::default(),
        );
        assert_eq!(g.send, ["com.example.ad"]);
        assert!(g.notes.contains(&Note::HomeChooser));
    }

    #[test]
    fn tvinput_results() {
        let mut t = tv(&BASE);
        t.0.insert(
            "TVINPUT".into(),
            "com.example.hdmi\r\nNOCHK com.example.old\nHUNG com.example.hung\n".into(),
        );
        let g = run(
            &pk(&[
                "com.example.hdmi",
                "com.example.old",
                "com.example.hung",
                "com.example.ad",
            ]),
            &mut t,
            Overrides::default(),
        );
        assert_eq!(g.send, ["com.example.ad"]);
        let reasons: Vec<_> = g.skipped.iter().map(|(_, r)| *r).collect();
        assert_eq!(
            reasons,
            [
                Reason::TvInputHung,
                Reason::TvInputUnchecked,
                Reason::TvInputHardware
            ]
        );
    }

    #[test]
    fn handlers_kept() {
        let mut t = tv(&BASE);
        t.0.insert(
            HANDLERS_QUERY.into(),
            "com.example.livetv/.Tune\n==\ncom.example.settings/.Main\n".into(),
        );
        let g = run(
            &pk(&[
                "com.example.livetv",
                "com.example.settings",
                "com.example.ad",
            ]),
            &mut t,
            Overrides::default(),
        );
        assert_eq!(g.send, ["com.example.ad"]);
    }

    #[test]
    fn failed_queries_are_reported_not_ignored() {
        let mut t = tv(&[
            ("getprop ro.adb.secure", "0"),
            (HOME_QUERY, "com.example.home/.H"),
        ]);
        let g = run(&pk(&["com.example.ad"]), &mut t, Overrides::default());
        assert_eq!(g.send, ["com.example.ad"]);
        assert!(g.notes.contains(&Note::TvInputCheckFailed));
        assert!(g.notes.contains(&Note::HandlersCheckFailed));
        assert!(g.notes.contains(&Note::AdbInsecure));
    }

    #[test]
    fn launcher_filters() {
        assert!(not_a_launcher(
            "com.tcl.keycustomfunctionservice/.FallbackHome"
        ));
        assert!(not_a_launcher("com.duokan.factorytest/.FactoryLauncher"));
        assert!(not_a_launcher(
            "com.google.android.tungsten.setupwraith/.RecoveryActivity"
        ));
        assert!(!not_a_launcher(
            "com.google.android.tvlauncher/.MainActivity"
        ));
        assert!(is_chooser(
            "android/com.android.internal.app.ResolverActivity"
        ));
        assert!(!is_chooser("com.x.y/.Z"));
    }

    #[test]
    fn script_only_takes_package_names() {
        let s = tvinput_script(&pk(&["com.a.b", "x;reboot"]));
        assert!(s.contains("for p in com.a.b ;"));
        assert!(!s.contains("reboot"));
    }
}
