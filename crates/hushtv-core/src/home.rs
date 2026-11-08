//! 広告のないホーム画面（cleanyourtv の launcher、`com.gathin.tvhome`）に切り替えるときの判断。
//! cleanyourtv の launcher/build-and-install.sh と同じ順番・同じ基準。

use crate::names::is_package;
use crate::safety::{CUSTOM_LAUNCHER, STOCK_LAUNCHERS, home_claims, resolved_home};
use serde::{Deserialize, Serialize};

/// ホーム画面アプリの最低 SDK（launcher/app/build.gradle の minSdk）。
pub const MIN_SDK: u32 = 26;

/// 今のホームを見て、止める純正ホームを決めた結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StockChoice {
    /// この純正ホームを止めて入れ替える
    Stock(String),
    /// もう広告のないホームになっている（何も止めない）
    AlreadyOurs,
    /// Android の選択画面で、純正ホームの候補がいくつもある（または 1 つもない）
    Ambiguous(usize),
    /// 知らないホーム（初期設定の画面・ほかのランチャーなど）。答えをそのまま持つ
    Unknown(String),
}

/// `resolve-activity`（ホームの答え）と `query-activities`（ホームに答える候補）から決める。
pub fn choose_stock(home_answer: &str, claims_answer: &str) -> StockChoice {
    let home = resolved_home(home_answer)
        .map(|c| c.split('/').next().unwrap_or("").to_string())
        .or_else(|| {
            let last = home_answer.lines().last().unwrap_or("").trim();
            (!last.is_empty()).then(|| last.split('/').next().unwrap_or("").to_string())
        })
        .unwrap_or_default();
    if STOCK_LAUNCHERS.contains(&home.as_str()) {
        return StockChoice::Stock(home);
    }
    if home == CUSTOM_LAUNCHER {
        return StockChoice::AlreadyOurs;
    }
    if home == "android" {
        let claims = home_claims(claims_answer).unwrap_or_default();
        let found: Vec<&str> = STOCK_LAUNCHERS
            .iter()
            .copied()
            .filter(|k| claims.iter().any(|c| c.split('/').next() == Some(k)))
            .collect();
        return match found.as_slice() {
            [one] => StockChoice::Stock((*one).to_string()),
            _ => StockChoice::Ambiguous(found.len()),
        };
    }
    StockChoice::Unknown(home)
}

/// 切り替えの記録（戻すときに使う）。`~/.local/share/hushtv/home-<host>.json` など。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HomeRecord {
    pub created: String,
    pub host: String,
    /// 止めた純正ホーム（止めていなければ None）
    pub stock: Option<String>,
    /// 切り替える前のホームのコンポーネント
    pub previous: Option<String>,
    /// hushtv が入れたか（もとから入っていたら false。戻すときに消さない）
    pub installed_by_hushtv: bool,
}

/// 入れ替えのあとに流すコマンド（スクリプトの printf | tv.py --batch と同じ並び）。
pub fn switch_commands(stock: Option<&str>) -> Vec<String> {
    let mut c = vec![
        format!(
            "pm grant {CUSTOM_LAUNCHER} android.permission.ACCESS_FINE_LOCATION 2>/dev/null; echo location-granted"
        ),
        format!("cmd package compile -m speed -f {CUSTOM_LAUNCHER}"),
    ];
    if let Some(s) = stock.filter(|s| is_package(s)) {
        c.push(format!("pm disable-user --user 0 {s}"));
    }
    c.push(format!(
        "cmd package set-home-activity --user 0 {CUSTOM_LAUNCHER}/.MainActivity"
    ));
    c
}

/// 純正ホームに戻すコマンド。
pub fn back_commands(stock: &str) -> Vec<String> {
    if !is_package(stock) {
        return vec![];
    }
    vec![
        format!("pm enable --user 0 {stock}"),
        format!(
            "cmd package set-home-activity --user 0 $(cmd package resolve-activity --brief -a android.intent.action.MAIN -c android.intent.category.HOME {stock} | tail -n 1)"
        ),
    ]
}

/// ホームが広告のないホームになったか。
pub fn is_ours(home_answer: &str) -> bool {
    resolved_home(home_answer)
        .map(|c| c.starts_with(&format!("{CUSTOM_LAUNCHER}/")))
        .unwrap_or(false)
}

/// ホームが指定の純正ホームに戻ったか。
pub fn is_stock(home_answer: &str, stock: &str) -> bool {
    resolved_home(home_answer)
        .map(|c| c.starts_with(&format!("{stock}/")))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stock_launcher_is_chosen() {
        let a = "priority=0 preferredOrder=0\ncom.google.android.tvlauncher/.MainActivity";
        assert_eq!(
            choose_stock(a, ""),
            StockChoice::Stock("com.google.android.tvlauncher".into())
        );
    }

    #[test]
    fn already_ours() {
        assert_eq!(
            choose_stock("com.gathin.tvhome/.MainActivity", ""),
            StockChoice::AlreadyOurs
        );
    }

    #[test]
    fn chooser_with_one_stock_candidate() {
        let claims =
            "com.google.android.apps.tv.launcherx/.MainActivity\ncom.example.other/.Home\n";
        assert_eq!(
            choose_stock("android/com.android.internal.app.ResolverActivity", claims),
            StockChoice::Stock("com.google.android.apps.tv.launcherx".into())
        );
    }

    #[test]
    fn chooser_with_two_stock_candidates_is_ambiguous() {
        let claims =
            "com.google.android.apps.tv.launcherx/.Main\ncom.google.android.tvlauncher/.Main\n";
        assert_eq!(
            choose_stock("android/com.android.internal.app.ResolverActivity", claims),
            StockChoice::Ambiguous(2)
        );
    }

    #[test]
    fn setup_wizard_is_unknown() {
        assert_eq!(
            choose_stock(
                "com.google.android.tungsten.setupwraith/.RecoveryActivity",
                ""
            ),
            StockChoice::Unknown("com.google.android.tungsten.setupwraith".into())
        );
    }

    #[test]
    fn commands_disable_only_a_valid_stock() {
        let c = switch_commands(Some("com.google.android.tvlauncher"));
        assert!(
            c.iter()
                .any(|x| x == "pm disable-user --user 0 com.google.android.tvlauncher")
        );
        assert!(c.last().unwrap().contains("set-home-activity"));
        let c = switch_commands(Some("x; reboot"));
        assert!(!c.iter().any(|x| x.contains("disable-user")));
        assert!(back_commands("bad name;").is_empty());
    }

    #[test]
    fn home_checks() {
        assert!(is_ours("com.gathin.tvhome/.MainActivity"));
        assert!(!is_ours("com.google.android.tvlauncher/.Main"));
        assert!(is_stock(
            "com.google.android.tvlauncher/.Main",
            "com.google.android.tvlauncher"
        ));
    }
}
