//! hushtv のパソコン用アプリ。画面（ui/）から呼ばれるコマンドを置く。
//!
//! テレビとの通信は時間がかかるので、どのコマンドも別のスレッドで動かし、画面を止めない。
//! つながったテレビは 1 台だけ持つ。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use base64::Engine;
use hushtv_adb::{Device, default_key_path, flow};
use hushtv_core::category::{self, Preset};
use hushtv_core::list::{self, DebloatList};
use hushtv_core::lists;
use hushtv_core::plan::{self, Plan};
use hushtv_core::rescue::{self, Level};
use hushtv_core::safety::{self, Lang, Overrides};
use serde::Serialize;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, State};

#[derive(Default)]
struct Tv {
    dev: Option<Device>,
}

type Shared = Arc<Mutex<Tv>>;

fn lang() -> Lang {
    Lang::from_env()
}

/// 画面で選んだ言語（なければ OS の言語）。
fn pick(l: &Option<String>) -> Lang {
    match l.as_deref() {
        Some("ja") => Lang::Ja,
        Some("en") => Lang::En,
        _ => lang(),
    }
}

/// 別のスレッドで動かす。
async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| e.to_string())?
}

fn with_dev<T>(tv: &Shared, f: impl FnOnce(&mut Device) -> Result<T, String>) -> Result<T, String> {
    let mut guard = tv.lock().map_err(|_| "busy".to_string())?;
    let dev = guard.dev.as_mut().ok_or_else(|| {
        if lang() == Lang::Ja {
            "テレビにつながっていません".to_string()
        } else {
            "Not connected to a TV".to_string()
        }
    })?;
    f(dev)
}

#[derive(Serialize)]
struct Found {
    address: String,
    name: Option<String>,
    tls_port: Option<u16>,
    pair_port: Option<u16>,
    legacy_open: bool,
}

#[tauri::command]
fn ui_lang() -> &'static str {
    if lang() == Lang::Ja { "ja" } else { "en" }
}

#[tauri::command]
async fn scan(seconds: u64) -> Result<Vec<Found>, String> {
    blocking(move || {
        Ok(
            hushtv_adb::discover::scan(Duration::from_secs(seconds.clamp(1, 15)))
                .into_iter()
                .map(|t| Found {
                    address: t.address.to_string(),
                    name: t.name,
                    tls_port: t.tls_port,
                    pair_port: t.pair_port,
                    legacy_open: t.legacy_open,
                })
                .collect(),
        )
    })
    .await
}

#[derive(Serialize)]
struct Ranked {
    id: String,
    title: String,
    score: f64,
}

#[derive(Serialize)]
struct Connected {
    host: String,
    model: Option<String>,
    android: Option<String>,
    lists: Vec<Ranked>,
    has_applied: bool,
}

/// つないで、機種と近いリストを読む（connect と pair_connect で共通）。
fn open_tv(tv: &Shared, host: String) -> Result<Connected, String> {
    let mut dev = Device::connect(&host, &default_key_path()).map_err(|e| e.to_string())?;
    let model = dev.model();
    let android = dev.android_version();
    let (installed, _) = dev.packages().map_err(|e| e.to_string())?;
    let all = lists::all();
    let lists = plan::rank_lists(&installed, model.as_deref(), &all)
        .into_iter()
        .take(8)
        .map(|(score, _, l)| Ranked {
            id: l.id.clone(),
            title: l.title(),
            score: score.min(1.0),
        })
        .collect();
    let has_applied = flow::load_applied(&host).is_some();
    tv.lock().map_err(|_| "busy")?.dev = Some(dev);
    Ok(Connected {
        host,
        model,
        android,
        lists,
        has_applied,
    })
}

#[tauri::command]
async fn connect(tv: State<'_, Shared>, host: String) -> Result<Connected, String> {
    let tv = tv.inner().clone();
    blocking(move || open_tv(&tv, host)).await
}

