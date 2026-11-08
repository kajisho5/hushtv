//! 機種別リスト（cleanyourtv の `examples/<機種>/debloat*.txt` と `tv.json`）の読み込み。

use crate::names::{PACKAGE_RE, is_package};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

/// リストの 1 行。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub package: String,
    /// 行末の `# メモ`
    pub note: Option<String>,
}

/// 読み込んだリスト。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebloatList {
    /// 止めるパッケージ（リストの順、重複なし）
    pub disable: Vec<Entry>,
    /// コメントアウトされた＝リストの作者が「残す」と判断したパッケージ
    pub keep: Vec<Entry>,
    /// 冒頭の注意書き（`#` を外したもの）
    pub header: String,
}

/// リストを受け付けない理由。
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ListError {
    /// パッケージ名の形をしていない行がある。テレビのシェルへ渡せないので、リストごと受け付けない。
    #[error("パッケージ名ではない行があります: {0:?}")]
    NotAPackage(Vec<String>),
    /// 止める行が 1 つもない
    #[error("止めるパッケージが 1 つもありません（全部コメントか空行）")]
    Empty,
}

static KEEP_RE: LazyLock<Regex> = LazyLock::new(|| {
    let pkg = PACKAGE_RE
        .as_str()
        .trim_start_matches('^')
        .trim_end_matches('$');
    Regex::new(&format!(r"^\s*#\s*({pkg})\s*(?:#\s*(?P<note>.*))?$")).unwrap()
});

fn note_of(rest: &str) -> Option<String> {
    let note = rest.trim();
    (!note.is_empty()).then(|| note.to_string())
}

/// テキストを読む。元の disable.sh と同じく、BOM と CRLF を落とし、`#` の行と空行を飛ばし、
/// 行末の ` # メモ` を外した最初の語をパッケージ名とする。1 行でも形が違えばリストごと断る。
pub fn parse(text: &str) -> Result<DebloatList, ListError> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut disable: Vec<Entry> = Vec::new();
    let mut keep = Vec::new();
    let mut bad = Vec::new();
    let mut header = Vec::new();
    let mut in_header = true;
    for raw in text.lines() {
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        let trimmed = line.trim_start();
        if trimmed.is_empty() {
            in_header = false;
            continue;
        }
        if trimmed.starts_with('#') {
            if let Some(c) = KEEP_RE.captures(line) {
                keep.push(Entry {
                    package: c[1].to_string(),
                    note: c.name("note").and_then(|m| note_of(m.as_str())),
                });
            } else if in_header {
                header.push(
                    trimmed
                        .trim_start_matches('#')
                        .strip_prefix(' ')
                        .unwrap_or(trimmed.trim_start_matches('#'))
                        .to_string(),
                );
            }
            continue;
        }
        in_header = false;
        // 行末の空白＋# 以降を外して、最初の語
        let (body, note) = match find_comment(line) {
            Some(i) => (
                &line[..i],
                note_of(line[i..].trim_start().trim_start_matches('#')),
            ),
            None => (line, None),
        };
        let Some(first) = body.split_whitespace().next() else {
            continue;
        };
        if !is_package(first) {
            bad.push(first.to_string());
            continue;
        }
        if !disable.iter().any(|e| e.package == first) {
            disable.push(Entry {
                package: first.to_string(),
                note,
            });
        }
    }
    if !bad.is_empty() {
        return Err(ListError::NotAPackage(bad));
    }
    if disable.is_empty() {
        return Err(ListError::Empty);
    }
    Ok(DebloatList {
        disable,
        keep,
        header: header.join("\n"),
    })
}

/// 空白のあとの `#` の位置（disable.sh の `sed -E 's/[[:space:]]+#.*$//'` と同じ）。
fn find_comment(line: &str) -> Option<usize> {
    let bytes = line.as_bytes();
    (1..bytes.len())
        .find(|&i| bytes[i] == b'#' && bytes[i - 1].is_ascii_whitespace())
        .map(|i| {
            let mut j = i;
            while j > 0 && bytes[j - 1].is_ascii_whitespace() {
                j -= 1;
            }
            j
        })
}

/// `tv.json` の中身。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TvInfo {
    #[serde(default)]
    pub tv: Option<String>,
    #[serde(default)]
    pub os: Option<String>,
    #[serde(default)]
    pub brand: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\u{feff}# ACME 55U8（架空の機種）\r\n# 注意書き 2 行目\r\n\r\n# 広告\r\ncom.example.adservice   # 広告の配信\r\ncom.example.telemetry\r\n# com.example.voiceassist   # 残す\r\ncom.example.adservice\r\n";

    #[test]
    fn reads_entries_header_and_keeps() {
        let list = parse(SAMPLE).unwrap();
        let names: Vec<_> = list.disable.iter().map(|e| e.package.as_str()).collect();
        assert_eq!(names, ["com.example.adservice", "com.example.telemetry"]);
        assert_eq!(list.disable[0].note.as_deref(), Some("広告の配信"));
        assert_eq!(list.keep[0].package, "com.example.voiceassist");
        assert_eq!(list.keep[0].note.as_deref(), Some("残す"));
        assert_eq!(list.header, "ACME 55U8（架空の機種）\n注意書き 2 行目");
    }

    #[test]
    fn refuses_shell_syntax() {
        let err = parse("com.example.ok\ncom.example.bad;reboot\n").unwrap_err();
        assert_eq!(
            err,
            ListError::NotAPackage(vec!["com.example.bad;reboot".into()])
        );
    }

    #[test]
    fn refuses_all_comment_list() {
        assert_eq!(parse("# com.example.a\n\n").unwrap_err(), ListError::Empty);
    }

    #[test]
    fn every_bundled_list_parses() {
        let mut n = 0;
        for info in crate::lists::all() {
            for (file, text) in &info.files {
                parse(text).unwrap_or_else(|e| panic!("{}/{file}: {e}", info.id));
                n += 1;
            }
        }
        assert!(n > 400, "同梱リストが少なすぎます: {n}");
    }
}
