//! 共有用のビフォー・アフター画像（1600×900 の PNG）。
//!
//! 文字は同梱の Inter（ラテン文字だけに絞ったもの、OFL）で描くので、画像の文字は英語。
//! 撮れなかった画面は「撮れませんでした」の枠にする。

use ab_glyph::{Font, FontRef, PxScale, ScaleFont, point};
use image::{DynamicImage, ImageFormat, Rgba, RgbaImage, imageops};
use std::io::Cursor;

const BOLD: &[u8] = include_bytes!("../fonts/Inter-Bold-latin.otf");
const REGULAR: &[u8] = include_bytes!("../fonts/Inter-Regular-latin.otf");

pub const WIDTH: u32 = 1600;
pub const HEIGHT: u32 = 900;

const BG: [u8; 4] = [0x0B, 0x10, 0x20, 255];
const PANEL: [u8; 4] = [0x14, 0x1C, 0x33, 255];
const TEXT: [u8; 4] = [0xE8, 0xEC, 0xF6, 255];
const SUB: [u8; 4] = [0x9A, 0xA6, 0xC4, 255];
const MINT: [u8; 4] = [0x5E, 0xEA, 0xD4, 255];
const DANGER: [u8; 4] = [0xF8, 0x71, 0x71, 255];

/// 画像に載せる数字。
#[derive(Debug, Clone, Default)]
pub struct Stats {
    /// 止めたアプリの数
    pub switched_off: usize,
    /// 種類ごとの数（例 ("Ads", 9)）。英語のラベルで渡す
    pub by_category: Vec<(String, usize)>,
}

#[derive(Debug)]
pub struct Error(pub String);
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for Error {}

fn fill_rect(img: &mut RgbaImage, x: i64, y: i64, w: u32, h: u32, c: [u8; 4]) {
    for yy in y.max(0)..(y + h as i64).min(img.height() as i64) {
        for xx in x.max(0)..(x + w as i64).min(img.width() as i64) {
            img.put_pixel(xx as u32, yy as u32, Rgba(c));
        }
    }
}

fn blend(dst: &mut Rgba<u8>, c: [u8; 4], a: f32) {
    for (d, s) in dst.0.iter_mut().zip(c).take(3) {
        *d = (*d as f32 * (1.0 - a) + s as f32 * a).round() as u8;
    }
}

/// 文字の幅（px）。
pub fn text_width(text: &str, size: f32, bold: bool) -> f32 {
    let font = FontRef::try_from_slice(if bold { BOLD } else { REGULAR }).unwrap();
    let f = font.as_scaled(PxScale::from(size));
    let mut w = 0.0;
    let mut prev = None;
    for ch in text.chars() {
        let id = f.glyph_id(ch);
        if let Some(p) = prev {
            w += f.kern(p, id);
        }
        w += f.h_advance(id);
        prev = Some(id);
    }
    w
}

/// 左上 (x, ベースライン y) から文字を描く。
pub fn draw_text(
    img: &mut RgbaImage,
    x: f32,
    y: f32,
    text: &str,
    size: f32,
    bold: bool,
    color: [u8; 4],
) {
    let font = FontRef::try_from_slice(if bold { BOLD } else { REGULAR }).unwrap();
    let f = font.as_scaled(PxScale::from(size));
    let mut caret = x;
    let mut prev = None;
    for ch in text.chars() {
        let id = f.glyph_id(ch);
        if let Some(p) = prev {
            caret += f.kern(p, id);
        }
        let glyph = id.with_scale_and_position(PxScale::from(size), point(caret, y));
        if let Some(outline) = font.outline_glyph(glyph) {
            let b = outline.px_bounds();
            outline.draw(|gx, gy, cov| {
                let px = b.min.x as i64 + gx as i64;
                let py = b.min.y as i64 + gy as i64;
                if px >= 0 && py >= 0 && (px as u32) < img.width() && (py as u32) < img.height() {
                    blend(img.get_pixel_mut(px as u32, py as u32), color, cov.min(1.0));
                }
            });
        }
        caret += f.h_advance(id);
        prev = Some(id);
    }
}

