//! 安全モード：少しずつ止めて、そのつどテレビが無事かを確かめる。
//!
//! 1 組止めるたびに、次の 3 つを確かめる（hushtv 独自。元の cleanyourtv にはない）。
//! 1. テレビがまだ答える（通信が切れていない）
//! 2. ホームボタンで開くものがある（選択画面・初期設定画面・仮のホームではない）
//! 3. 設定を開くアプリがある（開発者向けオプションへ戻る道がある）
//!
//! だめなら、その組だけ元に戻して止まる。

use crate::safety::{HOME_QUERY, Tv, is_chooser, not_a_launcher, resolved_home};
use serde::Serialize;

pub const SETTINGS_QUERY: &str =
    "cmd package query-activities --brief -a android.settings.SETTINGS";

/// 確かめた結果。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Health {
    Ok,
    /// テレビが答えない
    NoAnswer,
    /// ホームボタンで開くホーム画面がない
    NoHome,
    /// 設定を開くアプリがない
    NoSettings,
}

/// 止める前の状態（比べる基準）。
#[derive(Debug, Clone, Default)]
pub struct Baseline {
    /// 止める前にホームを確かめられたか（古いファームは確かめられない）
    pub home_known: bool,
    /// 止める前に設定アプリを確かめられたか
    pub settings_known: bool,
}

fn home_ok(answer: &str) -> bool {
    resolved_home(answer).is_some_and(|h| !is_chooser(&h) && !not_a_launcher(&h))
}

fn settings_ok(answer: &str) -> bool {
    answer
        .lines()
        .any(|l| l.trim().contains('/') && !l.contains("No activity"))
}

/// 止める前に、何を確かめられるかを調べる（止める前から確かめられないものは、あとで判断に使わない）。
pub fn baseline(tv: &mut dyn Tv) -> Baseline {
    Baseline {
        home_known: tv.sh(HOME_QUERY).is_some_and(|a| home_ok(&a)),
        settings_known: tv.sh(SETTINGS_QUERY).is_some_and(|a| settings_ok(&a)),
    }
}

/// 1 組止めたあとに確かめる。
pub fn check(tv: &mut dyn Tv, base: &Baseline) -> Health {
    let Some(echo) = tv.sh("echo hushtv-ok") else {
        return Health::NoAnswer;
    };
    if !echo.contains("hushtv-ok") {
        return Health::NoAnswer;
    }
    if base.home_known && !tv.sh(HOME_QUERY).is_some_and(|a| home_ok(&a)) {
        return Health::NoHome;
    }
    if base.settings_known && !tv.sh(SETTINGS_QUERY).is_some_and(|a| settings_ok(&a)) {
        return Health::NoSettings;
    }
    Health::Ok
}

/// 止めるものを組に分ける（組の大きさは 1 以上）。
pub fn batches(names: &[String], size: usize) -> Vec<Vec<String>> {
    names.chunks(size.max(1)).map(<[String]>::to_vec).collect()
}

/// 安全モードの結果。
#[derive(Debug, Clone, Default, Serialize)]
pub struct SafeRun {
    /// 止めて、問題がなかったもの
    pub disabled: Vec<String>,
    /// 問題が出て元に戻した組
    pub rolled_back: Vec<String>,
    /// 問題が出たので送らなかったもの
    pub not_sent: Vec<String>,
    /// 止まった理由
    pub stopped: Option<Health>,
}

/// 少しずつ止める。1 組ごとに `pm disable-user` を送って確かめ、だめならその組に `pm enable` を送って止まる。
pub fn run(
    tv: &mut dyn Tv,
    names: &[String],
    size: usize,
    mut progress: impl FnMut(usize, usize),
) -> SafeRun {
    let base = baseline(tv);
    let groups = batches(names, size);
    let total = groups.len();
    let mut out = SafeRun::default();
    for (i, group) in groups.iter().enumerate() {
        progress(i + 1, total);
        for p in group {
            let _ = tv.sh(&format!("pm disable-user --user 0 {p}"));
        }
        match check(tv, &base) {
            Health::Ok => out.disabled.extend(group.iter().cloned()),
            bad => {
                for p in group {
                    let _ = tv.sh(&format!("pm enable {p}"));
                }
                out.rolled_back = group.clone();
                out.not_sent = groups[i + 1..].concat();
                out.stopped = Some(bad);
                break;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    /// 「止めたもの」を覚える架空のテレビ。`breaks_home` を止めるとホームがなくなる。
    struct FakeTv {
        off: BTreeSet<String>,
        breaks_home: &'static str,
        sent: Vec<String>,
    }
    impl Tv for FakeTv {
        fn sh(&mut self, c: &str) -> Option<String> {
            self.sent.push(c.to_string());
            if let Some(p) = c.strip_prefix("pm disable-user --user 0 ") {
                self.off.insert(p.to_string());
                return Some("ok".into());
            }
            if let Some(p) = c.strip_prefix("pm enable ") {
                self.off.remove(p);
                return Some("ok".into());
            }
            match c {
                "echo hushtv-ok" => Some("hushtv-ok\n".into()),
                HOME_QUERY if self.off.contains(self.breaks_home) => {
                    Some("android/com.android.internal.app.ResolverActivity".into())
                }
                HOME_QUERY => Some("com.example.home/.Main".into()),
                SETTINGS_QUERY => Some("com.android.tv.settings/.MainSettings".into()),
                _ => None,
            }
        }
    }
    fn names(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn stops_and_rolls_back_the_bad_batch() {
        let mut tv = FakeTv {
            off: BTreeSet::new(),
            breaks_home: "com.c",
            sent: vec![],
        };
        let r = run(
            &mut tv,
            &names(&["com.a", "com.b", "com.c", "com.d", "com.e"]),
            2,
            |_, _| {},
        );
        assert_eq!(r.disabled, ["com.a", "com.b"]);
        assert_eq!(r.rolled_back, ["com.c", "com.d"]);
        assert_eq!(r.not_sent, ["com.e"]);
        assert_eq!(r.stopped, Some(Health::NoHome));
        assert!(!tv.off.contains("com.c") && !tv.off.contains("com.d"));
        assert!(!tv.sent.iter().any(|c| c.ends_with("com.e")));
    }

    #[test]
    fn all_fine() {
        let mut tv = FakeTv {
            off: BTreeSet::new(),
            breaks_home: "com.none",
            sent: vec![],
        };
        let r = run(&mut tv, &names(&["com.a", "com.b", "com.c"]), 5, |_, _| {});
        assert_eq!(r.disabled.len(), 3);
        assert!(r.stopped.is_none());
    }

    #[test]
    fn silent_tv_stops_the_run() {
        struct Dead;
        impl Tv for Dead {
            fn sh(&mut self, _: &str) -> Option<String> {
                None
            }
        }
        let r = run(&mut Dead, &names(&["com.a", "com.b"]), 1, |_, _| {});
        assert_eq!(r.stopped, Some(Health::NoAnswer));
        assert_eq!(r.not_sent, ["com.b"]);
    }
}
