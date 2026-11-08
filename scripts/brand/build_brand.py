#!/usr/bin/env python3
"""hushtv のロゴ・アイコン・ソーシャルプレビューを作る。

    python3 scripts/brand/build_brand.py

必要なもの：fontTools（文字を図形に変える）、rsvg-convert（SVG → PNG）、Inter フォント。
書き出し先は brand/。ロゴの文字は図形にしてあるので、フォントがない環境でも同じ見た目になる。
"""
import shutil
import subprocess
from pathlib import Path

from fontTools.pens.svgPathPen import SVGPathPen
from fontTools.pens.transformPen import TransformPen
from fontTools.ttLib import TTFont

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "brand"
INTER_BOLD = Path("/usr/share/fonts/opentype/inter/Inter-Bold.otf")

INK = "#0B1020"      # 夜の紺（背景・暗い文字）
DEEP = "#16224A"
MINT = "#5EEAD4"     # 強調（静かな波）
MINT_DARK = "#0E9F8E"  # 白い背景の上のミント
WHITE = "#F4F7FC"


def text_path(text: str, size: float, x: float, y: float, font_path: Path = INTER_BOLD, tracking: float = 0.0):
    """文字列を SVG の path にする。(path の d, 幅) を返す。y はベースライン。"""
    font = TTFont(font_path)
    gs = font.getGlyphSet()
    cmap = font.getBestCmap()
    upm = font["head"].unitsPerEm
    scale = size / upm
    pen = SVGPathPen(gs)
    caret = 0.0
    for ch in text:
        name = cmap[ord(ch)]
        g = gs[name]
        tp = TransformPen(pen, (scale, 0, 0, -scale, x + caret, y))
        g.draw(tp)
        caret += g.width * scale + tracking
    return pen.getCommands(), caret - tracking


def mark(x: float, y: float, s: float, stroke: str, wave: str, bg: str | None = None, radius: float = 0.0) -> str:
    """ロゴマーク：テレビの枠と、静かになった波。s は一辺の大きさ。"""
    k = s / 256
    parts = []
    if bg:
        parts.append(f'<rect x="{x}" y="{y}" width="{s}" height="{s}" rx="{radius * k}" fill="{bg}"/>')
    sw = 18 * k
    parts.append(f'<rect x="{x + 40 * k}" y="{y + 62 * k}" width="{176 * k}" height="{118 * k}" rx="{24 * k}" fill="none" stroke="{stroke}" stroke-width="{sw}"/>')
    parts.append(f'<path d="M{x + 98 * k} {y + 208 * k}h{60 * k}" stroke="{stroke}" stroke-width="{sw}" stroke-linecap="round"/>')
    # 波：左は大きく、右へ行くほど静かになる
    parts.append(
        f'<path d="M{x + 74 * k} {y + 121 * k}c{12 * k} {-26 * k} {24 * k} {-26 * k} {36 * k} 0s{22 * k} {18 * k} {32 * k} 0s{18 * k} {-8 * k} {26 * k} 0s{12 * k} 2 {16 * k} 0" '
        f'stroke="{wave}" stroke-width="{16 * k}" fill="none" stroke-linecap="round" stroke-linejoin="round"/>'
    )
    return "".join(parts)


def svg(w: float, h: float, body: str, label: str = "hushtv") -> str:
    return f'<svg xmlns="http://www.w3.org/2000/svg" xml:space="preserve" viewBox="0 0 {w:g} {h:g}" width="{w:g}" height="{h:g}" role="img" aria-label="{label}"><title>{label}</title>{body}</svg>\n'


def write(name: str, content: str) -> Path:
    p = OUT / name
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(content, encoding="utf-8")
    return p


def png(src: Path, dest: Path, width: int, height: int | None = None):
    args = ["rsvg-convert", "-w", str(width)]
    if height:
        args += ["-h", str(height)]
    subprocess.run(args + [str(src), "-o", str(dest)], check=True)


def main():
    OUT.mkdir(exist_ok=True)
    # 1. ロゴマーク単体
    write("logo/hushtv-mark.svg", svg(256, 256, mark(0, 0, 256, INK, MINT_DARK)))
    write("logo/hushtv-mark-on-dark.svg", svg(256, 256, mark(0, 0, 256, WHITE, MINT)))
    # 2. 文字だけ
    d, w = text_path("hushtv", 160, 0, 150, tracking=-3)
    write("logo/hushtv-wordmark-black.svg", svg(w + 6, 190, f'<path fill="{INK}" d="{d}"/>'))
    write("logo/hushtv-wordmark-white.svg", svg(w + 6, 190, f'<path fill="{WHITE}" d="{d}"/>'))
    # 3. 横並び（マーク＋文字）
    d, w = text_path("hushtv", 150, 250, 172, tracking=-3)
    total = 250 + w + 20
    write("logo/hushtv-horizontal.svg", svg(total, 256, mark(0, 0, 256, INK, MINT_DARK) + f'<path fill="{INK}" d="{d}"/>'))
    write("logo/hushtv-horizontal-on-dark.svg", svg(total, 256, mark(0, 0, 256, WHITE, MINT) + f'<path fill="{WHITE}" d="{d}"/>'))
    # 4. アプリのアイコン（暗い版が標準、明るい版も）
    grad = f'<defs><linearGradient id="g" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="{DEEP}"/><stop offset="1" stop-color="{INK}"/></linearGradient></defs>'
    icon_dark = write("app-icon-dark.svg", svg(1024, 1024, grad + mark(64, 64, 896, MINT, MINT, "url(#g)", 57)))
    icon_light = write("app-icon.svg", svg(1024, 1024, mark(64, 64, 896, INK, MINT_DARK, "#EAF7F5", 57)))
    png(icon_dark, OUT / "app-icon-dark.png", 512)
    png(icon_light, OUT / "app-icon.png", 512)
    # アプリ本体のアイコン（Tauri）の元
    png(icon_dark, OUT / "app-icon-1024.png", 1024)
    # 5. ソーシャルプレビュー（GitHub の推奨は 1280×640）
    write("og.svg", og())
    png(OUT / "og.svg", OUT / "og.png", 1280, 640)
    shutil.copy(OUT / "og.png", ROOT / "docs" / "social-preview.png")
    print("brand/ を作りました")


