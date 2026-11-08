//! hushtv のコマンド操作。
//!
//!     hushtv scan                       同じ Wi-Fi のテレビを探す
//!     hushtv lists [言葉]               同梱の機種別リストを探す
//!     hushtv info  <テレビ>             機種と、近いリストを表示
//!     hushtv plan  <テレビ> [--list ID] 何を止めるかを見る（テレビは変えない）
//!     hushtv apply <テレビ> [--list ID] 戻す記録を保存してから止める
//!     hushtv check <テレビ> [--fix]     ファーム更新のあとのずれを調べる（直す）
//!     hushtv restore <テレビ> [--list ID] 記録どおりに戻す

use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand};
use hushtv_adb::flow::{self, applied_path, data_dir, records_dir};
use hushtv_adb::{Device, default_key_path};
use hushtv_core::category::{self, Preset};
use hushtv_core::list::{self, DebloatList};
use hushtv_core::lists::{self, ListInfo};
use hushtv_core::plan::{self, Applied, Plan};
use hushtv_core::record::{self};
use hushtv_core::safety::{self, Lang, Overrides};
use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Parser)]
#[command(
    name = "hushtv",
    version,
    about = "Android TV / Google TV の不要アプリを Wi-Fi 経由で止める（削除はしない）"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// 同じ Wi-Fi のテレビを探す
    Scan {
        /// mDNS を聞く秒数
        #[arg(long, default_value_t = 4)]
        seconds: u64,
    },
    /// 同梱の機種別リストを探す
    Lists { word: Option<String> },
    /// 機種と、近いリストを表示
    Info { host: String },
    /// 何を止めるかを見る（テレビは変えない）
    Plan(Target),
    /// 戻す記録を保存してから止める
    Apply {
        #[command(flatten)]
        target: Target,
        /// 確認を飛ばす
        #[arg(long, short)]
        yes: bool,
    },
    /// ファーム更新のあとのずれを調べる
    Check {
        host: String,
        /// 戻ってきたアプリを止め直し、ホームを戻す
        #[arg(long)]
        fix: bool,
        #[arg(long, short)]
        yes: bool,
        #[command(flatten)]
        ov: OverrideArgs,
    },
    /// 調子が悪くなったテレビを診断して直す（cleanyourtv・CleanMyTV などで止めすぎたとき）
    Rescue {
        host: String,
        /// quick＝起動・ホーム・設定・入力・Wi-Fi にかかわるものだけ戻す（既定）／ all＝機種別リストのアプリを全部戻す
        #[arg(long, value_enum, default_value_t = LevelArg::Quick)]
        level: LevelArg,
        /// 見つかった戻す記録（cleanyourtv・CleanMyTV・hushtv）どおりに戻す
        #[arg(long, conflicts_with = "level")]
        record: bool,
        /// 診断だけして、何も変えない
        #[arg(long)]
        dry_run: bool,
        #[arg(long, short)]
        yes: bool,
    },
    /// 記録どおりに戻す
    Restore {
        #[command(flatten)]
        target: Target,
        #[arg(long, short)]
        yes: bool,
        /// 純正ホームのパッケージ名（記録にホームがないときに戻す先）
        #[arg(long)]
        stock: Option<String>,
    },
}

#[derive(Args, Clone)]
struct Target {
    /// テレビの住所（例 192.168.10.23 / 192.168.10.23:5555）
    host: String,
    /// 同梱リストのフォルダ名（省略すると一番近いもの）
    #[arg(long)]
    list: Option<String>,
    /// 自分で用意したリストのファイル
    #[arg(long, conflicts_with = "list")]
    file: Option<PathBuf>,
    /// 止め方：light＝広告と追跡だけ（既定）／ full＝リストの全部
    #[arg(long, value_enum, default_value_t = PresetArg::Light)]
    preset: PresetArg,
    /// 安全モードを使わず、一度に全部止める
    #[arg(long)]
    fast: bool,
    /// 安全モードで一度に止める数
    #[arg(long, default_value_t = 4)]
    batch: usize,
    /// 共有用のビフォー・アフター画像を作らない
    #[arg(long)]
    no_share: bool,
    #[command(flatten)]
    ov: OverrideArgs,
}

