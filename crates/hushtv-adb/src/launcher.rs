//! 広告のないホーム画面（cleanyourtv の launcher）を入れる・戻す。
//! 判断は hushtv_core::home、手順は cleanyourtv の launcher/build-and-install.sh と同じ。

use crate::Device;
use crate::flow::{data_dir, host_tag, now_stamp};
use hushtv_core::home::{self, HomeRecord, StockChoice};
use hushtv_core::safety::{CLAIM_QUERY, CUSTOM_LAUNCHER, HOME_QUERY};
use std::path::PathBuf;

/// 同梱のホーム画面アプリ（CI で作ったもの）。手元のビルドなど、入っていなければ None。
pub fn bundled() -> Option<&'static [u8]> {
    #[cfg(has_launcher)]
    {
        Some(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../data/launcher/tvhome.apk"
        )))
    }
    #[cfg(not(has_launcher))]
    {
        None
    }
}

#[derive(Debug, thiserror::Error)]
pub enum HomeError {
    #[error("このビルドにはホーム画面アプリが入っていません（Releases のアプリを使ってください）")]
    NotBundled,
    #[error("このテレビは Android の SDK {0} で、ホーム画面アプリ（SDK 26 以上）を入れられません")]
    TooOld(u32),
    #[error(
        "ホームが Android の選択画面で、純正ホームの候補が {0} 個あります。テレビでホームを 1 つ選んでから、もう一度試してください"
    )]
    Ambiguous(usize),
    #[error(
        "今のホームは「{0}」で、知っている純正ホームではありません。初期設定の画面なら、先に「片付ける」で setupwraith を止めてください"
    )]
    Unknown(String),
    #[error("テレビへの送信に失敗しました（{0}）")]
    Push(String),
    #[error("テレビがアプリを受け付けませんでした（{0}）。純正ホームはそのままです")]
    Install(String),
    #[error("ホーム画面アプリが起動しませんでした。純正ホームはそのままです")]
    Start,
    #[error("ホームが切り替わらなかったので、純正ホームに戻しました（今のホーム：{0}）")]
    RolledBack(String),
    #[error(
        "ホームが切り替わらず、純正ホームにも戻せませんでした（今のホーム：{0}）。「テレビを直す」を使ってください"
    )]
    Stuck(String),
    #[error("このテレビで hushtv がホームを切り替えた記録がありません")]
    NoRecord,
    #[error("記録を保存できません（{0}）")]
    Save(String),
}

fn record_path(host: &str) -> PathBuf {
    data_dir().join(format!("home-{}.json", host_tag(host)))
}

pub fn load_record(host: &str) -> Option<HomeRecord> {
    let text = std::fs::read_to_string(record_path(host)).ok()?;
    serde_json::from_str(&text).ok()
}

/// 入れたあとの様子。
#[derive(Debug, Clone, serde::Serialize)]
pub struct Installed {
    /// 止めた純正ホーム
    pub stock: Option<String>,
    /// もとから入っていた（入れ直さなかった）
    pub was_installed: bool,
    pub home: String,
}

/// ホームのいまの答え（最後の行）。
fn home_now(dev: &mut Device) -> String {
    dev.shell(HOME_QUERY)
        .unwrap_or_default()
        .lines()
        .last()
        .unwrap_or("")
        .trim()
        .to_string()
}

