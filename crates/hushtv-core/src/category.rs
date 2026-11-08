//! アプリの種類分けと、止め方の選択（軽め／全部）。
//!
//! 種類はリストの行のメモと、その上の見出しの言葉から決める（hushtv 独自の判定）。
//! メモで決まればメモを優先し、決まらなければ見出しで決める。

use crate::list::{DebloatList, Entry};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

/// アプリの種類。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    /// 利用状況の送信・追跡
    Tracking,
    /// 広告・宣伝
    Ads,
    /// おすすめ欄・コンテンツの行
    PromoRows,
    /// 店頭デモ
    StoreDemo,
    /// そのほか（使わない純正アプリなど）
    Other,
}

impl Category {
    pub fn label(self, lang: crate::safety::Lang) -> &'static str {
        use crate::safety::Lang::*;
        match (self, lang) {
            (Category::Tracking, Ja) => "追跡",
            (Category::Tracking, En) => "Tracking",
            (Category::Ads, Ja) => "広告",
            (Category::Ads, En) => "Ads",
            (Category::PromoRows, Ja) => "おすすめ欄",
            (Category::PromoRows, En) => "Promo rows",
            (Category::StoreDemo, Ja) => "店頭デモ",
            (Category::StoreDemo, En) => "Store demo",
            (Category::Other, Ja) => "そのほか",
            (Category::Other, En) => "Other",
        }
    }
}

static RULES: LazyLock<Vec<(Category, Regex)>> = LazyLock::new(|| {
    [
        (Category::Tracking, r"(?i)telemetry|track|analytic|metric|usage|report|\blogs?\b|feedback|diagnos|crash|statistic|data collect|privacy|beacon|survey"),
        (Category::Ads, r"(?i)\bads?\b|advert|sponsor|promo|marketing|personali[sz]|commercial|\boffers?\b|coupon|affiliate"),
        (Category::StoreDemo, r"(?i)\bdemo|retail|shop mode|store mode|in-store|hotel mode|showroom"),
        (Category::PromoRows, r"(?i)recommend|content rows?|\brows?\b|patchwall|watch next|discover|featured|channels? row|suggest"),
    ]
    .into_iter()
    .map(|(c, r)| (c, Regex::new(r).unwrap()))
    .collect()
});

fn classify_text(text: &str) -> Option<Category> {
    RULES
        .iter()
        .find(|(_, re)| re.is_match(text))
        .map(|(c, _)| *c)
}

/// 1 行の種類。
pub fn classify(e: &Entry) -> Category {
    e.note
        .as_deref()
        .and_then(classify_text)
        .or_else(|| e.section.as_deref().and_then(classify_text))
        .unwrap_or(Category::Other)
}

/// 止め方の選択。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Preset {
    /// 軽め：広告と追跡だけ（はじめての人の既定）
    #[default]
    Light,
    /// 全部：リストの止める行をすべて
    Full,
}

impl Preset {
    pub fn includes(self, c: Category) -> bool {
        match self {
            Preset::Light => matches!(c, Category::Ads | Category::Tracking),
            Preset::Full => true,
        }
    }
}

/// 選んだ止め方で、止める行の名前（リストの順）。
pub fn select(list: &DebloatList, preset: Preset) -> Vec<String> {
    list.disable
        .iter()
        .filter(|e| preset.includes(classify(e)))
        .map(|e| e.package.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(note: Option<&str>, section: Option<&str>) -> Entry {
        Entry {
            package: "com.example.x".into(),
            note: note.map(Into::into),
            section: section.map(Into::into),
        }
    }

    #[test]
    fn note_wins_over_section() {
        assert_eq!(
            classify(&e(Some("store demo"), Some("Telemetry and analytics"))),
            Category::StoreDemo
        );
        assert_eq!(
            classify(&e(Some("Play Movies"), Some("Telemetry and analytics"))),
            Category::Tracking
        );
        assert_eq!(
            classify(&e(None, Some("TCL content, ads and promos"))),
            Category::Ads
        );
        assert_eq!(
            classify(&e(None, Some("Patchwall, content rows"))),
            Category::PromoRows
        );
        assert_eq!(classify(&e(None, Some("Preloaded apps"))), Category::Other);
        assert_eq!(classify(&e(None, None)), Category::Other);
    }

    #[test]
    fn light_is_a_subset_of_full() {
        let list =
            crate::list::parse("# Telemetry\ncom.a.t\n# Retail demo\ncom.a.d\n# Ads\ncom.a.a\n")
                .unwrap();
        assert_eq!(select(&list, Preset::Light), ["com.a.t", "com.a.a"]);
        assert_eq!(
            select(&list, Preset::Full),
            ["com.a.t", "com.a.d", "com.a.a"]
        );
    }

    #[test]
    fn bundled_lists_have_light_entries() {
        let mut counts = std::collections::BTreeMap::new();
        let mut lists_with_light = 0;
        let all = crate::lists::all();
        for info in &all {
            let list = crate::list::parse(&info.files[0].1).unwrap();
            for entry in &list.disable {
                *counts.entry(classify(entry)).or_insert(0) += 1;
            }
            if !select(&list, Preset::Light).is_empty() {
                lists_with_light += 1;
            }
        }
        eprintln!("{counts:?} / light {lists_with_light} of {}", all.len());
        assert!(
            lists_with_light * 10 >= all.len() * 8,
            "軽めで止めるものがないリストが多すぎます"
        );
    }
}