#[derive(clap::ValueEnum, Clone, Copy, Debug)]
enum LevelArg {
    Quick,
    All,
}

#[derive(clap::ValueEnum, Clone, Copy, Debug)]
enum PresetArg {
    Light,
    Full,
}

impl From<PresetArg> for Preset {
    fn from(p: PresetArg) -> Self {
        match p {
            PresetArg::Light => Preset::Light,
            PresetArg::Full => Preset::Full,
        }
    }
}

#[derive(Args, Clone, Copy, Default)]
struct OverrideArgs {
    /// 安全装置を外す：初期設定画面（Wi-Fi の持ち主でも止める。有線 LAN のときだけ）
    #[arg(long)]
    force_setupwraith: bool,
    /// 安全装置を外す：今のホーム画面でも止める
    #[arg(long)]
    force_home: bool,
    /// 安全装置を外す：HDMI などの入力の土台でも止める
    #[arg(long)]
    tvinput_allow: bool,
    /// 安全装置を外す：入力切替・設定を開くアプリでも止める
    #[arg(long)]
    handlers_allow: bool,
}

impl From<OverrideArgs> for Overrides {
    fn from(o: OverrideArgs) -> Self {
        Overrides {
            force_setupwraith: o.force_setupwraith,
            force_home: o.force_home,
            tvinput_allow: o.tvinput_allow,
            handlers_allow: o.handlers_allow,
        }
    }
}

fn lang() -> Lang {
    Lang::from_env()
}
fn tr<'a>(ja: &'a str, en: &'a str) -> &'a str {
    if lang() == Lang::Ja { ja } else { en }
}

fn connect(host: &str) -> Result<Device> {
    eprintln!("{} {host} …", tr("つないでいます", "Connecting to"));
    eprintln!(
        "{}",
        tr(
            "（初めてのときは、テレビに「USB デバッグを許可しますか？」が出ます。［許可］を選んでください）",
            "(The first time, the TV asks \"Allow USB debugging?\". Choose Allow.)"
        )
    );
    Ok(Device::connect(host, &default_key_path())?)
}

/// 使うリスト：(表示名, 記録に使うファイルの場所, 中身)
fn resolve_list(t: &Target, dev: &mut Device) -> Result<(String, PathBuf, DebloatList)> {
    if let Some(f) = &t.file {
        let text = std::fs::read_to_string(f)
            .with_context(|| format!("{} {}", tr("読めません:", "cannot read"), f.display()))?;
        return Ok((f.display().to_string(), f.clone(), list::parse(&text)?));
    }
    let all = lists::all();
    let info: ListInfo = match &t.list {
        Some(id) => lists::get(id)
            .with_context(|| format!("{} {id}", tr("そのリストはありません:", "no such list:")))?,
        None => {
            let (installed, _) = dev.packages()?;
            let model = dev.model();
            let ranked = plan::rank_lists(&installed, model.as_deref(), &all);
            let Some(&(score, _, best)) = ranked.first() else {
                bail!(tr(
                    "近いリストが見つかりませんでした。--list か --file で指定してください",
                    "No close list found; pass --list or --file"
                ))
            };
            eprintln!(
                "{} {} ({}: {:.0}%)",
                tr("近いリスト:", "Closest list:"),
                best.id,
                tr("一致", "match"),
                score.min(1.0) * 100.0
            );
            best.clone()
        }
    };
    let (file, text) = info.files[0].clone();
    // 記録の名前はリストのファイルの場所で決まるので、決まった場所に書き出してから使う
    let path = data_dir().join("lists").join(&info.id).join(&file);
    std::fs::create_dir_all(path.parent().unwrap())?;
    if std::fs::read_to_string(&path).ok().as_deref() != Some(text.as_str()) {
        std::fs::write(&path, &text)?;
    }
    Ok((info.id.clone(), path, list::parse(&text)?))
}

