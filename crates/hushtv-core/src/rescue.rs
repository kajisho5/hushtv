//! 修復モード：cleanyourtv・CleanMyTV（や手作業の adb）で調子が悪くなったテレビを直す。
//!
//! 「止める」より「戻す」の方が安全なので、修復はどれも有効化（`pm enable`）と、ユーザー 0 から
//! 消されたアプリの再インストール（`cmd package install-existing`、端末に残っている本体を戻すだけ）
//! だけで行う。テレビと通信しない判定だけをここに置く。
//!
//! 修復の段階：
//! 1. **応急処置**（おすすめ）：起動・ホーム・設定・入力・Wi-Fi にかかわるものだけ戻す
//! 2. **記録どおりに戻す**：cleanyourtv・CleanMyTV・hushtv が残した戻す記録（同じ形式）を使う
//! 3. **全部戻す**：機種別リストに載っているアプリで、止まっている・消されているものを全部戻す

use crate::names::is_package;
use crate::safety::{
    CUSTOM_LAUNCHER, Lang, NOT_LAUNCHERS, STOCK_LAUNCHERS, is_chooser, is_core, not_a_launcher,
    resolved_home,
};
use regex::Regex;
use serde::Serialize;
use std::collections::BTreeSet;
use std::sync::LazyLock;

/// CleanMyTV のホーム画面アプリ。
pub const CLEANMYTV_LAUNCHER: &str = "io.github.wingedpanther.cleanmytv.home";
/// 設定アプリ。
pub const SETTINGS_APPS: [&str; 2] = ["com.android.tv.settings", "com.android.settings"];
pub const SETUPWRAITH: &str = "com.google.android.tungsten.setupwraith";

/// 入力切替・チューナー・テレビの制御にかかわる名前（止まっていたら戻す候補。名前からの推測）。
static INPUT_LIKE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)tvinput|tv\.input|\.input\b|hdmi|tuner|livetv|tvcenter|wwtv|tvplayer|mediatek\.tv|\.cec\b|hdmicec|tvsetting|channel|\.epg|\bdtv|isdb|tvservice|tv\.service|inputsource|sourcemanager").unwrap()
});

/// 診断に使う、テレビから読んだもの。
#[derive(Debug, Clone, Default)]
pub struct Facts {
    /// ユーザー 0 に入っているアプリ
    pub installed: BTreeSet<String>,
    /// ユーザー 0 で止まっているアプリ
    pub disabled: BTreeSet<String>,
    /// 端末にあるアプリ全部（ユーザー 0 から消されたものも含む。`pm list packages -u`）
    pub all: BTreeSet<String>,
    /// `resolve-activity` の答え
    pub home_answer: String,
    /// 設定を開くアプリの問い合わせの答え
    pub settings_answer: String,
    /// アニメーション速度（0 だと固まったように見える機種がある）
    pub scales: Vec<(String, String)>,
}

/// 見つかった問題。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Problem {
    /// Android の中核が止まっている（起動しなくなる・再起動を繰り返す原因）
    CoreDisabled { packages: Vec<String> },
    /// ホームボタンで開くホーム画面がない（真っ黒・「ホームアプリを選択」・初期設定画面が出る）
    NoHome { now: Option<String> },
    /// 設定が開かない
    SettingsBroken,
    /// 入力切替・チューナーまわりが止まっている（「信号なし」・入力が開かない）
    InputDisabled { packages: Vec<String> },
    /// 初期設定画面（setupwraith）が止まっている（Wi-Fi が消える・キャストできない原因）
    SetupwraithDisabled,
    /// ユーザー 0 から消された（アンインストールされた）アプリがある
    RemovedForUser { packages: Vec<String> },
    /// 機種別リストに載っているアプリが止まっている（ほかのツールで止めたもの）
    Debloated { packages: Vec<String> },
    /// アニメーションが 0 になっている
    AnimationsOff,
}

impl Problem {
    /// 重さ（大きいほど先に直す）。
    pub fn severity(&self) -> u8 {
        match self {
            Problem::CoreDisabled { .. } => 5,
            Problem::NoHome { .. } => 4,
            Problem::SettingsBroken => 4,
            Problem::InputDisabled { .. } => 3,
            Problem::SetupwraithDisabled => 2,
            Problem::RemovedForUser { .. } => 2,
            Problem::AnimationsOff => 1,
            Problem::Debloated { .. } => 0,
        }
    }