/// 6 桁のコードでペアリングしてから、接続用のポートにつなぐ。
#[tauri::command]
async fn pair_connect(
    tv: State<'_, Shared>,
    address: String,
    pair_port: u16,
    code: String,
    tls_port: Option<u16>,
    lang: Option<String>,
) -> Result<Connected, String> {
    use hushtv_adb::pair::PairError as P;
    let ja = pick(&lang) == Lang::Ja;
    let say = move |j: &str, e: &str| if ja { j.to_string() } else { e.to_string() };
    let tv = tv.inner().clone();
    blocking(move || {
        let ip: std::net::IpAddr = address
            .trim()
            .parse()
            .map_err(|_| say("IP アドレスが読めません", "Can't read the IP address"))?;
        hushtv_adb::pair::pair((ip, pair_port).into(), &code, &default_key_path()).map_err(|e| match e {
            P::BadCode => say("コードは 6 桁の数字です", "The code is six digits"),
            P::WrongCode => say(
                "コードが違います。テレビに出ている 6 桁を入れ直してください",
                "Wrong code. Type the six digits shown on the TV again",
            ),
            P::Connect(d) => format!(
                "{}（{d}）",
                say(
                    "ペアリング用のポートにつながりません。テレビのペアリングの画面を開いたままにしてください",
                    "Can't reach the pairing port. Keep the pairing screen open on the TV"
                )
            ),
            other => other.to_string(),
        })?;
        // 接続用のポートはペアリング用とは別。分からなければテレビが知らせている番号を探す
        let port = tls_port.or_else(|| {
            hushtv_adb::discover::scan(Duration::from_secs(4))
                .into_iter()
                .find(|t| t.address == ip)
                .and_then(|t| t.tls_port)
        });
        let Some(port) = port else {
            return Err(say(
                "ペアリングできました。テレビの［ワイヤレスデバッグ］画面の「IP アドレスとポート」を、下の「見つからないとき」に入れてつないでください",
                "Paired. Now type the \"IP address & port\" from the TV's Wireless debugging screen into \"Not listed?\" and connect",
            ));
        };
        let host = if ip.is_ipv6() { format!("[{ip}]:{port}") } else { format!("{ip}:{port}") };
        open_tv(&tv, host)
    })
    .await
}

#[derive(Serialize)]
struct Row {
    package: String,
    category: &'static str,
    category_label: &'static str,
    /// on / off / absent
    state: &'static str,
    note: Option<String>,
    lists: usize,
    /// 安全装置で残す理由
    protected: Option<&'static str>,
    /// 「軽め」なら最初から選ぶか
    light: bool,
    /// リストの作者が残すと判断した行
    keep: bool,
}

#[derive(Serialize)]
struct PlanView {
    list_id: String,
    list_title: String,
    header: String,
    rows: Vec<Row>,
    notes: Vec<&'static str>,
}

fn cat_key(c: category::Category) -> &'static str {
    match c {
        category::Category::Tracking => "tracking",
        category::Category::Ads => "ads",
        category::Category::PromoRows => "promo",
        category::Category::StoreDemo => "demo",
        category::Category::Other => "other",
    }
}

fn load_list(id: &str) -> Result<(lists::ListInfo, DebloatList), String> {
    let info = lists::get(id).ok_or_else(|| format!("no list {id}"))?;
    let parsed = list::parse(&info.files[0].1).map_err(|e| e.to_string())?;
    Ok((info, parsed))
}

/// 計画を作る（テレビは変えない）。全部の行に安全装置を通して、状態と理由を付ける。
#[tauri::command]
async fn make_plan(
    tv: State<'_, Shared>,
    list_id: String,
    lang: Option<String>,
) -> Result<PlanView, String> {
    let ui = pick(&lang);
    let tv = tv.inner().clone();
    blocking(move || {
        let (info, l) = load_list(&list_id)?;
        with_dev(&tv, |dev| {
            let state = dev.state().map_err(|e| e.to_string())?;
            let names = plan::names(&l);
            let guarded = safety::run(&names, dev, Overrides::default());
            let p = Plan::build(&names, &guarded, &state);
            let counts = plan::list_counts();
            let lg = ui;
            let mut rows: Vec<Row> = l
                .disable
                .iter()
                .map(|e| {
                    let c = category::classify(e);
                    let st = if !state.installed.contains(&e.package) {
                        "absent"
                    } else if state.disabled.contains(&e.package) {
                        "off"
                    } else {
                        "on"
                    };
                    Row {
                        package: e.package.clone(),
                        category: cat_key(c),
                        category_label: c.label(lg),
                        state: st,
                        note: e.note.clone(),
                        lists: counts.get(&e.package).copied().unwrap_or(0),
                        protected: p
                            .protected
                            .iter()
                            .find(|(n, _)| n == &e.package)
                            .map(|(_, r)| r.text(lg)),
                        light: Preset::Light.includes(c),
                        keep: false,
                    }
                })
                .collect();
            rows.extend(
                l.keep
                    .iter()
                    .filter(|k| !l.disable.iter().any(|e| e.package == k.package))
                    .map(|e| Row {
                        package: e.package.clone(),
                        category: "other",
                        category_label: category::Category::Other.label(lg),
                        state: if !state.installed.contains(&e.package) {
                            "absent"
                        } else if state.disabled.contains(&e.package) {
                            "off"
                        } else {
                            "on"
                        },
                        note: e.note.clone(),
                        lists: counts.get(&e.package).copied().unwrap_or(0),
                        protected: None,
                        light: false,
                        keep: true,
                    }),
            );
            Ok(PlanView {
                list_id: info.id.clone(),
                list_title: info.title(),
                header: l.header.clone(),
                rows,
                notes: p.notes.iter().map(|n| n.text(lg)).collect(),
            })
        })
    })
    .await
}