/// 画面写真を枠に収める。読めなければ「撮れませんでした」の枠。
fn place_shot(img: &mut RgbaImage, shot: Option<&[u8]>, x: i64, y: i64, w: u32, h: u32) {
    fill_rect(img, x, y, w, h, PANEL);
    let decoded = shot.and_then(|b| image::load_from_memory_with_format(b, ImageFormat::Png).ok());
    match decoded {
        Some(pic) => {
            let fitted = pic.resize(w, h, imageops::FilterType::Triangle).to_rgba8();
            let ox = x + (w as i64 - fitted.width() as i64) / 2;
            let oy = y + (h as i64 - fitted.height() as i64) / 2;
            imageops::overlay(img, &fitted, ox, oy);
        }
        None => {
            let msg = "Screenshot not available";
            let tw = text_width(msg, 24.0, false);
            draw_text(
                img,
                x as f32 + (w as f32 - tw) / 2.0,
                y as f32 + h as f32 / 2.0 + 8.0,
                msg,
                24.0,
                false,
                SUB,
            );
        }
    }
}

/// ビフォー・アフター画像を作って PNG で返す。
pub fn render(
    before: Option<&[u8]>,
    after: Option<&[u8]>,
    stats: &Stats,
) -> Result<Vec<u8>, Error> {
    let mut img = RgbaImage::from_pixel(WIDTH, HEIGHT, Rgba(BG));
    // 見出し
    draw_text(&mut img, 64.0, 92.0, "hushtv", 44.0, true, TEXT);
    draw_text(
        &mut img,
        64.0 + text_width("hushtv", 44.0, true) + 18.0,
        92.0,
        "Quiet your Android TV",
        26.0,
        false,
        SUB,
    );
    // 写真 2 枚（16:9）
    let (w, h) = (720u32, 405u32);
    let (lx, rx, top) = (64i64, 816i64, 172i64);
    place_shot(&mut img, before, lx, top, w, h);
    place_shot(&mut img, after, rx, top, w, h);
    for (x, label, color) in [(lx, "BEFORE", DANGER), (rx, "AFTER", MINT)] {
        draw_text(
            &mut img,
            x as f32,
            (top - 14) as f32,
            label,
            20.0,
            true,
            color,
        );
    }
    // 数字
    let big = format!("{} apps switched off", stats.switched_off);
    draw_text(&mut img, 64.0, 672.0, &big, 64.0, true, MINT);
    let detail: Vec<String> = stats
        .by_category
        .iter()
        .filter(|(_, n)| *n > 0)
        .map(|(l, n)| format!("{l} {n}"))
        .collect();
    if !detail.is_empty() {
        draw_text(
            &mut img,
            64.0,
            718.0,
            &detail.join("  ·  "),
            30.0,
            false,
            TEXT,
        );
    }
    draw_text(
        &mut img,
        64.0,
        780.0,
        "Nothing uninstalled. Every change can be undone.",
        26.0,
        false,
        SUB,
    );
    let url = "github.com/kajisho5/hushtv";
    let uw = text_width(url, 26.0, true);
    draw_text(
        &mut img,
        WIDTH as f32 - 64.0 - uw,
        846.0,
        url,
        26.0,
        true,
        SUB,
    );
    let mut out = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(img)
        .write_to(&mut out, ImageFormat::Png)
        .map_err(|e| Error(e.to_string()))?;
    Ok(out.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_shot(color: [u8; 4]) -> Vec<u8> {
        let img = RgbaImage::from_pixel(1920, 1080, Rgba(color));
        let mut out = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(img)
            .write_to(&mut out, ImageFormat::Png)
            .unwrap();
        out.into_inner()
    }

    #[test]
    fn renders_a_png_of_the_right_size() {
        let stats = Stats {
            switched_off: 24,
            by_category: vec![
                ("Ads".into(), 9),
                ("Tracking".into(), 11),
                ("Promo rows".into(), 4),
            ],
        };
        let png = render(Some(&fake_shot([40, 60, 120, 255])), None, &stats).unwrap();
        let img = image::load_from_memory(&png).unwrap();
        assert_eq!((img.width(), img.height()), (WIDTH, HEIGHT));
        if let Ok(dir) = std::env::var("HUSHTV_CARD_OUT") {
            std::fs::write(format!("{dir}/card-sample.png"), &png).unwrap();
        }
    }

    #[test]
    fn text_has_width() {
        assert!(text_width("hushtv", 44.0, true) > 100.0);
    }
}