fn ask(question: &str) -> bool {
    eprint!("{question} [y/N] ");
    let _ = std::io::stderr().flush();
    let mut s = String::new();
    std::io::stdin().read_line(&mut s).is_ok() && matches!(s.trim(), "y" | "Y" | "yes")
}

fn print_plan(p: &Plan) {
    let l = lang();
    println!(
        "{}: {}  /  {}: {}  /  {}: {}  /  {}: {}",
        tr("止める", "disable"),
        p.to_disable.len(),
        tr("元から止まっている", "already off"),
        p.already_disabled.len(),
        tr("守る", "protected"),
        p.protected.len(),
        tr("このテレビにない", "not installed"),
        p.absent.len()
    );
    for n in &p.to_disable {
        println!("  - {n}");
    }
    if !p.protected.is_empty() {
        println!(
            "{}",
            tr("止めずに残すもの（安全装置）:", "Kept on (safety checks):")
        );
        for (n, r) in &p.protected {
            println!("  ! {n}\n      {}", r.text(l));
        }
    }
    for note in &p.notes {
        println!("{} {}", tr("注意:", "Note:"), note.text(l));
    }
}

fn make_plan(
    dev: &mut Device,
    list: &DebloatList,
    ov: Overrides,
    preset: Preset,
) -> Result<(Plan, record::TvState)> {
    let names = category::select(list, preset);
    let state = dev.state()?;
    let guarded = safety::run(&names, dev, ov);
    Ok((Plan::build(&names, &guarded, &state), state))
}

