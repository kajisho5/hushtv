//! 実行前の計画・機種の当てはめ・点検（ファーム更新のあとのずれ）。

use crate::list::DebloatList;
use crate::lists::ListInfo;
use crate::record::TvState;
use crate::safety::{Guarded, Note, Reason};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// 変える前に見せる計画。
#[derive(Debug, Clone, Default, Serialize)]
pub struct Plan {
    /// 止める（検査を通った、今動いているもの）
    pub to_disable: Vec<String>,
    /// 元から止まっている
    pub already_disabled: Vec<String>,
    /// このテレビに入っていない
    pub absent: Vec<String>,
    /// 安全装置で残すもの
    pub protected: Vec<(String, Reason)>,
    pub notes: Vec<Note>,
}

impl Plan {
    /// リストの止める行（`chosen`）と検査の結果とテレビの状態から作る。
    pub fn build(chosen: &[String], guarded: &Guarded, s: &TvState) -> Plan {
        let mut p = Plan {
            protected: guarded.skipped.clone(),
            notes: guarded.notes.clone(),
            ..Default::default()
        };
        for name in chosen {
            if guarded.skipped.iter().any(|(n, _)| n == name) {
                continue;
            }
            if !s.installed.contains(name) {
                p.absent.push(name.clone());
            } else if s.disabled.contains(name) {
                p.already_disabled.push(name.clone());
            } else if guarded.send.contains(name) {
                p.to_disable.push(name.clone());
            }
        }
        p
    }
}

/// テレビに入っているアプリとリストの重なりで、近い機種のリストを点数順に並べる。
/// （点数＝リストの止める行のうち、このテレビに入っている割合。同点なら数の多い方）
pub fn rank_lists<'a>(
    installed: &BTreeSet<String>,
    model: Option<&str>,
    lists: &'a [ListInfo],
) -> Vec<(f64, usize, &'a ListInfo)> {
    let model = model
        .map(|m| m.trim().to_lowercase())
        .filter(|m| m.len() >= 3);
    let mut out: Vec<(f64, usize, &ListInfo)> = lists
        .iter()
        .filter_map(|info| {
            let list = crate::list::parse(&info.files.first()?.1).ok()?;
            let total = list.disable.len();
            let hit = list
                .disable
                .iter()
                .filter(|e| installed.contains(&e.package))
                .count();
            let mut score = hit as f64 / total.max(1) as f64;
            if let Some(m) = &model {
                let hay = format!("{} {}", info.id, info.title())
                    .to_lowercase()
                    .replace(['-', '_'], " ");
                if hay.contains(&m.replace(['-', '_'], " ")) {
                    score += 1.0;
                }
            }
            (hit > 0).then_some((score, hit, info))
        })
        .collect();
    out.sort_by(|a, b| {
        b.0.total_cmp(&a.0)
            .then(b.1.cmp(&a.1))
            .then(a.2.id.cmp(&b.2.id))
    });
    out
}

/// 片付けたときの控え（hushtv 独自。点検に使う）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Applied {
    pub host: String,
    pub list_id: String,
    pub at: String,
    /// hushtv が止めたもの
    pub disabled: Vec<String>,
    /// そのとき入っていたアプリ全部（新しく増えたアプリを見つけるため）
    pub installed: Vec<String>,
    /// そのときのホーム
    pub home: Option<String>,
}

/// 点検の結果。
#[derive(Debug, Clone, Default, Serialize)]
pub struct Drift {
    /// 止めたのに、また動いている
    pub returned: Vec<String>,
    /// 片付けのあとに増えたアプリ（動いているもの）
    pub new_apps: Vec<String>,
    /// ホームが変わっていた（変わる前, 今）
    pub home_changed: Option<(Option<String>, Option<String>)>,
}

impl Drift {
    pub fn is_clean(&self) -> bool {
        self.returned.is_empty() && self.new_apps.is_empty() && self.home_changed.is_none()
    }
}

