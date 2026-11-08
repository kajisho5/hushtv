//! hushtv の中身。テレビと通信しない計算だけを置く（テストしやすくするため）。
//!
//! - [`list`]：機種別リストの読み込み
//! - [`lists`]：同梱リスト（cleanyourtv の examples/）
//! - [`safety`]：安全装置（cleanyourtv の disable.sh の検査を移したもの）
//! - [`record`]：戻す記録（cleanyourtv と同じ形式）
//! - [`plan`]：計画・機種の当てはめ・点検

pub mod category;
pub mod health;
pub mod list;
pub mod lists;
pub mod names;
pub mod plan;
pub mod record;
pub mod safety;
