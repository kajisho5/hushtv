//! 片付けの一連の流れ（コマンド版と画面版で共通）。
//!
//! 1. 選んだアプリをもう一度安全装置に通す
//! 2. 変える前に戻す記録を保存（最初の記録は上書きしない）
//! 3. 共有用に、変える前のホーム画面を撮る
//! 4. 止める（安全モードなら少しずつ確かめながら）→ テレビに問い合わせて読み戻す
//! 5. 点検用の控えを保存
//! 6. 変えた後のホーム画面を撮って、共有用の画像を作る

use crate::{Device, Error};
use hushtv_core::category::{self, Category};
use hushtv_core::health::{self, SafeRun};
use hushtv_core::list::DebloatList;
use hushtv_core::lists::ListInfo;
use hushtv_core::plan::Applied;
use hushtv_core::record::{self, RecoveryRecord};
use hushtv_core::safety::{self, Lang, Overrides, Reason};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// hushtv が使うフォルダ（OS のアプリ用データの場所）。
pub fn data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_default())
        .join("hushtv")
}

/// 戻す記録のフォルダ（cleanyourtv と同じ `~/.cleanyourtv/`）。
pub fn records_dir() -> PathBuf {
    record::record_dir(&dirs::home_dir().unwrap_or_default())
}

/// ファイル名に使える形のテレビの名前。
pub fn host_tag(host: &str) -> String {
    record::lan_address(host)
        .unwrap_or_else(|| host.to_string())
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// 点検用の控えの場所。
pub fn applied_path(host: &str) -> PathBuf {
    data_dir().join(format!("applied-{}.json", host_tag(host)))
}

/// 点検用の控えを読む。
pub fn load_applied(host: &str) -> Option<Applied> {
    serde_json::from_str(&std::fs::read_to_string(applied_path(host)).ok()?).ok()
}

/// UTC の今の時刻（元の記録と同じ `%Y-%m-%dT%H:%M:%S%z` の形）。
pub fn now_stamp() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let (days, rem) = (secs.div_euclid(86400), secs.rem_euclid(86400));
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}+0000",
        rem / 3600,
        rem / 60 % 60,
        rem % 60
    )
}

/// 同梱リストを決まった場所に書き出す（戻す記録の名前はリストのファイルの場所で決まるため）。
pub fn materialize_list(info: &ListInfo) -> std::io::Result<PathBuf> {
    let (file, text) = &info.files[0];
    let path = data_dir().join("lists").join(&info.id).join(file);
    std::fs::create_dir_all(path.parent().unwrap())?;
    if std::fs::read_to_string(&path).ok().as_deref() != Some(text.as_str()) {
        std::fs::write(&path, text)?;
    }
    Ok(path)
}

/// 進み具合の知らせ。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Progress {
    Checking,
    Saving,
    ShotBefore,
    Batch {
        done: usize,
        total: usize,
    },
    Package {
        done: usize,
        total: usize,
        name: String,
    },
    ReadBack,
    ShotAfter,
}

#[derive(Debug, Clone, Copy)]
pub struct ApplyOptions {
    pub safe: bool,
    pub batch: usize,
    pub share: bool,
    pub ov: Overrides,
}

impl Default for ApplyOptions {
    fn default() -> Self {
        Self {
            safe: true,
            batch: 4,
            share: true,
            ov: Overrides::default(),
        }
    }
}