#[derive(Serialize)]
struct ApplyView {
    disabled: usize,
    still_on: Vec<String>,
    protected: usize,
    problems: usize,
    batches: usize,
    stopped: Option<String>,
    rolled_back: Vec<String>,
    by_category: Vec<(String, usize)>,
    card_path: Option<String>,
    card_png: Option<String>,
    record_path: String,
}

#[tauri::command]
async fn apply(
    app: AppHandle,
    tv: State<'_, Shared>,
    list_id: String,
    selected: Vec<String>,
    safe: bool,
    batch: usize,
    lang: Option<String>,
) -> Result<ApplyView, String> {
    let ui = pick(&lang);
    let tv = tv.inner().clone();
    blocking(move || {
        let (info, l) = load_list(&list_id)?;
        let path = flow::materialize_list(&info).map_err(|e| e.to_string())?;
        with_dev(&tv, |dev| {
            let opt = flow::ApplyOptions {
                safe,
                batch: batch.clamp(1, 20),
                share: true,
                ov: Overrides::default(),
            };
            let out = flow::apply(dev, &info.id, &path, &l, &selected, opt, |p| {
                let _ = app.emit("progress", &p);
            })
            .map_err(|e| e.to_string())?;
            let card_png = out
                .card_path
                .as_ref()
                .and_then(|p| std::fs::read(p).ok())
                .map(|b| base64::engine::general_purpose::STANDARD.encode(b));
            Ok(ApplyView {
                disabled: out.disabled.len(),
                still_on: out.still_on.clone(),
                protected: out.protected.len(),
                problems: out.problems,
                batches: out.batches,
                stopped: out
                    .safe
                    .as_ref()
                    .and_then(|s| s.stopped.as_ref())
                    .map(|h| format!("{h:?}")),
                rolled_back: out
                    .safe
                    .as_ref()
                    .map(|s| s.rolled_back.clone())
                    .unwrap_or_default(),
                by_category: out
                    .by_category
                    .iter()
                    .map(|(c, n)| (c.label(ui).to_string(), *n))
                    .collect(),
                card_path: out.card_path.as_ref().map(|p| p.display().to_string()),
                card_png,
                record_path: out.record_path.display().to_string(),
            })
        })
    })
    .await
}

#[derive(Serialize)]
struct DriftView {
    has_applied: bool,
    at: Option<String>,
    returned: Vec<String>,
    new_apps: Vec<String>,
    home_before: Option<String>,
    home_now: Option<String>,
    home_changed: bool,
}

#[tauri::command]
async fn check(tv: State<'_, Shared>) -> Result<DriftView, String> {
    let tv = tv.inner().clone();
    blocking(move || {
        with_dev(&tv, |dev| {
            let Some(applied) = flow::load_applied(&dev.host) else {
                return Ok(DriftView {
                    has_applied: false,
                    at: None,
                    returned: vec![],
                    new_apps: vec![],
                    home_before: None,
                    home_now: None,
                    home_changed: false,
                });
            };
            let now = dev.state().map_err(|e| e.to_string())?;
            let d = plan::drift(&applied, &now);
            let (hb, hn) = d
                .home_changed
                .clone()
                .unwrap_or((applied.home.clone(), now.home.clone()));
            Ok(DriftView {
                has_applied: true,
                at: Some(applied.at),
                returned: d.returned,
                new_apps: d.new_apps,
                home_before: hb,
                home_now: hn,
                home_changed: d.home_changed.is_some(),
            })
        })
    })
    .await
}

/// 点検で見つかったずれを直す：戻ってきたアプリ（と選んだ新しいアプリ）を安全装置を通して止め直し、ホームを戻す。
#[tauri::command]
async fn fix_drift(tv: State<'_, Shared>, extra: Vec<String>) -> Result<usize, String> {
    let tv = tv.inner().clone();
    blocking(move || {
        with_dev(&tv, |dev| {
            let applied = flow::load_applied(&dev.host).ok_or("no snapshot")?;
            let now = dev.state().map_err(|e| e.to_string())?;
            let d = plan::drift(&applied, &now);
            let mut names = d.returned.clone();
            names.extend(extra.into_iter().filter(|n| d.new_apps.contains(n)));
            let g = safety::run(&names, dev, Overrides::default());
            let still = dev.disable(&g.send, |_, _| {});
            if let (true, Some(home)) = (d.home_changed.is_some(), applied.home.as_ref())
                && hushtv_core::names::is_component(home)
            {
                let _ = dev.shell(&format!("cmd package set-home-activity --user 0 {home}"));
            }
            Ok(g.send.len() - still.len())
        })
    })
    .await
}

#[derive(Serialize)]
struct ProblemView {
    kind: String,
    title: String,
    severity: u8,
}