/// 広告のないホームに切り替える。`stock` で止める純正ホームを指定できる（`Some("none")` で何も止めない）。
pub fn install(dev: &mut Device, stock: Option<&str>) -> Result<Installed, HomeError> {
    let apk = bundled().ok_or(HomeError::NotBundled)?;
    let sdk: u32 = dev
        .shell("getprop ro.build.version.sdk")
        .unwrap_or_default()
        .trim()
        .parse()
        .unwrap_or(0);
    if sdk != 0 && sdk < home::MIN_SDK {
        return Err(HomeError::TooOld(sdk));
    }
    let previous = hushtv_core::safety::resolved_home(&dev.shell(HOME_QUERY).unwrap_or_default());

    // 止める純正ホームを決める
    let stock: Option<String> = match stock {
        Some("none") => None,
        Some(s) => Some(s.to_string()),
        None => {
            let h = dev.shell(HOME_QUERY).unwrap_or_default();
            let c = dev.shell(CLAIM_QUERY).unwrap_or_default();
            match home::choose_stock(&h, &c) {
                StockChoice::Stock(s) => Some(s),
                StockChoice::AlreadyOurs => None,
                StockChoice::Ambiguous(n) => return Err(HomeError::Ambiguous(n)),
                StockChoice::Unknown(u) => return Err(HomeError::Unknown(u)),
            }
        }
    };

    // もう入っていれば入れ直さない（自分で入れた cleanyourtv の版は署名が違い、上書きできないため）
    let was_installed = dev
        .shell(&format!("pm path {CUSTOM_LAUNCHER}"))
        .unwrap_or_default()
        .contains("package:");
    if !was_installed {
        let tmp = "/data/local/tmp/hushtv-home.apk";
        dev.push(apk, tmp).map_err(HomeError::Push)?;
        let out = dev
            .shell(&format!("pm install -r {tmp}; rm -f {tmp}"))
            .unwrap_or_default();
        if !out.contains("Success") {
            return Err(HomeError::Install(out.trim().to_string()));
        }
    }

    let start = if stock.is_none() {
        format!("am start -W -n {CUSTOM_LAUNCHER}/.MainActivity")
    } else {
        format!(
            "am start -W -a android.intent.action.MAIN -c android.intent.category.HOME -n {CUSTOM_LAUNCHER}/.MainActivity"
        )
    };
    let started = dev.shell(&start).unwrap_or_default();
    if !started.contains("Status: ok") || started.lines().any(|l| l.starts_with("Error")) {
        return Err(HomeError::Start);
    }
    dev.run_all(&home::switch_commands(stock.as_deref()));

    let now = home_now(dev);
    if !home::is_ours(&now) {
        if let Some(s) = &stock {
            dev.run_all(&home::back_commands(s));
            let back = home_now(dev);
            if home::is_stock(&back, s) {
                return Err(HomeError::RolledBack(now));
            }
            return Err(HomeError::Stuck(back));
        }
        return Err(HomeError::Stuck(now));
    }
    // 画面の設定を書き換える強い権限は、使わないので外す（スクリプトの既定と同じ）
    let _ = dev.shell(&format!(
        "pm revoke {CUSTOM_LAUNCHER} android.permission.WRITE_SECURE_SETTINGS"
    ));

    let rec = HomeRecord {
        created: now_stamp(),
        host: dev.host.clone(),
        stock: stock.clone(),
        previous,
        installed_by_hushtv: !was_installed,
    };
    let path = record_path(&dev.host);
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d).map_err(|e| HomeError::Save(e.to_string()))?;
    }
    // 2 回目以降の切り替えでは、最初の記録（本当の純正ホーム）を残す
    if !path.exists() {
        let json =
            serde_json::to_string_pretty(&rec).map_err(|e| HomeError::Save(e.to_string()))?;
        std::fs::write(&path, json).map_err(|e| HomeError::Save(e.to_string()))?;
    }
    Ok(Installed {
        stock,
        was_installed,
        home: now,
    })
}

/// 純正ホームに戻す。hushtv が入れたホーム画面アプリは消す（もとからあったものは残す）。
pub fn remove(dev: &mut Device) -> Result<String, HomeError> {
    let rec = load_record(&dev.host).ok_or(HomeError::NoRecord)?;
    let Some(stock) = rec.stock.clone() else {
        // 何も止めていない：ホームの指定だけ外す
        if rec.installed_by_hushtv {
            let _ = dev.shell(&format!("pm uninstall {CUSTOM_LAUNCHER}"));
        }
        let _ = std::fs::remove_file(record_path(&dev.host));
        return Ok(home_now(dev));
    };
    dev.run_all(&home::back_commands(&stock));
    let now = home_now(dev);
    if !home::is_stock(&now, &stock) {
        return Err(HomeError::Stuck(now));
    }
    if rec.installed_by_hushtv {
        let _ = dev.shell(&format!("pm uninstall {CUSTOM_LAUNCHER}"));
    }
    let _ = std::fs::remove_file(record_path(&dev.host));
    Ok(now)
}

impl HomeError {
    /// 英語の説明（画面・コマンドが英語のとき）。
    pub fn en(&self) -> String {
        match self {
            Self::NotBundled => {
                "This build has no home screen app inside. Use the app from Releases".into()
            }
            Self::TooOld(s) => {
                format!("This TV runs Android SDK {s}; the home screen app needs SDK 26 or newer")
            }
            Self::Ambiguous(n) => format!(
                "Home is Android's chooser with {n} stock launcher candidates. Pick one Home on the TV, then try again"
            ),
            Self::Unknown(u) => format!(
                "Home is currently \"{u}\", not a stock launcher hushtv knows. If it's the setup screen, switch off setupwraith in Clean up first"
            ),
            Self::Push(e) => format!("Couldn't send the app to the TV ({e})"),
            Self::Install(e) => {
                format!("The TV refused the app ({e}). The stock home is untouched")
            }
            Self::Start => "The home screen app didn't start. The stock home is untouched".into(),
            Self::RolledBack(h) => {
                format!("Home didn't switch, so the stock home was put back (Home is now {h})")
            }
            Self::Stuck(h) => format!(
                "Home didn't switch and the stock home couldn't be put back (Home is now {h}). Use Repair a TV"
            ),
            Self::NoRecord => "hushtv has no record of switching Home on this TV".into(),
            Self::Save(e) => format!("Couldn't save the record ({e})"),
        }
    }

    pub fn text(&self, ja: bool) -> String {
        if ja { self.to_string() } else { self.en() }
    }
}
