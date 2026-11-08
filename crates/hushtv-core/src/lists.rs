//! アプリに同梱した機種別リスト（data/lists/ を実行ファイルに埋め込む）。

use crate::list::TvInfo;
use include_dir::{Dir, include_dir};

static LISTS: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/../../data/lists");
const UPSTREAM: &str = include_str!("../../../data/UPSTREAM.json");

/// 1 機種ぶんのリスト。
#[derive(Debug, Clone)]
pub struct ListInfo {
    /// フォルダ名。例 `acme-55u8`
    pub id: String,
    /// (ファイル名, 中身)。`debloat.txt` が先頭
    pub files: Vec<(String, String)>,
    pub tv: Option<TvInfo>,
}

impl ListInfo {
    /// 表示用の名前（tv.json の tv、なければフォルダ名）。
    pub fn title(&self) -> String {
        self.tv
            .as_ref()
            .and_then(|t| t.tv.clone())
            .unwrap_or_else(|| self.id.clone())
    }
}

/// 同梱リストを全部、フォルダ名の順で。
pub fn all() -> Vec<ListInfo> {
    let mut out: Vec<ListInfo> = LISTS
        .dirs()
        .map(|dir| {
            let id = dir
                .path()
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_string();
            let mut files: Vec<(String, String)> = dir
                .files()
                .filter_map(|f| {
                    let name = f.path().file_name()?.to_string_lossy().to_string();
                    (name.starts_with("debloat") && name.ends_with(".txt"))
                        .then(|| (name, f.contents_utf8().unwrap_or_default().to_string()))
                })
                .collect();
            files.sort_by_key(|(n, _)| (n != "debloat.txt", n.clone()));
            let tv = dir
                .files()
                .find(|f| f.path().file_name().is_some_and(|n| n == "tv.json"))
                .and_then(|f| serde_json::from_slice(f.contents()).ok());
            ListInfo { id, files, tv }
        })
        .filter(|l| !l.files.is_empty())
        .collect();
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

/// フォルダ名で 1 つ取り出す。
pub fn get(id: &str) -> Option<ListInfo> {
    all().into_iter().find(|l| l.id == id)
}

/// 取り込んだ cleanyourtv のコミット。
pub fn upstream_commit() -> String {
    serde_json::from_str::<serde_json::Value>(UPSTREAM)
        .ok()
        .and_then(|v| v["commit"].as_str().map(str::to_string))
        .unwrap_or_default()
}