    pub fn title(&self, lang: Lang) -> String {
        let ja = lang == Lang::Ja;
        match self {
            Problem::CoreDisabled { packages } => {
                if ja {
                    format!(
                        "Android の中核が {} 個止まっています（起動しない・再起動を繰り返す原因）",
                        packages.len()
                    )
                } else {
                    format!(
                        "{} core Android packages are off (boot loops)",
                        packages.len()
                    )
                }
            }
            Problem::NoHome { now } => {
                let n = now.clone().unwrap_or_else(|| "-".into());
                if ja {
                    format!("ホームボタンで開くホーム画面がありません（今: {n}）")
                } else {
                    format!("Home has no working launcher (now: {n})")
                }
            }
            Problem::SettingsBroken => {
                if ja {
                    "設定が開きません".into()
                } else {
                    "Settings does not open".into()
                }
            }
            Problem::InputDisabled { packages } => {
                if ja {
                    format!(
                        "入力切替・チューナーまわりが {} 個止まっています（「信号なし」の原因）",
                        packages.len()
                    )
                } else {
                    format!(
                        "{} input/tuner packages are off (\"No Signal\")",
                        packages.len()
                    )
                }
            }
            Problem::SetupwraithDisabled => {
                if ja {
                    "初期設定画面が止まっています（Wi-Fi が消える・キャストできない原因）".into()
                } else {
                    "The setup wizard is off (lost Wi-Fi, casting fails)".into()
                }
            }
            Problem::RemovedForUser { packages } => {
                if ja {
                    format!(
                        "消されたアプリが {} 個あります（端末には本体が残っているので戻せます）",
                        packages.len()
                    )
                } else {
                    format!(
                        "{} apps were uninstalled for the user (they can be brought back)",
                        packages.len()
                    )
                }
            }
            Problem::Debloated { packages } => {
                if ja {
                    format!("機種別リストのアプリが {} 個止まっています", packages.len())
                } else {
                    format!("{} apps from the debloat lists are off", packages.len())
                }
            }
            Problem::AnimationsOff => {
                if ja {
                    "アニメーションが 0 です（画面が固まったように見えることがあります）".into()
                } else {
                    "Animations are set to 0 (the UI can look frozen)".into()
                }
            }
        }
    }
}

/// 修復の段階。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    /// 応急処置：起動・ホーム・設定・入力・Wi-Fi にかかわるものだけ
    Quick,
    /// 応急処置＋機種別リストに載っているものを全部戻す
    All,
}

/// 機種別リストに載っているパッケージ全部（同梱 456 機種ぶん）。
pub fn list_universe() -> &'static BTreeSet<String> {
    static U: LazyLock<BTreeSet<String>> = LazyLock::new(|| {
        crate::lists::all()
            .iter()
            .flat_map(|l| l.files.iter())
            .filter_map(|(_, text)| crate::list::parse(text).ok())
            .flat_map(|l| l.disable.into_iter().map(|e| e.package))
            .collect()
    });
    &U
}

fn home_ok(answer: &str) -> (bool, Option<String>) {
    let h = resolved_home(answer);
    let ok = h
        .as_deref()
        .is_some_and(|h| !is_chooser(h) && !not_a_launcher(h));
    (ok, h)
}

fn launcher_like(p: &str) -> bool {
    STOCK_LAUNCHERS.contains(&p)
        || p == CUSTOM_LAUNCHER
        || p == CLEANMYTV_LAUNCHER
        || p.to_lowercase().contains("launcher")
        || p.to_lowercase().ends_with("tvhome")
}

