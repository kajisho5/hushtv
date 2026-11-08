//! テレビのシェルへ渡す名前の形の検査。
//!
//! リストの中身やテレビの答えは、そのままテレビのシェルのコマンドに入る。シェルの記号を運べる
//! 文字列は通さない（元の cleanyourtv の PACKAGE_RE / HOME_RE / SCALE_RE と同じ形）。

use regex::Regex;
use std::sync::LazyLock;

/// パッケージ名の形。例 `com.example.app`
pub static PACKAGE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[A-Za-z_][A-Za-z0-9_]*(\.[A-Za-z0-9_]+)+$").unwrap());

/// ホーム画面のコンポーネントの形。例 `com.example.home/.MainActivity`
pub static HOME_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[A-Za-z_][A-Za-z0-9_.]*/[A-Za-z0-9_.$]+$").unwrap());

/// アニメーション速度の値の形。
pub static SCALE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^-?\d+(\.\d+)?$").unwrap());

/// パッケージ名の形をしているか。
pub fn is_package(name: &str) -> bool {
    PACKAGE_RE.is_match(name)
}

/// ホーム画面のコンポーネントの形をしているか。
pub fn is_component(name: &str) -> bool {
    HOME_RE.is_match(name)
}

/// `package:<name>` の行からパッケージ名を集める（`pm list packages` の答え）。
/// `-f` 付きの `package:<apk のパス>=<name>` の形も読む。
pub fn package_names(answer: &str) -> std::collections::BTreeSet<String> {
    answer
        .lines()
        .filter_map(|line| line.trim().strip_prefix("package:"))
        .map(|rest| rest.rsplit('=').next().unwrap_or(rest).trim().to_string())
        .filter(|name| !name.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_shape() {
        assert!(is_package("com.example.app"));
        assert!(is_package("android.ext.services"));
        assert!(!is_package("android"));
        assert!(!is_package("com.example.app;reboot"));
        assert!(!is_package("com.example.app $(id)"));
        assert!(!is_package("1com.example"));
    }

    #[test]
    fn component_shape() {
        assert!(is_component("com.example.home/.MainActivity"));
        assert!(is_component(
            "android/com.android.internal.app.ResolverActivity"
        ));
        assert!(is_component("com.example/.Outer$Inner"));
        assert!(!is_component("No activity found"));
    }

    #[test]
    fn reads_pm_list() {
        let names = package_names("package:com.a.b\r\npackage:/system/app/x.apk=com.c.d\nnoise\n");
        assert_eq!(
            names.into_iter().collect::<Vec<_>>(),
            ["com.a.b", "com.c.d"]
        );
    }
}
