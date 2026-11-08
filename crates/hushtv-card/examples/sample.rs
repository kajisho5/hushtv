//! README 用の見本画像を作る。架空のホーム画面の写真 2 枚から共有用画像を作る。
//!
//!     cargo run -p hushtv-card --example sample -- before.png after.png out.png
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let before = std::fs::read(&a[1]).ok();
    let after = std::fs::read(&a[2]).ok();
    let stats = hushtv_card::Stats {
        switched_off: 20,
        by_category: vec![("Ads".into(), 9), ("Tracking".into(), 11)],
    };
    let png = hushtv_card::render(before.as_deref(), after.as_deref(), &stats).expect("render");
    std::fs::write(&a[3], png).expect("write");
}