/// 問題を探す（重い順）。
pub fn diagnose(f: &Facts) -> Vec<Problem> {
    let mut out = vec![];
    let core: Vec<String> = f.disabled.iter().filter(|p| is_core(p)).cloned().collect();
    if !core.is_empty() {
        out.push(Problem::CoreDisabled { packages: core });
    }
    let (ok, now) = home_ok(&f.home_answer);
    if !ok {
        out.push(Problem::NoHome { now });
    }
    if !f.settings_answer.lines().any(|l| l.contains('/')) {
        out.push(Problem::SettingsBroken);
    }
    let input: Vec<String> = f
        .disabled
        .iter()
        .filter(|p| INPUT_LIKE.is_match(p) && !is_core(p))
        .cloned()
        .collect();
    if !input.is_empty() {
        out.push(Problem::InputDisabled { packages: input });
    }
    if f.disabled.contains(SETUPWRAITH) {
        out.push(Problem::SetupwraithDisabled);
    }
    let universe = list_universe();
    let removed: Vec<String> = f
        .all
        .difference(&f.installed)
        .filter(|p| {
            is_core(p)
                || universe.contains(*p)
                || launcher_like(p)
                || SETTINGS_APPS.contains(&p.as_str())
                || INPUT_LIKE.is_match(p)
        })
        .cloned()
        .collect();
    if !removed.is_empty() {
        out.push(Problem::RemovedForUser { packages: removed });
    }
    if f.scales
        .iter()
        .any(|(_, v)| v.trim() == "0" || v.trim() == "0.0")
    {
        out.push(Problem::AnimationsOff);
    }
    let debloated: Vec<String> = f
        .disabled
        .iter()
        .filter(|p| universe.contains(*p))
        .cloned()
        .collect();
    if !debloated.is_empty() {
        out.push(Problem::Debloated {
            packages: debloated,
        });
    }
    out.sort_by_key(|p| std::cmp::Reverse(p.severity()));
    out
}

/// 直すコマンド（1 行 1 コマンド）。中身は「有効化」「再インストール」「ホームの指定」「速度を 1 に」だけ。
pub fn fix_commands(problems: &[Problem], f: &Facts, level: Level) -> Vec<String> {
    let mut enable: BTreeSet<String> = BTreeSet::new();
    let mut reinstall: BTreeSet<String> = BTreeSet::new();
    let mut set_home = false;
    let mut scales = false;
    for p in problems {
        match p {
            Problem::CoreDisabled { packages } | Problem::InputDisabled { packages } => {
                enable.extend(packages.iter().cloned())
            }
            Problem::NoHome { .. } => {
                // 止まっているホーム画面アプリを戻す（純正を先に）
                for l in STOCK_LAUNCHERS
                    .iter()
                    .map(|s| s.to_string())
                    .chain(f.disabled.iter().filter(|p| launcher_like(p)).cloned())
                {
                    if f.disabled.contains(&l) {
                        enable.insert(l);
                    }
                }
                // 初期設定画面は止めたままだとホームを奪うことがあるので、戻すときは一緒に戻す
                if f.disabled.contains(SETUPWRAITH) {
                    enable.insert(SETUPWRAITH.into());
                }
                set_home = true;
            }
            Problem::SettingsBroken => {
                for s in SETTINGS_APPS {
                    if f.disabled.contains(s) {
                        enable.insert(s.into());
                    }
                    if f.all.contains(s) && !f.installed.contains(s) {
                        reinstall.insert(s.into());
                    }
                }
            }
            Problem::SetupwraithDisabled => {
                enable.insert(SETUPWRAITH.into());
            }
            Problem::RemovedForUser { packages } => {
                for p in packages {
                    let vital = is_core(p)
                        || launcher_like(p)
                        || SETTINGS_APPS.contains(&p.as_str())
                        || INPUT_LIKE.is_match(p);
                    if vital || level == Level::All {
                        reinstall.insert(p.clone());
                    }
                }
            }
            Problem::AnimationsOff => scales = true,
            Problem::Debloated { packages } => {
                if level == Level::All {
                    enable.extend(packages.iter().cloned());
                    set_home = true;
                }
            }
        }
    }
    let mut cmds: Vec<String> = reinstall
        .iter()
        .filter(|p| is_package(p))
        .map(|p| format!("cmd package install-existing --user 0 {p}"))
        .collect();
    cmds.extend(
        enable
            .iter()
            .filter(|p| is_package(p))
            .map(|p| format!("pm enable {p}")),
    );
    if set_home {
        // 純正のホームの中から、入っているものに向ける（なければ Android に任せる）
        let target = STOCK_LAUNCHERS
            .iter()
            .find(|s| f.installed.contains(**s) || f.all.contains(**s));
        if let Some(t) = target {
            cmds.push(format!(
                "cmd package set-home-activity --user 0 $(cmd package resolve-activity --brief -a android.intent.action.MAIN -c android.intent.category.HOME {t} | tail -n 1)"
            ));
        }
    }
    if scales {
        cmds.extend(
            crate::record::SETTINGS
                .iter()
                .map(|k| format!("settings put global {k} 1.0")),
        );
    }
    cmds
}

