//! 何台もまとめて：同じやり方（軽め／全部・安全モード）を、テレビごとに一番近いリストで順番に反映する。
//! 1 台が失敗しても次へ進む。結果は 1 台ずつ返す。

use crate::flow::{self, ApplyOptions, materialize_list};
use crate::{Device, default_key_path};
use hushtv_core::category::{self, Preset};
use hushtv_core::plan::{self, Plan};
use hushtv_core::{list, lists, safety};
use serde::Serialize;

/// 1 台ぶんの結果。
#[derive(Debug, Clone, Default, Serialize)]
pub struct TvResult {
    pub host: String,
    pub model: Option<String>,
    pub list_id: Option<String>,
    /// 止めた数
    pub disabled: usize,
    /// 安全装置で残した数
    pub protected: usize,
    /// 安全モードで戻した組があったか
    pub problems: usize,
    /// 点検：戻ってきたアプリの数
    pub returned: usize,
    /// 点検：ホームが変わっていたか
    pub home_changed: bool,
    /// 戻した数（restore）
    pub restored: usize,
    /// うまくいかなかった理由（そのテレビは飛ばした）
    pub error: Option<String>,
}

/// まとめてやること。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Job {
    /// 戻す記録を保存してから止める
    Apply,
    /// ファーム更新のあとのずれを数える（変えない）
    Check,
    /// ずれを直す（戻ってきたアプリを止め直す）
    Fix,
    /// 記録どおりに戻す
    Restore,
}

/// テレビの住所を読みやすい一覧から取り出す（空行・# のコメントは飛ばす。1 行に 1 台、空白区切りも可）。
pub fn parse_hosts(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("");
        for w in line.split([' ', '\t', ',']) {
            let w = w.trim();
            if !w.is_empty() && !out.iter().any(|h| h == w) {
                out.push(w.to_string());
            }
        }
    }
    out
}

/// 1 台ぶん。進み具合は `say` に流す。
pub fn run_one(
    host: &str,
    job: Job,
    preset: Preset,
    opt: ApplyOptions,
    say: &mut dyn FnMut(&str),
) -> TvResult {
    let mut r = TvResult {
        host: host.to_string(),
        ..Default::default()
    };
    let mut dev = match Device::connect(host, &default_key_path()) {
        Ok(d) => d,
        Err(e) => {
            r.error = Some(e.to_string());
            return r;
        }
    };
    r.model = dev.model();
    let result = match job {
        Job::Apply => apply_one(&mut dev, &mut r, preset, opt, say),
        Job::Check | Job::Fix => check_one(&mut dev, &mut r, job == Job::Fix, opt),
        Job::Restore => restore_one(&mut dev, &mut r),
    };
    if let Err(e) = result {
        r.error = Some(e);
    }
    r
}

fn apply_one(
    dev: &mut Device,
    r: &mut TvResult,
    preset: Preset,
    opt: ApplyOptions,
    say: &mut dyn FnMut(&str),
) -> Result<(), String> {
    let (installed, _) = dev.packages().map_err(|e| e.to_string())?;
    let all = lists::all();
    let ranked = plan::rank_lists(&installed, r.model.as_deref(), &all);
    let Some(&(_, _, best)) = ranked.first() else {
        return Err("no close list / 近いリストがありません".into());
    };
    r.list_id = Some(best.id.clone());
    let path = materialize_list(best).map_err(|e| e.to_string())?;
    let parsed = list::parse(&best.files[0].1).map_err(|e| e.to_string())?;
    let names = category::select(&parsed, preset);
    let state = dev.state().map_err(|e| e.to_string())?;
    let guarded = safety::run(&names, dev, opt.ov);
    let p: Plan = Plan::build(&names, &guarded, &state);
    r.protected = p.protected.len();
    if p.to_disable.is_empty() {
        return Ok(());
    }
    let out = flow::apply(dev, &best.id, &path, &parsed, &p.to_disable, opt, |pr| {
        if let flow::Progress::Batch { done, total } = pr {
            say(&format!("{done}/{total}"));
        }
    })
    .map_err(|e| e.to_string())?;
    r.disabled = out.disabled.len();
    r.problems = out.problems;
    Ok(())
}

fn check_one(
    dev: &mut Device,
    r: &mut TvResult,
    fix: bool,
    opt: ApplyOptions,
) -> Result<(), String> {
    let applied = flow::load_applied(&dev.host)
        .ok_or_else(|| "not cleaned with hushtv yet / まだ片付けていません".to_string())?;
    let now = dev.state().map_err(|e| e.to_string())?;
    let d = plan::drift(&applied, &now);
    r.returned = d.returned.len();
    r.home_changed = d.home_changed.is_some();
    if fix && !d.returned.is_empty() {
        let guarded = safety::run(&d.returned, dev, opt.ov);
        let send: Vec<String> = d
            .returned
            .iter()
            .filter(|n| !guarded.skipped.iter().any(|(s, _)| s == *n))
            .cloned()
            .collect();
        let still_on = dev.disable(&send, |_, _| {});
        r.disabled = send.len() - still_on.len();
    }
    Ok(())
}

fn restore_one(dev: &mut Device, r: &mut TvResult) -> Result<(), String> {
    let applied = flow::load_applied(&dev.host)
        .ok_or_else(|| "no restore point / 戻す記録がありません".to_string())?;
    let info = lists::get(&applied.list_id)
        .ok_or_else(|| format!("list {} is gone / リストがありません", applied.list_id))?;
    r.list_id = Some(applied.list_id.clone());
    let path = materialize_list(&info).map_err(|e| e.to_string())?;
    r.restored = flow::restore(dev, &path, None).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hosts_from_text() {
        let t =
            "# 1 階\n192.168.10.23\n192.168.10.41:5555, 192.168.10.57  # 書斎\n\n192.168.10.23\n";
        assert_eq!(
            parse_hosts(t),
            vec!["192.168.10.23", "192.168.10.41:5555", "192.168.10.57"]
        );
    }
}