fn main() {
    if let Err(e) = run(Cli::parse()) {
        eprintln!("{} {e:#}", tr("エラー:", "error:"));
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<()> {
    match cli.cmd {
        Cmd::Scan { seconds } => {
            eprintln!("{}", tr("探しています…", "Searching…"));
            let found = hushtv_adb::discover::scan(Duration::from_secs(seconds));
            if found.is_empty() {
                println!(
                    "{}",
                    tr(
                        "見つかりませんでした。テレビのデバッグがオンか、同じ Wi-Fi かを確かめてください",
                        "Nothing found. Check that debugging is on and the TV is on the same network"
                    )
                );
            }
            for tv in found {
                let how = match (tv.legacy_open, tv.tls_port) {
                    (true, _) => format!("{}:5555", tv.address),
                    (false, Some(p)) => format!(
                        "{}:{p}（{}）",
                        tv.address,
                        tr("ワイヤレスデバッグ", "wireless debugging")
                    ),
                    _ => format!("{}（{}）", tv.address, tr("デバッグ オフ", "debugging off")),
                };
                println!("{}  {how}", tv.name.unwrap_or_else(|| "-".into()));
            }
        }
        Cmd::Lists { word } => {
            let w = word.unwrap_or_default().to_lowercase();
            let all = lists::all();
            let mut n = 0;
            for l in all.iter().filter(|l| {
                w.is_empty() || l.id.contains(&w) || l.title().to_lowercase().contains(&w)
            }) {
                println!("{:<44} {}", l.id, l.title());
                n += 1;
            }
            eprintln!(
                "{n} / {} （cleanyourtv {}）",
                all.len(),
                &lists::upstream_commit()[..12.min(lists::upstream_commit().len())]
            );
        }
        Cmd::Info { host } => {
            let mut dev = connect(&host)?;
            println!(
                "{}: {}",
                tr("機種", "Model"),
                dev.model().unwrap_or_else(|| "?".into())
            );
            println!(
                "Android: {}",
                dev.android_version().unwrap_or_else(|| "?".into())
            );
            let (installed, disabled) = dev.packages()?;
            println!(
                "{}: {} / {}: {}",
                tr("入っているアプリ", "Installed"),
                installed.len(),
                tr("止まっている", "disabled"),
                disabled.len()
            );
            let all = lists::all();
            let model = dev.model();
            println!("{}", tr("近いリスト:", "Closest lists:"));
            for (score, hit, l) in plan::rank_lists(&installed, model.as_deref(), &all)
                .into_iter()
                .take(5)
            {
                println!(
                    "  {:>4.0}%  {:>3}  {:<40} {}",
                    score.min(1.0) * 100.0,
                    hit,
                    l.id,
                    l.title()
                );
            }
        }
        Cmd::Plan(t) => {
            let mut dev = connect(&t.host)?;
            let (_, _, list) = resolve_list(&t, &mut dev)?;
            let (p, _) = make_plan(&mut dev, &list, t.ov.into(), t.preset.into())?;
            print_plan(&p);
            println!(
                "{}",
                tr("（まだ何も変えていません）", "(Nothing was changed.)")
            );
        }
        Cmd::Apply { target: t, yes } => {
            let mut dev = connect(&t.host)?;
            let (id, path, list) = resolve_list(&t, &mut dev)?;
            let (p, _) = make_plan(&mut dev, &list, t.ov.into(), t.preset.into())?;
            print_plan(&p);
            if p.to_disable.is_empty() {
                println!("{}", tr("止めるものはありません。", "Nothing to disable."));
                return Ok(());
            }
            if !yes
                && !ask(tr(
                    "戻す記録を保存してから、上のアプリを止めます。続けますか？",
                    "Save the restore record, then disable the apps above?",
                ))
            {
                println!(
                    "{}",
                    tr(
                        "やめました。何も変えていません。",
                        "Stopped. Nothing was changed."
                    )
                );
                return Ok(());
            }
            let opt = flow::ApplyOptions {
                safe: !t.fast,
                batch: t.batch,
                share: !t.no_share,
                ov: t.ov.into(),
            };
            let out = flow::apply(
                &mut dev,
                &id,
                &path,
                &list,
                &p.to_disable,
                opt,
                |pr| match pr {
                    flow::Progress::Batch { done, total } => eprintln!(
                        "{} {done}/{total}",
                        tr(
                            "安全モード：確かめながら止めています",
                            "Safe mode: disabling and checking"
                        )
                    ),
                    flow::Progress::Package { done, total, name } => {
                        eprintln!("[{done}/{total}] {name}")
                    }
                    flow::Progress::ShotBefore | flow::Progress::ShotAfter => eprintln!(
                        "{}",
                        tr("ホーム画面を撮っています…", "Taking a home screen picture…")
                    ),
                    _ => {}
                },
            )?;
            println!(
                "{} {}",
                tr("戻す記録:", "Restore record:"),
                out.record_path.display()
            );
            println!("{} {}", tr("止めました:", "Disabled:"), out.disabled.len());
            if let Some(run) = &out.safe
                && let Some(why) = &run.stopped
            {
                println!(
                    "{} {why:?}",
                    tr(
                        "テレビの様子がおかしくなったので、その組を元に戻して止めました:",
                        "The TV stopped responding normally, so that batch was undone and the run stopped:"
                    )
                );
                println!("  {} {:?}", tr("元に戻した:", "undone:"), run.rolled_back);
                println!(
                    "  {} {}",
                    tr("送らなかった:", "not sent:"),
                    run.not_sent.len()
                );
            }
            if let Some(card) = &out.card_path {
                println!("{} {}", tr("共有用の画像:", "Share image:"), card.display());
            }
            if !out.still_on.is_empty() {
                println!(
                    "{}",
                    tr(
                        "テレビが受け付けず、まだ動いているもの（繰り返し実行しないでください）:",
                        "Still enabled; the TV did not take these (do not rerun in a loop):"
                    )
                );
                for n in &out.still_on {
                    println!("  - {n}");
                }
            }
            if out.problems > 0 || !out.still_on.is_empty() {
                std::process::exit(1);
            }
            println!(
                "{}",
                tr(
                    "終わったら、テレビの設定でデバッグをオフにしてください。",
                    "When you are done, turn debugging off in the TV's settings."
                )
            );
        }
        Cmd::Check { host, fix, yes, ov } => {
            let applied: Applied = serde_json::from_str(
                &std::fs::read_to_string(applied_path(&host)).with_context(|| {
                    tr(
                        "このテレビを片付けた控えがありません（先に apply を実行してください）",
                        "No record of cleaning this TV; run apply first",
                    )
                })?,
            )?;
            let mut dev = connect(&host)?;
            let now = dev.state()?;
            let d = plan::drift(&applied, &now);
            if d.is_clean() {
                println!("{}", tr("ずれはありません。", "No drift."));
                return Ok(());
            }
            for n in &d.returned {
                println!("{} {n}", tr("戻ってきた:", "came back:"));
            }
            for n in &d.new_apps {
                println!("{} {n}", tr("新しく増えた:", "new app:"));
            }
            if let Some((before, after)) = &d.home_changed {
                println!(
                    "{} {} → {}",
                    tr("ホームが変わった:", "Home changed:"),
                    before.as_deref().unwrap_or("-"),
                    after.as_deref().unwrap_or("-")
                );
            }
            if !fix {
                println!(
                    "{}",
                    tr(
                        "直すには --fix を付けて実行してください（新しく増えたアプリは止めません）。",
                        "Run with --fix to repair (new apps are left alone)."
                    )
                );
                std::process::exit(1);
            }
            if !yes
                && !ask(tr(
                    "戻ってきたアプリを安全装置を通して止め直し、ホームを戻します。続けますか？",
                    "Disable the returned apps again (through the safety checks) and put Home back?",
                ))
            {
                return Ok(());
            }
            let g = safety::run(&d.returned, &mut dev, ov.into());
            for (n, r) in &g.skipped {
                println!("  ! {n}: {}", r.text(lang()));
            }
            let still = dev.disable(&g.send, |_, n| eprintln!("  {n}"));
            if let (Some(_), Some(home)) = (&d.home_changed, &applied.home)
                && hushtv_core::names::is_component(home)
            {
                let _ = dev.shell(&format!("cmd package set-home-activity --user 0 {home}"));
            }
            if still.is_empty() {
                println!("{}", tr("直しました。", "Fixed."));
            } else {
                println!("{} {still:?}", tr("まだ動いているもの:", "Still enabled:"));
                std::process::exit(1);
            }
        }
        Cmd::Rescue {
            host,
            level,
            record,
            dry_run,
            yes,
        } => {
            let mut dev = match Device::connect(&host, &default_key_path()) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("{} {e}", tr("つながりません:", "Cannot connect:"));
                    println!("{}", tr(OFFLINE_JA, OFFLINE_EN));
                    std::process::exit(1);
                }
            };
            let facts = flow::rescue_facts(&mut dev)?;
            let problems = hushtv_core::rescue::diagnose(&facts);
            if problems.is_empty() {
                println!(
                    "{}",
                    tr("問題は見つかりませんでした。", "No problems found.")
                );
            }
            for p in &problems {
                println!(
                    "{} {}",
                    if p.severity() >= 4 {
                        "!!"
                    } else if p.severity() >= 2 {
                        " !"
                    } else {
                        " -"
                    },
                    p.title(lang())
                );
            }
            let records = flow::records_for_host(&host);
            for (path, r) in &records {
                println!(
                    "{} {} ({})",
                    tr("戻す記録:", "Restore record:"),
                    path.display(),
                    r.created
                );
            }
            if dry_run || (problems.is_empty() && !record) {
                return Ok(());
            }
            if record {
                let Some((path, rec)) = records.first() else {
                    bail!(tr(
                        "このテレビの戻す記録が見つかりません",
                        "No restore record for this TV"
                    ))
                };
                if !yes
                    && !ask(tr(
                        "この記録どおりに戻します。続けますか？",
                        "Restore this record?",
                    ))
                {
                    return Ok(());
                }
                let n = flow::restore_from(&mut dev, rec, None);
                println!(
                    "{} {n} ({})",
                    tr("送ったコマンド:", "Commands sent:"),
                    path.display()
                );
            } else {
                let lvl = match level {
                    LevelArg::Quick => hushtv_core::rescue::Level::Quick,
                    LevelArg::All => hushtv_core::rescue::Level::All,
                };
                let cmds = hushtv_core::rescue::fix_commands(&problems, &facts, lvl);
                for c in &cmds {
                    println!("  {c}");
                }
                if cmds.is_empty()
                    || (!yes
                        && !ask(tr(
                            "上のコマンドで直します（戻すだけで、止めたり消したりはしません）。続けますか？",
                            "Run the commands above? (They only turn things back on.)",
                        )))
                {
                    return Ok(());
                }
                flow::rescue(&mut dev, lvl)?;
            }
            let after = hushtv_core::rescue::diagnose(&flow::rescue_facts(&mut dev)?);
            let serious: Vec<_> = after.iter().filter(|p| p.severity() >= 2).collect();
            if serious.is_empty() {
                println!(
                    "{}",
                    tr(
                        "直りました。テレビを再起動して、ホーム・入力・設定・Wi-Fi を確かめてください。",
                        "Done. Reboot the TV and check Home, inputs, Settings and Wi-Fi."
                    )
                );
            } else {
                println!("{}", tr("まだ残っている問題:", "Still not fixed:"));
                for p in serious {
                    println!("  {}", p.title(lang()));
                }
                std::process::exit(1);
            }
        }
        Cmd::Restore {
            target: t,
            yes,
            stock,
        } => {
            let mut dev = connect(&t.host)?;
            let (_, path, _) = resolve_list(&t, &mut dev)?;
            let rec_path = record::find_record(&records_dir(), &path, &t.host)
                .map_err(|f| anyhow::anyhow!("{f:#?}"))?;
            if !rec_path.exists() {
                bail!(
                    "{} {}",
                    tr(
                        "このテレビとリストの戻す記録がありません:",
                        "No restore record for this TV and list:"
                    ),
                    rec_path.display()
                );
            }
            let rec =
                record::load(&std::fs::read_to_string(&rec_path)?).map_err(anyhow::Error::msg)?;
            let cmds = record::restore_commands(&rec, stock.as_deref());
            println!("{} {}", tr("記録の日時:", "Recorded at:"), rec.created);
            for c in &cmds {
                println!("  {c}");
            }
            if !yes
                && !ask(tr(
                    "上のコマンドで元に戻します。続けますか？",
                    "Run the commands above to restore?",
                ))
            {
                return Ok(());
            }
            for (c, out) in dev.run_all(&cmds) {
                let out = out.trim();
                println!(
                    "{c}{}",
                    if out.is_empty() {
                        String::new()
                    } else {
                        format!("\n    {out}")
                    }
                );
            }
            let _ = std::fs::remove_file(applied_path(&t.host));
            println!("{}", tr("元に戻しました。", "Restored."));
        }
    }
    Ok(())
}

const OFFLINE_JA: &str = "テレビにつながらないときの直し方（docs/RESCUE.md にも同じ内容）:
  1. Wi-Fi が消えた: 有線 LAN をつなぐか、リモコンの設定ボタン → ネットワーク で Wi-Fi に入り直す
  2. 画面が真っ黒・ホームが開かない: リモコンの［設定］［入力切替］ボタンか音声で設定を開き、開発者向けオプションでデバッグをオンにしてから、もう一度 hushtv rescue
  3. 起動ロゴのまま・再起動を繰り返す: 電源ボタンなどを押しながら電源を入れて出る「リカバリー」から初期化（データが消えます。機種ごとに操作が違うので説明書を確認）";
const OFFLINE_EN: &str = "If the TV cannot be reached (also in docs/RESCUE.md):
  1. Wi-Fi is gone: plug in Ethernet, or re-join Wi-Fi from the remote's Settings button -> Network
  2. Black screen / Home does not open: open Settings with the remote's Settings or Input button or by voice, turn debugging back on in Developer options, then run hushtv rescue again
  3. Stuck on the boot logo / restarting: factory reset from the recovery menu (hold a button while powering on; it erases the TV, and the steps differ per model)";