/// 片付けの結果。
#[derive(Debug, Clone, serde::Serialize)]
pub struct Outcome {
    /// 止めて、テレビでも止まっていたもの
    pub disabled: Vec<String>,
    /// 送ったのにまだ動いているもの
    pub still_on: Vec<String>,
    /// 実行直前の安全装置で残したもの
    pub protected: Vec<(String, Reason)>,
    /// 安全モードの結果
    pub safe: Option<SafeRun>,
    pub record_path: PathBuf,
    /// 種類ごとの止めた数
    pub by_category: Vec<(Category, usize)>,
    pub card_path: Option<PathBuf>,
    /// 安全モードで止まった組の数（問題が出たら 1）
    pub problems: usize,
    pub batches: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum FlowError {
    #[error(transparent)]
    Tv(#[from] Error),
    #[error("ファイルを保存できません: {0}")]
    Io(#[from] std::io::Error),
    #[error(
        "この TV とリストの戻す記録が 2 つあります。正しくない方を別の場所へ移してから、もう一度実行してください（テレビは変えていません）: {0:?}"
    )]
    TwoRecords(Vec<PathBuf>),
    #[error("戻す記録が壊れています: {0}")]
    BadRecord(String),
}

/// 片付ける。`selected` は画面やコマンドで選んだアプリ（このあともう一度安全装置に通す）。
#[allow(clippy::too_many_arguments)]
pub fn apply(
    dev: &mut Device,
    list_id: &str,
    list_path: &Path,
    list: &DebloatList,
    selected: &[String],
    opt: ApplyOptions,
    mut progress: impl FnMut(Progress),
) -> Result<Outcome, FlowError> {
    let host = dev.host.clone();
    progress(Progress::Checking);
    let state = dev.state()?;
    let guarded = safety::run(selected, dev, opt.ov);
    let enabled: BTreeSet<&String> = state
        .installed
        .iter()
        .filter(|p| !state.disabled.contains(*p))
        .collect();
    let to_disable: Vec<String> = guarded
        .send
        .iter()
        .filter(|p| enabled.contains(p))
        .cloned()
        .collect();

    // 戻す記録（リストの全部と、選んだものを記録する）
    progress(Progress::Saving);
    let dir = records_dir();
    std::fs::create_dir_all(&dir)?;
    let rec_path = record::find_record(&dir, list_path, &host).map_err(FlowError::TwoRecords)?;
    let mut names: Vec<String> = list.disable.iter().map(|e| e.package.clone()).collect();
    for s in selected {
        if !names.contains(s) {
            names.push(s.clone());
        }
    }
    let mut rec: RecoveryRecord = if rec_path.exists() {
        record::load(&std::fs::read_to_string(&rec_path)?).map_err(FlowError::BadRecord)?
    } else {
        record::new_record(&now_stamp(), &host, list_path, &names, &state)
    };
    if !rec_path.exists() || record::extend_record(&mut rec, &names, &state) {
        std::fs::write(&rec_path, record::to_json(&rec))?;
    }

    // 変える前の写真
    let before = if opt.share && !to_disable.is_empty() {
        progress(Progress::ShotBefore);
        dev.home_screenshot(Duration::from_millis(2500))
    } else {
        None
    };

    // 止める
    let total_batches = to_disable.len().div_ceil(opt.batch.max(1));
    let safe = if opt.safe {
        Some(health::run(dev, &to_disable, opt.batch, |done, total| {
            progress(Progress::Batch { done, total })
        }))
    } else {
        let total = to_disable.len();
        dev.disable(&to_disable, |i, n| {
            progress(Progress::Package {
                done: i + 1,
                total,
                name: n.to_string(),
            })
        });
        None
    };

    // 読み戻し
    progress(Progress::ReadBack);
    let (installed, disabled) = dev.packages()?;
    let on_now: BTreeSet<&String> = installed
        .iter()
        .filter(|p| !disabled.contains(*p))
        .collect();
    let done: Vec<String> = to_disable
        .iter()
        .filter(|p| !on_now.contains(p))
        .cloned()
        .collect();
    let stopped = safe.as_ref().is_some_and(|s| s.stopped.is_some());
    let still_on: Vec<String> = if stopped {
        vec![]
    } else {
        to_disable
            .iter()
            .filter(|p| on_now.contains(p))
            .cloned()
            .collect()
    };

    // 点検用の控え（前回の控えがあれば、止めたものを足す）
    let mut all_disabled = load_applied(&host)
        .filter(|a| a.host == host)
        .map(|a| a.disabled)
        .unwrap_or_default();
    for d in &done {
        if !all_disabled.contains(d) {
            all_disabled.push(d.clone());
        }
    }
    let applied = Applied {
        host: host.clone(),
        list_id: list_id.to_string(),
        at: now_stamp(),
        disabled: all_disabled,
        installed: state.installed.iter().cloned().collect(),
        home: state.home.clone(),
    };
    std::fs::create_dir_all(data_dir())?;
    std::fs::write(
        applied_path(&host),
        serde_json::to_string_pretty(&applied).unwrap_or_default(),
    )?;

    // 種類ごとの数
    let mut counts: BTreeMap<Category, usize> = BTreeMap::new();
    for p in &done {
        let c = list
            .disable
            .iter()
            .find(|e| &e.package == p)
            .map(category::classify)
            .unwrap_or(Category::Other);
        *counts.entry(c).or_default() += 1;
    }

    // 共有用の画像
    let mut card_path = None;
    if opt.share && !done.is_empty() {
        progress(Progress::ShotAfter);
        let after = dev.home_screenshot(Duration::from_millis(3000));
        let stats = hushtv_card::Stats {
            switched_off: done.len(),
            by_category: counts
                .iter()
                .map(|(c, n)| (c.label(Lang::En).to_string(), *n))
                .collect(),
        };
        if let Ok(png) = hushtv_card::render(before.as_deref(), after.as_deref(), &stats) {
            let out = data_dir().join("share").join(format!(
                "hushtv-{}-{}.png",
                host_tag(&host),
                now_stamp().replace([':', '+'], "")
            ));
            std::fs::create_dir_all(out.parent().unwrap())?;
            std::fs::write(&out, png)?;
            card_path = Some(out);
        }
    }

    Ok(Outcome {
        disabled: done,
        still_on,
        protected: guarded.skipped,
        problems: usize::from(stopped),
        batches: if opt.safe { total_batches } else { 0 },
        safe,
        record_path: rec_path,
        by_category: counts.into_iter().collect(),
        card_path,
    })
}

/// 記録どおりに戻す。戻したコマンドの数を返す。
pub fn restore(
    dev: &mut Device,
    list_path: &Path,
    stock: Option<&str>,
) -> Result<usize, FlowError> {
    let rec_path = record::find_record(&records_dir(), list_path, &dev.host.clone())
        .map_err(FlowError::TwoRecords)?;
    let text = std::fs::read_to_string(&rec_path)?;
    let rec = record::load(&text).map_err(FlowError::BadRecord)?;
    let cmds = record::restore_commands(&rec, stock);
    dev.run_all(&cmds);
    let _ = std::fs::remove_file(applied_path(&dev.host));
    Ok(cmds.len())
}

/// テレビ側でデバッグを切る（このあと通信は切れる）。切れたかは [`crate::port_open`] で確かめる。
pub fn turn_off_debugging(dev: &mut Device) {
    let _ = dev.shell("settings put global adb_wifi_enabled 0; settings put global adb_enabled 0");
}

/// 修復のために、テレビの状態を読む。
pub fn rescue_facts(dev: &mut Device) -> Result<hushtv_core::rescue::Facts, FlowError> {
    use hushtv_core::names::package_names;
    let (installed, disabled) = dev.packages()?;
    let mut all = package_names(&dev.shell("pm list packages -u").unwrap_or_default());
    all.extend(installed.iter().cloned());
    let home_answer = dev.shell(safety::HOME_QUERY).unwrap_or_default();
    let settings_answer = dev
        .shell(hushtv_core::health::SETTINGS_QUERY)
        .unwrap_or_default();
    let scales = record::SETTINGS
        .iter()
        .map(|k| {
            (
                k.to_string(),
                dev.shell(&format!("settings get global {k}"))
                    .unwrap_or_default()
                    .trim()
                    .to_string(),
            )
        })
        .collect();
    let mut settings = std::collections::BTreeMap::new();
    for (ns, key) in hushtv_core::rescue::SETTINGS_TO_READ
        .iter()
        .chain(std::iter::once(&hushtv_core::rescue::VPN_APP_KEY))
    {
        settings.insert(
            key.to_string(),
            dev.shell(&format!("settings get {ns} {key}"))
                .unwrap_or_default()
                .trim()
                .to_string(),
        );
    }
    Ok(hushtv_core::rescue::Facts {
        installed,
        disabled,
        all,
        home_answer,
        settings_answer,
        scales,
        settings,
    })
}

/// このテレビの戻す記録を探す（cleanyourtv・CleanMyTV・hushtv のどれが作ったものでも）。新しい順。
pub fn records_for_host(host: &str) -> Vec<(PathBuf, RecoveryRecord)> {
    let address = record::lan_address(host).unwrap_or_else(|| host.to_string());
    let tags: Vec<String> = [address.clone(), format!("{address}:5555"), host.to_string()]
        .iter()
        .map(|t| {
            t.chars()
                .map(|c| {
                    if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
                        c
                    } else {
                        '_'
                    }
                })
                .collect()
        })
        .collect();
    let mut out: Vec<(PathBuf, RecoveryRecord)> = std::fs::read_dir(records_dir())
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            let name = p
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            tags.iter().any(|t| {
                name.starts_with(&format!("recovery-{t}-"))
                    && name.ends_with(".json")
                    && name.len() == format!("recovery-{t}-").len() + 13
            })
        })
        .filter_map(|p| {
            Some((
                p.clone(),
                record::load(&std::fs::read_to_string(&p).ok()?).ok()?,
            ))
        })
        .collect();
    out.sort_by(|a, b| b.1.created.cmp(&a.1.created));
    out
}

/// 修復の結果。
#[derive(Debug, Clone, serde::Serialize)]
pub struct RescueOutcome {
    pub commands: Vec<String>,
    pub before: Vec<hushtv_core::rescue::Problem>,
    pub after: Vec<hushtv_core::rescue::Problem>,
}

/// 修復コマンドを送り、もう一度診断する。
pub fn rescue(
    dev: &mut Device,
    level: hushtv_core::rescue::Level,
) -> Result<RescueOutcome, FlowError> {
    use hushtv_core::rescue::{diagnose, fix_commands};
    let facts = rescue_facts(dev)?;
    let before = diagnose(&facts);
    let commands = fix_commands(&before, &facts, level);
    dev.run_all(&commands);
    std::thread::sleep(Duration::from_millis(1500));
    let after = diagnose(&rescue_facts(dev)?);
    Ok(RescueOutcome {
        commands,
        before,
        after,
    })
}

/// 記録ファイルを指定して戻す（ほかのツールの記録も同じ形式なので使える）。
pub fn restore_from(dev: &mut Device, rec: &RecoveryRecord, stock: Option<&str>) -> usize {
    let cmds = record::restore_commands(rec, stock);
    dev.run_all(&cmds);
    cmds.len()
}