#[derive(Serialize)]
struct RecordView {
    path: String,
    created: String,
}

#[derive(Serialize)]
struct RescueView {
    problems: Vec<ProblemView>,
    records: Vec<RecordView>,
}

fn problem_views(p: &[rescue::Problem], lg: Lang) -> Vec<ProblemView> {
    p.iter()
        .map(|x| ProblemView {
            kind: serde_json::to_value(x)
                .ok()
                .and_then(|v| v["kind"].as_str().map(str::to_string))
                .unwrap_or_default(),
            title: x.title(lg),
            severity: x.severity(),
        })
        .collect()
}

#[tauri::command]
async fn rescue_diagnose(
    tv: State<'_, Shared>,
    lang: Option<String>,
) -> Result<RescueView, String> {
    let ui = pick(&lang);
    let tv = tv.inner().clone();
    blocking(move || {
        with_dev(&tv, |dev| {
            let facts = flow::rescue_facts(dev).map_err(|e| e.to_string())?;
            let records = flow::records_for_host(&dev.host)
                .into_iter()
                .map(|(p, r)| RecordView {
                    path: p.display().to_string(),
                    created: r.created,
                })
                .collect();
            Ok(RescueView {
                problems: problem_views(&rescue::diagnose(&facts), ui),
                records,
            })
        })
    })
    .await
}

/// 直す。`how` は quick / all / record。
#[tauri::command]
async fn rescue_run(
    tv: State<'_, Shared>,
    how: String,
    lang: Option<String>,
) -> Result<Vec<ProblemView>, String> {
    let ui = pick(&lang);
    let tv = tv.inner().clone();
    blocking(move || {
        with_dev(&tv, |dev| {
            match how.as_str() {
                "record" => {
                    let (_, rec) = flow::records_for_host(&dev.host)
                        .into_iter()
                        .next()
                        .ok_or("no record")?;
                    flow::restore_from(dev, &rec, None);
                }
                "all" => {
                    flow::rescue(dev, Level::All).map_err(|e| e.to_string())?;
                }
                _ => {
                    flow::rescue(dev, Level::Quick).map_err(|e| e.to_string())?;
                }
            }
            let facts = flow::rescue_facts(dev).map_err(|e| e.to_string())?;
            Ok(problem_views(&rescue::diagnose(&facts), ui))
        })
    })
    .await
}

/// 片付けたときのリストで、記録どおりに戻す。
#[tauri::command]
async fn restore(tv: State<'_, Shared>) -> Result<usize, String> {
    let tv = tv.inner().clone();
    blocking(move || {
        with_dev(&tv, |dev| {
            let applied = flow::load_applied(&dev.host);
            match applied.and_then(|a| lists::get(&a.list_id)) {
                Some(info) => {
                    let path = flow::materialize_list(&info).map_err(|e| e.to_string())?;
                    flow::restore(dev, &path, None).map_err(|e| e.to_string())
                }
                None => {
                    let (_, rec) = flow::records_for_host(&dev.host)
                        .into_iter()
                        .next()
                        .ok_or("no record")?;
                    Ok(flow::restore_from(dev, &rec, None))
                }
            }
        })
    })
    .await
}

/// テレビのデバッグを切り、ポートが閉じたかを確かめる。
#[tauri::command]
async fn close_debugging(tv: State<'_, Shared>) -> Result<bool, String> {
    let tv = tv.inner().clone();
    blocking(move || {
        let host = with_dev(&tv, |dev| {
            flow::turn_off_debugging(dev);
            Ok(dev.host.clone())
        })?;
        tv.lock().map_err(|_| "busy")?.dev = None;
        std::thread::sleep(Duration::from_secs(2));
        let addr = hushtv_adb::parse_endpoint(&host).map_err(|e| e.to_string())?;
        Ok(!hushtv_adb::port_open(addr, Duration::from_millis(800)))
    })
    .await
}

/// 共有画像を「ピクチャ」フォルダへ写して、その場所を返す。
#[tauri::command]
fn save_card(path: String) -> Result<String, String> {
    let src = std::path::PathBuf::from(&path);
    let dir = dirs::picture_dir()
        .or_else(dirs::download_dir)
        .ok_or("no folder")?;
    let dest = dir.join(src.file_name().ok_or("bad path")?);
    std::fs::copy(&src, &dest).map_err(|e| e.to_string())?;
    Ok(dest.display().to_string())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage::<Shared>(Arc::new(Mutex::new(Tv::default())))
        .invoke_handler(tauri::generate_handler![
            ui_lang,
            scan,
            connect,
            pair_connect,
            make_plan,
            apply,
            check,
            fix_drift,
            rescue_diagnose,
            rescue_run,
            restore,
            close_debugging,
            save_card
        ])
        .run(tauri::generate_context!())
        .expect("hushtv を起動できませんでした");
}