def og() -> str:
    W, H = 1280, 640
    mono = "'DejaVu Sans Mono','JetBrains Mono',monospace"
    inter = "Inter,'Inter Display',sans-serif"
    d, w = text_path("hushtv", 64, 168, 222, tracking=-1.5)
    b = [f'<rect width="{W}" height="{H}" fill="{INK}"/>']
    b.append(f'<radialGradient id="glow" cx="0.82" cy="0.3" r="0.7"><stop offset="0" stop-color="#123C46" stop-opacity="0.8"/><stop offset="1" stop-color="{INK}" stop-opacity="0"/></radialGradient><rect width="{W}" height="{H}" fill="url(#glow)"/>')
    b.append(mark(80, 160, 76, WHITE, MINT))
    b.append(f'<path fill="{WHITE}" d="{d}"/>')
    b.append(f'<text x="80" y="300" font-family="{inter}" font-size="15" font-weight="700" letter-spacing="3" fill="{MINT}">ANDROID TV · GOOGLE TV · 456 TESTED MODELS</text>')
    b.append(f'<text x="80" y="370" font-family="{inter}" font-size="54" font-weight="700" fill="{WHITE}">Quiet your</text>')
    b.append(f'<text x="80" y="432" font-family="{inter}" font-size="54" font-weight="700" fill="{WHITE}">Android TV<tspan fill="{MINT}">.</tspan></text>')
    b.append(f'<text x="80" y="480" font-family="{inter}" font-size="22" fill="#9AA6C4">Switch off ads and trackers over Wi-Fi.</text>')
    b.append(f'<text x="80" y="510" font-family="{inter}" font-size="22" fill="#9AA6C4">Nothing uninstalled. Every change can be undone.</text>')
    b.append(f'<text x="80" y="566" font-family="{mono}" font-size="19" fill="#6B7699">github.com/kajisho5/hushtv</text>')
    # 右：端末の様子
    x0, y0, pw, ph = 690, 128, 510, 384
    b.append(f'<rect x="{x0}" y="{y0}" width="{pw}" height="{ph}" rx="16" fill="#111832" stroke="#263152"/>')
    for i, c in enumerate(["#F87171", "#FBBF24", "#4B5563"]):
        b.append(f'<circle cx="{x0 + 22 + i * 16}" cy="{y0 + 22}" r="5" fill="{c}"/>')
    b.append(f'<text x="{x0 + pw - 18}" y="{y0 + 26}" font-family="{mono}" font-size="11" fill="#6B7699" text-anchor="end">one Rust binary · no adb</text>')
    b.append(f'<line x1="{x0}" y1="{y0 + 42}" x2="{x0 + pw}" y2="{y0 + 42}" stroke="#263152"/>')
    b.append(f'<text x="{x0 + 22}" y="{y0 + 78}" font-family="{mono}" font-size="16" fill="{WHITE}"><tspan fill="{MINT}">$</tspan> hushtv apply 192.168.1.50</text>')
    rows = [
        ("closest list  acme-55u8", "92%", "#9AA6C4"),
        ("safety checks  Home · Wi-Fi · HDMI", "6 kept", "#FBBF24"),
        ("restore point saved", "undo", "#9AA6C4"),
        ("safe mode  batch 5/5", "PASS", MINT),
        ("before / after image", "PNG", "#9AA6C4"),
    ]
    y = y0 + 124
    for left, right, col in rows:
        b.append(f'<circle cx="{x0 + 27}" cy="{y - 5}" r="3" fill="#4B5563"/>')
        b.append(f'<text x="{x0 + 42}" y="{y}" font-family="{mono}" font-size="14" fill="#C9D2E8">{left}</text>')
        if right == "PASS":
            b.append(f'<rect x="{x0 + pw - 84}" y="{y - 18}" width="62" height="24" rx="12" fill="#0F3A3A"/><text x="{x0 + pw - 53}" y="{y - 1}" font-family="{inter}" font-size="12" font-weight="700" fill="{MINT}" text-anchor="middle">PASS</text>')
        else:
            b.append(f'<text x="{x0 + pw - 22}" y="{y}" font-family="{inter}" font-size="12" fill="{col}" text-anchor="end">{right}</text>')
        y += 38
    b.append(f'<line x1="{x0 + 22}" y1="{y - 14}" x2="{x0 + pw - 22}" y2="{y - 14}" stroke="#263152" stroke-dasharray="3 4"/>')
    b.append(f'<circle cx="{x0 + 30}" cy="{y + 14}" r="9" fill="#0F3A3A"/><circle cx="{x0 + 30}" cy="{y + 14}" r="5" fill="{MINT}"/>')
    b.append(f'<text x="{x0 + 50}" y="{y + 20}" font-family="{mono}" font-size="16" font-weight="700" fill="{MINT}">20 apps switched off · 0 problems</text>')
    return svg(W, H, "".join(b), "hushtv: quiet your Android TV")


if __name__ == "__main__":
    main()