/// 控えと今のテレビを比べる。
pub fn drift(a: &Applied, now: &TvState) -> Drift {
    let enabled = |p: &String| now.installed.contains(p) && !now.disabled.contains(p);
    let before: BTreeSet<&String> = a.installed.iter().collect();
    let home_pkg = |h: &Option<String>| {
        h.as_ref()
            .map(|h| h.split('/').next().unwrap_or("").to_string())
    };
    Drift {
        returned: a.disabled.iter().filter(|p| enabled(p)).cloned().collect(),
        new_apps: now
            .installed
            .iter()
            .filter(|p| !before.contains(p) && enabled(p))
            .cloned()
            .collect(),
        home_changed: (a.home.is_some() && home_pkg(&a.home) != home_pkg(&now.home))
            .then(|| (a.home.clone(), now.home.clone())),
    }
}

/// リストの止める行の名前。
pub fn names(list: &DebloatList) -> Vec<String> {
    list.disable.iter().map(|e| e.package.clone()).collect()
}

/// 各パッケージが、同梱リストのいくつで「止める」になっているか（画面の「41 機種で停止」）。
pub fn list_counts() -> &'static std::collections::HashMap<String, usize> {
    static C: std::sync::LazyLock<std::collections::HashMap<String, usize>> =
        std::sync::LazyLock::new(|| {
            let mut m = std::collections::HashMap::new();
            for info in crate::lists::all() {
                if let Some((_, text)) = info.files.first()
                    && let Ok(list) = crate::list::parse(text)
                {
                    for e in list.disable {
                        *m.entry(e.package).or_insert(0) += 1;
                    }
                }
            }
            m
        });
    &C
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(v: &[&str]) -> BTreeSet<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn plan_sorts_each_name() {
        let s = TvState {
            installed: set(&["com.a.on", "com.a.off", "com.a.core"]),
            disabled: set(&["com.a.off"]),
            ..Default::default()
        };
        let g = Guarded {
            send: vec!["com.a.on".into(), "com.a.off".into(), "com.a.gone".into()],
            skipped: vec![("com.a.core".into(), Reason::Core)],
            notes: vec![],
        };
        let p = Plan::build(
            &[
                "com.a.on".into(),
                "com.a.off".into(),
                "com.a.gone".into(),
                "com.a.core".into(),
            ],
            &g,
            &s,
        );
        assert_eq!(p.to_disable, ["com.a.on"]);
        assert_eq!(p.already_disabled, ["com.a.off"]);
        assert_eq!(p.absent, ["com.a.gone"]);
        assert_eq!(p.protected.len(), 1);
    }

    #[test]
    fn drift_finds_returned_new_and_home() {
        let a = Applied {
            host: "h".into(),
            list_id: "x".into(),
            at: "t".into(),
            disabled: vec!["com.a.ad".into(), "com.a.track".into()],
            installed: vec!["com.a.ad".into(), "com.a.track".into(), "com.a.home".into()],
            home: Some("com.a.home/.H".into()),
        };
        let now = TvState {
            installed: set(&[
                "com.a.ad",
                "com.a.track",
                "com.a.home",
                "com.a.promo",
                "com.a.stock",
            ]),
            disabled: set(&["com.a.track"]),
            home: Some("com.a.stock/.S".into()),
            ..Default::default()
        };
        let d = drift(&a, &now);
        assert_eq!(d.returned, ["com.a.ad"]);
        assert_eq!(d.new_apps, ["com.a.promo", "com.a.stock"]);
        assert!(d.home_changed.is_some());
        assert!(!d.is_clean());
    }

    #[test]
    fn ranks_bundled_lists() {
        let lists = crate::lists::all();
        let first = crate::list::parse(&lists[0].files[0].1).unwrap();
        let installed: BTreeSet<String> = first.disable.iter().map(|e| e.package.clone()).collect();
        let ranked = rank_lists(&installed, None, &lists);
        assert!((ranked[0].0 - 1.0).abs() < 1e-9);
    }
}