/// ホームに答えても戻し先にしないもの（診断の表示用）。
pub fn is_not_launcher(p: &str) -> bool {
    NOT_LAUNCHERS.contains(&p)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(v: &[&str]) -> BTreeSet<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    fn broken_tv() -> Facts {
        Facts {
            installed: set(&[
                "com.google.android.tvlauncher",
                "com.android.tv.settings",
                "com.example.ad",
                "com.mediatek.wwtv.tvcenter",
                SETUPWRAITH,
                "com.android.permissioncontroller",
            ]),
            disabled: set(&[
                "com.google.android.tvlauncher",
                "com.example.ad",
                "com.mediatek.wwtv.tvcenter",
                SETUPWRAITH,
                "com.android.permissioncontroller",
            ]),
            all: set(&[
                "com.google.android.tvlauncher",
                "com.android.tv.settings",
                "com.example.ad",
                "com.mediatek.wwtv.tvcenter",
                SETUPWRAITH,
                "com.android.permissioncontroller",
                "com.android.packageinstaller",
            ]),
            home_answer: "com.google.android.tungsten.setupwraith/.RecoveryActivity".into(),
            settings_answer: "com.android.tv.settings/.MainSettings".into(),
            scales: vec![("window_animation_scale".into(), "0".into())],
        }
    }

    #[test]
    fn finds_problems_in_order() {
        let p = diagnose(&broken_tv());
        let kinds: Vec<u8> = p.iter().map(Problem::severity).collect();
        assert!(kinds.windows(2).all(|w| w[0] >= w[1]));
        assert!(matches!(p[0], Problem::CoreDisabled { .. }));
        assert!(p.iter().any(|x| matches!(x, Problem::NoHome { .. })));
        assert!(p.iter().any(|x| matches!(x, Problem::InputDisabled { .. })));
        assert!(p.contains(&Problem::SetupwraithDisabled));
        assert!(p.contains(&Problem::AnimationsOff));
        assert!(p.iter().any(|x| matches!(x, Problem::RemovedForUser { packages } if packages == &vec!["com.android.packageinstaller".to_string()])));
    }

    #[test]
    fn quick_fix_only_enables_what_matters() {
        let f = broken_tv();
        let cmds = fix_commands(&diagnose(&f), &f, Level::Quick);
        assert!(cmds.contains(&"pm enable com.android.permissioncontroller".into()));
        assert!(cmds.contains(&"pm enable com.google.android.tvlauncher".into()));
        assert!(cmds.contains(&"pm enable com.mediatek.wwtv.tvcenter".into()));
        assert!(cmds.contains(&format!("pm enable {SETUPWRAITH}")));
        assert!(cmds.contains(
            &"cmd package install-existing --user 0 com.android.packageinstaller".into()
        ));
        assert!(
            cmds.iter()
                .any(|c| c.starts_with("cmd package set-home-activity"))
        );
        assert!(cmds.contains(&"settings put global window_animation_scale 1.0".into()));
        assert!(
            !cmds.contains(&"pm enable com.example.ad".into()),
            "応急処置で広告アプリは戻さない"
        );
        // 止める・消すコマンドは決して出さない
        assert!(
            cmds.iter()
                .all(|c| !c.contains("disable") && !c.contains("uninstall"))
        );
    }

    #[test]
    fn healthy_tv_has_no_problems() {
        let f = Facts {
            installed: set(&["com.google.android.tvlauncher", "com.android.tv.settings"]),
            all: set(&["com.google.android.tvlauncher", "com.android.tv.settings"]),
            home_answer: "com.google.android.tvlauncher/.MainActivity".into(),
            settings_answer: "com.android.tv.settings/.MainSettings".into(),
            ..Default::default()
        };
        assert!(diagnose(&f).is_empty());
    }

    #[test]
    fn universe_is_large() {
        assert!(list_universe().len() > 1000);
    }
}
