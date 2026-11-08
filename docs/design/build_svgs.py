#!/usr/bin/env python3
"""hushtv の画面案（SVG）を作る。データはすべて架空。"""
from pathlib import Path
from html import escape

W, H = 1280, 800
C = dict(bg="#0B1020", side="#0F1629", panel="#141C33", panel2="#1A2340", border="#263152",
         text="#E8ECF6", sub="#9AA6C4", dim="#6B7699", mint="#5EEAD4", mintbg="#0F3A3A",
         danger="#F87171", dangerbg="#3A1A22", warn="#FBBF24", warnbg="#3A2E12",
         ok="#4ADE80", okbg="#123322", ink="#06201D")
JP = "'Noto Sans JP','Noto Sans CJK JP','Hiragino Sans','Yu Gothic UI',sans-serif"
MONO = "'JetBrains Mono','SFMono-Regular',Consolas,monospace"
OUT = Path(__file__).parent / "svg"


LANG = "ja"
EN = {}
MISSING = set()


def tr(s):
    if LANG != "en" or not any(ord(c) > 0x2E7F for c in s):
        return s
    if s not in EN:
        MISSING.add(s)
    return EN.get(s, s)


def t(x, y, s, size=13, fill=C["text"], weight=400, anchor="start", mono=False, op=1):
    fam = MONO if mono else JP
    s = tr(s)
    return (f'<text x="{x}" y="{y}" font-family="{fam}" font-size="{size}" font-weight="{weight}" '
            f'fill="{fill}" text-anchor="{anchor}" opacity="{op}">{escape(s)}</text>')


def r(x, y, w, h, fill, rx=10, stroke=None, sw=1, op=1):
    st = f' stroke="{stroke}" stroke-width="{sw}"' if stroke else ""
    return f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{rx}" fill="{fill}"{st} opacity="{op}"/>'


def chip(x, y, label, fg, bg, size=11):
    label = tr(label)
    w = text_w(label, size) + 18
    return r(x, y, w, 22, bg, rx=11) + t(x + w / 2, y + 15, label, size, fg, 600, "middle"), w


def text_w(s, size):
    s = tr(s)
    return sum(size if ord(ch) > 0x2E7F else size * 0.6 for ch in s)


def button(x, y, w, label, kind="primary", h=44, size=14):
    if kind == "primary":
        return r(x, y, w, h, C["mint"], rx=10) + t(x + w / 2, y + h / 2 + 5, label, size, C["ink"], 700, "middle")
    if kind == "danger":
        return r(x, y, w, h, "none", rx=10, stroke=C["danger"], sw=1.5) + t(x + w / 2, y + h / 2 + 5, label, size, C["danger"], 600, "middle")
    if kind == "disabled":
        return r(x, y, w, h, "none", rx=10, stroke=C["border"], sw=1.5) + t(x + w / 2, y + h / 2 + 5, label, size, C["dim"], 600, "middle")
    return r(x, y, w, h, "none", rx=10, stroke=C["sub"], sw=1.2) + t(x + w / 2, y + h / 2 + 5, label, size, C["text"], 600, "middle")


def check(x, y, on=True, locked=False):
    if locked:
        return (r(x, y, 18, 18, C["warnbg"], rx=4, stroke=C["warn"]) +
                f'<path d="M{x+5} {y+9}v-2a4 4 0 0 1 8 0v2" stroke="{C["warn"]}" stroke-width="1.6" fill="none"/>'
                + r(x + 4, y + 9, 10, 6, C["warn"], rx=1.5))
    if on:
        return (r(x, y, 18, 18, C["mint"], rx=4) +
                f'<path d="M{x+4} {y+9.5}l3.5 3.5 6.5-7" stroke="{C["ink"]}" stroke-width="2.2" fill="none" stroke-linecap="round" stroke-linejoin="round"/>')
    return r(x, y, 18, 18, "none", rx=4, stroke=C["dim"], sw=1.5)


def tv_icon(x, y, s=1.0, col=None):
    col = col or C["mint"]
    return (f'<g transform="translate({x},{y}) scale({s})">'
            f'<rect x="0" y="0" width="28" height="19" rx="4" fill="none" stroke="{col}" stroke-width="2.4"/>'
            f'<path d="M9 24h10" stroke="{col}" stroke-width="2.4" stroke-linecap="round"/>'
            f'<path d="M8 10.5c2-2.5 4-2.5 6 0s4 2.5 6 0" stroke="{col}" stroke-width="2" fill="none" stroke-linecap="round"/></g>')


STEPS = [("つなぐ", "テレビを見つけて接続"), ("片付ける", "不要なアプリを止める"),
         ("ホーム画面", "広告のないホームに"), ("点検", "更新後のずれを直す"), ("終わる", "デバッグを閉じる")]


STEPS_FULL = STEPS
FLEET = None  # 何台もまとめて のボタンを出すか（1.0 の画面案）
UPDATE = None  # 更新のお知らせ（例 "0.3.0"）。None なら出さない
VERSION = "hushtv v0.1.0 · MIT"


def sidebar(active, done):
    o = [r(0, 0, 260, H, C["side"], rx=0), f'<line x1="260" y1="0" x2="260" y2="{H}" stroke="{C["border"]}"/>']
    o.append(r(24, 26, 40, 40, C["mintbg"], rx=10) + tv_icon(30, 34, 1.0))
    o.append(t(76, 45, "hushtv", 20, C["text"], 700))
    o.append(t(76, 62, "テレビを静かにする", 11, C["sub"]))
    # 接続中のテレビ
    connected = active != 0
    o.append(r(20, 92, 220, 52, C["panel"], rx=12, stroke=C["border"]))
    o.append(f'<circle cx="38" cy="118" r="5" fill="{C["ok"] if connected else C["dim"]}"/>')
    o.append(t(52, 114, "リビングのテレビ" if connected else "未接続", 13, C["text"], 600))
    o.append(t(52, 132, "192.168.10.23 · 暗号化あり" if connected else "テレビを選んでください", 11, C["sub"], mono=connected))
    y = 168
    for i, (title, sub) in enumerate(STEPS):
        if i == active:
            o.append(r(16, y, 228, 60, C["panel2"], rx=10))
            o.append(r(16, y + 10, 4, 40, C["mint"], rx=2))
        cx, cy = 46, y + 30
        if i in done:
            o.append(f'<circle cx="{cx}" cy="{cy}" r="13" fill="{C["mint"]}"/>'
                     f'<path d="M{cx-6} {cy}l4 4 8-8" stroke="{C["ink"]}" stroke-width="2.2" fill="none" stroke-linecap="round" stroke-linejoin="round"/>')
        else:
            col = C["mint"] if i == active else C["dim"]
            o.append(f'<circle cx="{cx}" cy="{cy}" r="13" fill="none" stroke="{col}" stroke-width="1.6"/>')
            o.append(t(cx, cy + 5, str(i + 1), 13, col, 700, "middle"))
        o.append(t(72, y + 26, title, 15, C["text"] if i == active or i in done else C["sub"], 700 if i == active else 500))
        o.append(t(72, y + 44, sub, 11, C["sub"] if i == active else C["dim"]))
        y += 68
    # 更新のお知らせ
    if UPDATE:
        uy = H - 290
        o.append(r(16, uy, 228, 64, C["mintbg"], rx=10, stroke=C["mint"]))
        o.append(f'<circle cx="34" cy="{uy+22}" r="9" fill="{C["mint"]}"/>'
                 f'<path d="M34 {uy+17}v9M30 {uy+22}l4 4 4-4" stroke="{C["ink"]}" stroke-width="1.8" fill="none" stroke-linecap="round" stroke-linejoin="round"/>')
        o.append(t(52, uy + 26, "新しいバージョン " + UPDATE, 13, C["text"], 700))
        o.append(t(52, uy + 46, "更新する →", 12, C["mint"], 700))
    # 何台もまとめて（1.0 から）
    if FLEET is not None:
        on = active == -2
        fy = H - 264
        o.append(r(16, fy, 228, 44, C["panel2"] if on else C["side"], rx=10, stroke=C["mint"] if on else C["border"]))
        for k in range(3):
            o.append(f'<rect x="{30 + k*7}" y="{fy+14+k*3}" width="12" height="9" rx="2" fill="none" stroke="{C["mint"]}" stroke-width="1.4"/>')
        o.append(t(64, fy + 28, "何台もまとめて", 13, C["text"] if on else C["sub"], 700))
    # テレビを直す
    rescue_on = active == -1
    o.append(r(16, H - 214, 228, 44, C["panel2"] if rescue_on else C["side"], rx=10, stroke=C["warn"] if rescue_on else C["border"]))
    o.append(f'<path d="M34 {H-200}l8 16h-16z" fill="none" stroke="{C["warn"]}" stroke-width="1.8" stroke-linejoin="round"/>')
    o.append(t(56, H - 186, "テレビを直す", 13, C["text"] if rescue_on else C["sub"], 700))
    # 言語の切り替え
    o.append(r(24, H - 158, 212, 32, C["panel"], rx=8, stroke=C["border"]))
    on_en = LANG == "en"
    o.append(r(27 if on_en else 131, H - 155, 102, 26, C["panel2"], rx=6))
    o.append(f'<text x="78" y="{H-137}" font-family="{JP}" font-size="12" font-weight="{700 if on_en else 400}" fill="{C["text"] if on_en else C["dim"]}" text-anchor="middle">English</text>')
    o.append(f'<text x="182" y="{H-137}" font-family="{JP}" font-size="12" font-weight="{400 if on_en else 700}" fill="{C["dim"] if on_en else C["text"]}" text-anchor="middle">日本語</text>')
    o.append(f'<line x1="24" y1="{H-110}" x2="236" y2="{H-110}" stroke="{C["border"]}"/>')
    o.append(t(24, H - 84, "アプリは削除しません。", 12, C["sub"]))
    o.append(t(24, H - 64, "いつでも元に戻せます。", 12, C["sub"]))
    o.append(t(24, H - 32, VERSION, 11, C["dim"], mono=True))
    return "".join(o)


def header(title, desc, desc2=None):
    o = t(300, 70, title, 26, C["text"], 700) + t(300, 98, desc, 13, C["sub"])
    if desc2:
        o += t(300, 118, desc2, 13, C["sub"])
    return o


def page(name, active, done, body):
    svg = (f'<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H}" viewBox="0 0 {W} {H}">'
           f'<rect width="{W}" height="{H}" fill="{C["bg"]}"/>{sidebar(active, done)}{body}</svg>')
    OUT.mkdir(exist_ok=True)
    (OUT / f"{name}{'' if LANG == 'ja' else '-' + LANG}.svg").write_text(svg, encoding="utf-8")


# ---------- 1. つなぐ ----------
def screen_connect():
    o = [header("テレビにつなぐ", "同じ Wi-Fi にあるテレビを探しています。見つからないときは IP アドレスを入れてください。")]
    # 見つかったテレビ
    o.append(r(300, 136, 560, 420, C["panel"], rx=14, stroke=C["border"]))
    o.append(t(324, 168, "見つかったテレビ", 16, C["text"], 700))
    o.append(t(334 + text_w("見つかったテレビ", 16), 168, "3 台", 13, C["sub"]))
    o.append(button(748, 148, 92, "再検索", "secondary", h=32, size=12))
    rows = [("リビングのテレビ", "ACME 55U8 · Android TV 12", "192.168.10.23", "ワイヤレスデバッグ", C["mint"], C["mintbg"], "ペアリングが必要", C["warn"], True),
            ("寝室のテレビ", "Nimbus TV Box · Android TV 11", "192.168.10.41", "ポート 5555", C["sub"], C["panel2"], "つなげます", C["ok"], False),
            ("書斎のプロジェクター", "Lumen P2 · Google TV", "192.168.10.57", "デバッグ オフ", C["dim"], C["panel2"], "テレビ側の設定が必要", C["dim"], False)]
    y = 190
    for name, model, ip, tag, tf, tb, st, sc, sel in rows:
        o.append(r(316, y, 528, 76, C["panel2"] if sel else C["panel"], rx=12, stroke=C["mint"] if sel else C["border"], sw=1.6 if sel else 1))
        o.append(tv_icon(334, y + 24, 1.0, C["mint"] if sel else C["sub"]))
        o.append(t(380, y + 31, name, 15, C["text"], 700))
        o.append(t(380, y + 52, model, 12, C["sub"]))
        o.append(t(380, y + 68, ip, 11, C["dim"], mono=True))
        c, w = chip(824 - text_w(tag, 11) - 18, y + 14, tag, tf, tb)
        o.append(c)
        o.append(f'<circle cx="{824 - text_w(st, 12) - 12}" cy="{y + 55}" r="4" fill="{sc}"/>')
        o.append(t(824, y + 59, st, 12, sc, 600, "end"))
        y += 88
    # 手入力
    o.append(f'<line x1="316" y1="462" x2="844" y2="462" stroke="{C["border"]}"/>')
    o.append(t(324, 490, "見つからないとき", 13, C["sub"], 600))
    o.append(r(324, 502, 360, 38, C["bg"], rx=8, stroke=C["border"]))
    o.append(t(338, 526, "例 192.168.10.80:5555", 13, C["dim"], mono=True))
    o.append(button(696, 502, 144, "この IP でつなぐ", "secondary", h=38, size=13))
    # ペアリング
    o.append(r(880, 136, 360, 420, C["panel"], rx=14, stroke=C["border"]))
    o.append(t(904, 168, "ペアリング", 16, C["text"], 700))
    o.append(t(904, 192, "リビングのテレビ", 12, C["mint"], 600))
    o.append(t(904, 220, "テレビの［ワイヤレスデバッグ］→", 12, C["sub"]))
    o.append(t(904, 238, "［ペア設定コードによるデバイスの", 12, C["sub"]))
    o.append(t(904, 256, "ペア設定］に出る 6 桁を入れてください。", 12, C["sub"]))
    digits = ["4", "8", "2", "", "", ""]
    for i, d in enumerate(digits):
        x = 904 + i * 52
        act = i == 3
        o.append(r(x, 276, 44, 56, C["bg"], rx=10, stroke=C["mint"] if act else C["border"], sw=2 if act else 1))
        if d:
            o.append(t(x + 22, 313, d, 24, C["text"], 700, "middle", mono=True))
        elif act:
            o.append(f'<line x1="{x+22}" y1="292" x2="{x+22}" y2="316" stroke="{C["mint"]}" stroke-width="2"/>')
    o.append(t(904, 362, "ペア設定用のポート", 12, C["sub"]))
    o.append(r(904, 372, 312, 36, C["bg"], rx=8, stroke=C["border"]))
    o.append(t(918, 395, "37145", 14, C["text"], mono=True))
    o.append(t(1202, 395, "自動で入力", 11, C["dim"], anchor="end"))
    o.append(button(904, 424, 312, "ペアリングしてつなぐ"))
    o.append(t(904, 494, "Google の adb は使いません。", 12, C["sub"]))
    o.append(t(904, 512, "鍵はこのパソコンにだけ保存されます。", 12, C["sub"]))
    o.append(t(904, 538, "QR コードで読み取る →", 12, C["mint"], 600))
    # テレビ側でやること
    o.append(r(300, 576, 940, 184, C["panel"], rx=14, stroke=C["border"]))
    o.append(t(324, 608, "テレビ側でやること（最初の 1 回だけ）", 15, C["text"], 700))
    steps = [("1", "開発者向けオプションを出す", "設定 → デバイス設定 → 端末情報", "→［ビルド］を 7 回押す"),
             ("2", "デバッグをオンにする", "開発者向けオプション →", "［ワイヤレスデバッグ］をオン"),
             ("3", "このパソコンを許可する", "テレビに確認が出たら", "［許可］を選ぶ")]
    x = 324
    for n, a, b, c in steps:
        o.append(f'<circle cx="{x+14}" cy="640" r="14" fill="{C["mintbg"]}"/>' + t(x + 14, 645, n, 13, C["mint"], 700, "middle"))
        o.append(t(x + 38, 645, a, 14, C["text"], 600))
        o.append(t(x + 38, 672, b, 12, C["sub"]))
        o.append(t(x + 38, 690, c, 12, C["sub"]))
        x += 306
    o.append(r(324, 708, 892, 36, C["warnbg"], rx=8))
    o.append(t(340, 731, "注意：デバッグをオンにしている間は、同じ Wi-Fi の機器からテレビを操作できます。終わったら最後の画面で閉じます。", 12, C["warn"]))
    page("1-connect", 0, set(), "".join(o))


# ---------- 2. 片付ける ----------
def screen_clean():
    o = [header("不要なアプリを止める", "ACME 55U8（Android TV 12）に一番近いリスト「acme-55u8」を使います。",
                "アプリは削除せず、止めるだけです。")]
    # 止め方（軽め／全部）
    o.append(r(870, 44, 370, 40, C["panel"], rx=10, stroke=C["border"]))
    o.append(r(874, 48, 210, 32, C["mintbg"], rx=8, stroke=C["mint"]))
    o.append(t(979, 69, "軽め：広告と追跡だけ", 13, C["mint"], 700, "middle"))
    o.append(t(1160, 69, "全部", 13, C["sub"], 600, "middle"))
    o.append(t(1240, 112, "リストを変える →", 13, C["mint"], 600, "end"))
    stats = [("20", "止める", C["mint"]), ("3", "元から止まっている", C["sub"]),
             ("6", "守る（止めると危険）", C["warn"]), ("5", "このテレビにない", C["dim"])]
    x = 300
    for n, lab, col in stats:
        o.append(r(x, 136, 226, 80, C["panel"], rx=12, stroke=C["border"]))
        o.append(t(x + 20, 178, n, 30, col, 700, mono=True))
        o.append(t(x + 20, 202, lab, 12, C["sub"]))
        x += 238
    o.append(r(300, 230, 940, 44, C["mintbg"], rx=10))
    o.append(f'<path d="M322 242l8-3 8 3v6c0 5-3.5 8-8 9.5-4.5-1.5-8-4.5-8-9.5z" fill="none" stroke="{C["mint"]}" stroke-width="1.8"/>')
    o.append(t(348, 257, "戻す地点はまだありません。［反映］を押すと、変える前のテレビの状態を自動で保存します。", 13, C["text"]))
    # タブ
    tabs = [("すべて 38", True), ("止める 20", False), ("守る 6", False), ("元から止まっている 3", False), ("ない 5", False)]
    x = 300
    for lab, on in tabs:
        w = text_w(lab, 13) + 28
        o.append(r(x, 290, w, 34, C["text"] if on else C["panel"], rx=8, stroke=None if on else C["border"]))
        o.append(t(x + w / 2, 312, lab, 13, C["bg"] if on else C["sub"], 600, "middle"))
        x += w + 8
    o.append(r(1010, 290, 230, 34, C["panel"], rx=8, stroke=C["border"]))
    o.append(f'<circle cx="1029" cy="305" r="6" fill="none" stroke="{C["dim"]}" stroke-width="1.6"/><path d="M1033.5 309.5l4 4" stroke="{C["dim"]}" stroke-width="1.6"/>')
    o.append(t(1046, 312, "アプリ名で絞り込み", 12, C["dim"]))
    # 表
    o.append(r(300, 338, 940, 368, C["panel"], rx=12, stroke=C["border"]))
    for lab, x in (("アプリ", 352), ("種類", 690), ("テレビ上", 800), ("理由", 900)):
        o.append(t(x, 364, lab, 11, C["dim"], 600))
    o.append(f'<line x1="300" y1="376" x2="1240" y2="376" stroke="{C["border"]}"/>')
    rows = [("on", "com.example.adservice", "広告", C["danger"], C["dangerbg"], "動作中", "広告の配信", "41 機種で停止"),
            ("on", "com.example.telemetry", "追跡", C["danger"], C["dangerbg"], "動作中", "利用状況の送信", "33 機種で停止"),
            ("off", "com.example.tv.recommend", "おすすめ欄", C["mint"], C["mintbg"], "動作中", "ホームのおすすめ行", "「全部」で止める（チェックで追加）"),
            ("off", "com.example.storedemo", "店頭デモ", C["sub"], C["panel2"], "動作中", "店頭用の宣伝画面", "「全部」で止める（チェックで追加）"),
            ("off", "com.example.voiceassist", "音声", C["sub"], C["panel2"], "動作中", "リストの作者が「残す」と判断", "残す"),
            ("lock", "com.example.tvinput", "守る", C["warn"], C["warnbg"], "動作中", "止めると入力切替が使えなくなる", "安全装置"),
            ("lock", "com.android.systemui", "守る", C["warn"], C["warnbg"], "動作中", "Android の中核。止めると起動しない", "安全装置")]
    y = 376
    for i, (st, pkg, kind, kf, kb, on_tv, why, why2) in enumerate(rows):
        if st == "lock":
            o.append(f'<rect x="301" y="{y}" width="938" height="44" fill="{C["warnbg"]}" opacity="0.35"/>')
        o.append(check(320, y + 13, on=st == "on", locked=st == "lock"))
        o.append(t(352, y + 27, pkg, 13, C["text"] if st != "lock" else C["sub"], 500, mono=True))
        c, _ = chip(690, y + 11, kind, kf, kb)
        o.append(c)
        o.append(f'<circle cx="806" cy="{y+22}" r="4" fill="{C["ok"]}"/>' + t(816, y + 26, on_tv, 12, C["sub"]))
        o.append(t(900, y + 21, why, 12, C["text"] if st != "lock" else C["warn"]))
        o.append(t(900, y + 36, why2, 11, C["dim"]))
        if i < len(rows) - 1:
            o.append(f'<line x1="316" y1="{y+44}" x2="1224" y2="{y+44}" stroke="{C["border"]}" opacity="0.6"/>')
        y += 44
    o.append(f'<line x1="300" y1="{y}" x2="1240" y2="{y}" stroke="{C["border"]}"/>')
    o.append(t(770, y + 17, "ほか 31 件を表示 ↓", 12, C["sub"], 600, "middle"))
    # 下のバー
    o.append(f'<line x1="260" y1="716" x2="{W}" y2="716" stroke="{C["border"]}"/>')
    o.append(t(300, 754, "▲ 実行ログ", 13, C["sub"], 600))
    o.append(button(430, 732, 300, "戻す地点を保存して反映（20 件）"))
    o.append(button(744, 732, 110, "元に戻す", "disabled"))
    o.append(r(874, 742, 40, 22, C["mint"], rx=11) + f'<circle cx="903" cy="753" r="8" fill="{C["ink"]}"/>')
    o.append(t(922, 750, "安全モード", 12, C["text"], 700))
    o.append(t(922, 766, "4 個ずつ止めて確かめる", 11, C["sub"]))
    o.append(t(1240, 759, "次へ：ホーム画面 →", 13, C["mint"], 600, "end"))
    page("2-clean", 1, {0}, "".join(o))


# ---------- 3. ホーム画面 ----------
def screen_home():
    o = [header("広告のないホーム画面にする", "cleanyourtv のホーム画面アプリ（約 572KB）を入れて、ホームボタンで開くようにします。",
                "うまく切り替わらないときは、自動で元のホームに戻します。")]
    # テレビの見本
    o.append('<defs><linearGradient id="sky" x1="0" y1="0" x2="0" y2="1">'
             '<stop offset="0" stop-color="#1B2B4A"/><stop offset="1" stop-color="#0A1222"/></linearGradient></defs>')
    o.append(r(300, 140, 600, 338, "#05070D", rx=16, stroke=C["border"]))
    o.append(r(312, 152, 576, 314, "url(#sky)", rx=8))
    o.append(t(336, 200, "こんばんは", 20, C["text"], 600))
    o.append(t(864, 196, "21:08", 30, C["text"], 300, "end", mono=True))
    o.append(t(864, 216, "11月8日（日）", 11, C["sub"], anchor="end"))
    tiles = [("動画 A", "#2E4A7A"), ("動画 B", "#5A3270"), ("音楽", "#2F6B57"), ("写真", "#7A5A2E"), ("入力切替", "#33415E")]
    x = 336
    for i, (lab, col) in enumerate(tiles):
        sel = i == 0
        o.append(r(x, 330, 96, 62, col, rx=8, stroke=C["mint"] if sel else None, sw=2.5))
        o.append(t(x + 48, 366, lab, 12, C["text"], 600, "middle"))
        x += 106
    o.append(t(336, 420, "すべてのアプリ", 11, C["sub"], 600))
    for i in range(6):
        o.append(r(336 + i * 58, 430, 48, 22, "#24314F", rx=5))
    c, _ = chip(336, 214, "広告・おすすめ欄なし", C["mint"], C["mintbg"])
    o.append(c)
    o.append(t(600, 498, "見本（実際のアプリの絵はテレビで表示されます）", 11, C["dim"], anchor="middle"))
    # 状態カード
    o.append(r(930, 140, 310, 338, C["panel"], rx=14, stroke=C["border"]))
    o.append(t(954, 172, "今の状態", 16, C["text"], 700))
    items = [("ホーム画面アプリ", "未導入", C["sub"]), ("今のホーム", "com.example.stockhome", C["text"]),
             ("必要な Android", "8.0 以上 ✓（12）", C["ok"]), ("アプリの出どころ", "cleanyourtv（MIT）", C["text"])]
    y = 204
    for lab, val, col in items:
        o.append(t(954, y, lab, 12, C["dim"]))
        o.append(t(954, y + 20, val, 13, col, 600, mono=val.startswith("com.")))
        y += 50
    o.append(button(954, 404, 262, "導入してホームにする"))
    # 流れ
    o.append(r(300, 520, 940, 176, C["panel"], rx=14, stroke=C["border"]))
    o.append(t(324, 552, "導入の流れ", 15, C["text"], 700))
    flow = [("APK を送る", "約 572KB"), ("インストール", "既存は上書きしない"), ("ホームに設定", "元のホームは記録済み"),
            ("ホームボタンで確認", "テレビの画面を確認"), ("だめなら元に戻す", "自動で巻き戻し")]
    x = 324
    for i, (a, b) in enumerate(flow):
        col = C["warn"] if i == 4 else C["mint"]
        o.append(f'<circle cx="{x+16}" cy="600" r="16" fill="{C["warnbg"] if i == 4 else C["mintbg"]}"/>')
        o.append(t(x + 16, 605, str(i + 1), 13, col, 700, "middle"))
        if i < 4:
            o.append(f'<line x1="{x+38}" y1="600" x2="{x+170}" y2="600" stroke="{C["border"]}" stroke-width="2" stroke-dasharray="4 4"/>')
        o.append(t(x, 644, a, 13, C["text"], 600))
        o.append(t(x, 664, b, 11, C["sub"]))
        x += 182
    o.append(f'<line x1="260" y1="716" x2="{W}" y2="716" stroke="{C["border"]}"/>')
    o.append(t(300, 754, "← 片付けに戻る", 13, C["sub"], 600))
    o.append(button(470, 732, 200, "元のホームに戻す", "secondary"))
    o.append(t(690, 758, "並び替え・テーマは次のバージョンで対応", 12, C["dim"]))
    o.append(t(1240, 759, "次へ：点検 →", 13, C["mint"], 600, "end"))
    page("3-home", 2, {0, 1}, "".join(o))


# ---------- 4. 点検 ----------
def screen_check():
    o = [header("ファーム更新のあとの点検", "前回の片付け（2026-10-30）と、今のテレビを比べました。")]
    o.append(r(300, 126, 940, 64, C["warnbg"], rx=12))
    o.append(f'<path d="M326 168l11-20 11 20z" fill="none" stroke="{C["warn"]}" stroke-width="2" stroke-linejoin="round"/>'
             f'<path d="M337 156v6" stroke="{C["warn"]}" stroke-width="2" stroke-linecap="round"/><circle cx="337" cy="165" r="1.3" fill="{C["warn"]}"/>')
    o.append(t(362, 156, "4 件のずれが見つかりました", 16, C["warn"], 700))
    o.append(t(362, 177, "ファームの更新で、止めていたアプリが戻り、ホームも元に戻っていました。", 12, C["text"]))
    o.append(button(1004, 136, 220, "まとめて直す（3 件）", h=44, size=14))
    cols = [("戻ってきたアプリ", "2", [("com.example.adservice", "更新で再び動作中"), ("com.example.tv.recommend", "更新で再び動作中")], C["danger"]),
            ("新しく増えたアプリ", "1", [("com.example.newpromo", "どのリストにも載っていない")], C["warn"]),
            ("ホーム画面", "1", [("com.example.stockhome", "元のホームに戻っていた")], C["warn"])]
    x = 300
    for title, n, items, col in cols:
        o.append(r(x, 206, 304, 272, C["panel"], rx=14, stroke=C["border"]))
        o.append(t(x + 22, 240, title, 15, C["text"], 700))
        o.append(t(x + 282, 240, n, 18, col, 700, "end", mono=True))
        y = 262
        for pkg, note in items:
            o.append(r(x + 16, y, 272, 62, C["panel2"], rx=10))
            o.append(f'<circle cx="{x+34}" cy="{y+22}" r="4" fill="{col}"/>')
            o.append(t(x + 46, y + 26, pkg, 12, C["text"], 500, mono=True))
            o.append(t(x + 46, y + 46, note, 11, C["sub"]))
            y += 72
        if title == "新しく増えたアプリ":
            o.append(t(x + 22, y + 14, "止めるか選んでください（未選択は残す）", 12, C["sub"]))
            o.append(check(x + 22, y + 30, on=False) + t(x + 50, y + 44, "止める", 13, C["text"]))
            o.append(check(x + 120, y + 30, on=True) + t(x + 148, y + 44, "このまま残す", 13, C["text"]))
        if title == "ホーム画面":
            o.append(t(x + 22, y + 14, "直すと hushtv のホームに戻します", 12, C["sub"]))
        x += 318
    # 定期点検
    o.append(r(300, 494, 460, 206, C["panel"], rx=14, stroke=C["border"]))
    o.append(t(324, 526, "定期点検", 15, C["text"], 700))
    o.append(r(680, 508, 52, 28, C["mint"], rx=14) + f'<circle cx="718" cy="522" r="11" fill="{C["ink"]}"/>')
    o.append(t(324, 556, "週 1 回、テレビにつながったときに自動で点検し、", 12, C["sub"]))
    o.append(t(324, 574, "ずれがあれば通知します。", 12, C["sub"]))
    o.append(t(324, 610, "最後の点検", 12, C["dim"]))
    o.append(t(324, 630, "2026-11-08 21:10", 13, C["text"], mono=True))
    o.append(t(324, 664, "コマンドでも同じことができます", 12, C["dim"]))
    o.append(r(324, 674, 412, 2, C["border"], rx=1, op=0))
    o.append(t(324, 690, "$ hushtv check --fix", 13, C["mint"], mono=True))
    # 終わる前に
    o.append(r(780, 494, 460, 206, C["panel"], rx=14, stroke=C["border"]))
    o.append(t(804, 526, "終わる前に", 15, C["text"], 700))
    lines = [("ok", "戻す記録を保存済み", "~/.cleanyourtv/recovery-192.168.10.23-3f9a1c07.json"),
             ("todo", "ネットワークデバッグを閉じる", "閉じたかどうか、ポートで確かめます")]
    y = 552
    for st, a, b in lines:
        if st == "ok":
            o.append(f'<circle cx="814" cy="{y+4}" r="9" fill="{C["ok"]}"/><path d="M809 {y+4}l3.5 3.5 6-6.5" stroke="{C["ink"]}" stroke-width="2" fill="none"/>')
        else:
            o.append(f'<circle cx="814" cy="{y+4}" r="9" fill="none" stroke="{C["dim"]}" stroke-width="1.6"/>')
        o.append(t(832, y + 9, a, 13, C["text"], 600))
        o.append(t(832, y + 28, b, 11, C["sub"], mono=b.startswith("~")))
        y += 50
    o.append(button(804, 650, 200, "デバッグを閉じて確認", "secondary", h=38, size=13))
    o.append(button(1016, 650, 200, "すべて元に戻す", "danger", h=38, size=13))
    o.append(f'<line x1="260" y1="716" x2="{W}" y2="716" stroke="{C["border"]}"/>')
    o.append(t(300, 754, "▲ 実行ログ", 13, C["sub"], 600))
    o.append(button(470, 732, 160, "もう一度調べる", "secondary"))
    o.append(t(1240, 759, "次へ：終わる →", 13, C["mint"], 600, "end"))
    page("4-check", 3, {0, 1, 2}, "".join(o))


def screen_finish():
    o = [header("片付けが終わりました", "安全モードで 5 組に分けて止め、毎回ホームと設定が開くことを確かめました。問題は 0 件です。")]
    # 共有画像の見本
    o.append(r(300, 128, 600, 338, "#0B1020", rx=12, stroke=C["border"]))
    o.append(t(324, 162, "hushtv", 20, C["text"], 700))
    o.append(t(398, 162, "Quiet your Android TV", 12, C["sub"]))
    o.append(r(324, 180, 270, 152, "#2A2F4A", rx=4))
    o.append(r(606, 180, 270, 152, "#1B2B4A", rx=4))
    for x in (336, 618):
        for i in range(4):
            o.append(r(x + i * 62, 290, 54, 30, "#33415E", rx=3))
    for i in range(3):
        o.append(r(336 + i * 82, 218, 74, 40, "#7A3A3A", rx=3))
        o.append(t(373 + i * 82, 243, "AD", 11, "#FFD0D0", 700, "middle"))
    o.append(r(332, 188, 64, 20, "#0B1020", rx=3) + t(364, 202, "BEFORE", 10, C["danger"], 700, "middle"))
    o.append(r(614, 188, 56, 20, "#0B1020", rx=3) + t(642, 202, "AFTER", 10, C["mint"], 700, "middle"))
    o.append(t(324, 382, "20 apps switched off", 30, C["mint"], 700))
    o.append(t(324, 410, "Ads 9  ·  Tracking 11", 14, C["text"]))
    o.append(t(324, 436, "Nothing uninstalled. Every change can be undone.", 12, C["sub"]))
    o.append(t(876, 452, "github.com/kajisho5/hushtv", 11, C["sub"], 700, "end"))
    o.append(t(600, 488, "共有用の画像（1600×900、文字は英語）", 11, C["dim"], anchor="middle"))
    # 共有ボタン
    o.append(button(300, 506, 190, "画像を保存", "primary", h=40, size=13))
    o.append(button(502, 506, 190, "画像をコピー", "secondary", h=40, size=13))
    o.append(button(704, 506, 196, "X に投稿", "secondary", h=40, size=13))
    o.append(t(300, 572, "X に投稿を押すと、文と画像を用意してブラウザで開きます。投稿するかはあなたが決めます。", 12, C["sub"]))
    # 結果
    o.append(r(300, 596, 600, 104, C["panel"], rx=14, stroke=C["border"]))
    for i, (n, lab, col) in enumerate([("20", "止めた", C["mint"]), ("6", "守った", C["warn"]), ("0", "問題", C["ok"])]):
        x = 324 + i * 196
        o.append(t(x, 646, n, 30, col, 700, mono=True))
        o.append(t(x, 676, lab, 12, C["sub"]))
    # 終わる前に
    o.append(r(930, 128, 310, 572, C["panel"], rx=14, stroke=C["border"]))
    o.append(t(954, 162, "終わる前に", 16, C["text"], 700))
    items = [("ok", "戻す記録を保存済み", "いつでも 1 回で元に戻せます"),
             ("ok", "点検用の控えを保存済み", "ファーム更新のあとに使います"),
             ("todo", "ネットワークデバッグを閉じる", "閉じたかどうか、ポートで確かめます")]
    y = 196
    for st, a, b in items:
        if st == "ok":
            o.append(f'<circle cx="964" cy="{y+4}" r="9" fill="{C["ok"]}"/><path d="M959 {y+4}l3.5 3.5 6-6.5" stroke="{C["ink"]}" stroke-width="2" fill="none"/>')
        else:
            o.append(f'<circle cx="964" cy="{y+4}" r="9" fill="none" stroke="{C["dim"]}" stroke-width="1.6"/>')
        o.append(t(982, y + 9, a, 13, C["text"], 600))
        o.append(t(982, y + 28, b, 11, C["sub"]))
        y += 58
    o.append(button(954, 382, 262, "デバッグを閉じて確認", "primary", h=40, size=13))
    o.append(t(954, 446, "テレビ側の操作", 12, C["dim"], 600))
    o.append(t(954, 470, "設定 → デバイス設定 → 開発者向け", 12, C["sub"]))
    o.append(t(954, 488, "オプション → USB デバッグをオフ", 12, C["sub"]))
    o.append(f'<line x1="954" y1="520" x2="1216" y2="520" stroke="{C["border"]}"/>')
    o.append(t(954, 552, "気に入らなかったら", 12, C["dim"], 600))
    o.append(button(954, 568, 262, "すべて元に戻す", "danger", h=40, size=13))
    o.append(t(954, 636, "複数のテレビにまとめて反映は", 11, C["dim"]))
    o.append(t(954, 654, "次のバージョンで対応します", 11, C["dim"]))
    o.append(f'<line x1="260" y1="716" x2="{W}" y2="716" stroke="{C["border"]}"/>')
    o.append(t(300, 754, "▲ 実行ログ", 13, C["sub"], 600))
    o.append(t(1240, 759, "ほかのテレビも片付ける →", 13, C["mint"], 600, "end"))
    page("5-finish", 4, {0, 1, 2, 3}, "".join(o))


def screen_rescue():
    o = [header("テレビを直す", "ほかのツール（cleanyourtv・CleanMyTV・adb）で止めすぎて調子が悪くなったテレビを診断します。", "直し方は「戻す」だけ。止めたり消したりはしません。")]
    o.append(r(300, 136, 600, 420, C["panel"], rx=14, stroke=C["border"]))
    o.append(t(324, 168, "見つかった問題", 16, C["text"], 700))
    probs = [("!!", "ホームボタンで開くホーム画面がありません", "今: 初期設定画面（真っ黒になる原因）", C["danger"], C["dangerbg"]),
             ("!!", "Android の中核が 2 個止まっています", "起動しない・再起動を繰り返す原因", C["danger"], C["dangerbg"]),
             ("!", "入力切替・チューナーまわりが 1 個止まっています", "HDMI が「信号なし」になる原因", C["warn"], C["warnbg"]),
             ("!", "初期設定画面が止まっています", "Wi-Fi が消える・キャストできない原因", C["warn"], C["warnbg"]),
             ("-", "機種別リストのアプリが 31 個止まっています", "「全部戻す」のときだけ戻します", C["sub"], C["panel2"])]
    y = 186
    for mark, a, b, col, bg in probs:
        o.append(r(316, y, 568, 62, C["panel2"], rx=10))
        o.append(r(330, y + 16, 30, 30, bg, rx=15) + t(345, y + 36, mark, 13, col, 700, "middle"))
        o.append(t(374, y + 28, a, 14, C["text"], 600))
        o.append(t(374, y + 48, b, 12, C["sub"]))
        y += 72
    # 直し方
    o.append(r(930, 136, 310, 420, C["panel"], rx=14, stroke=C["border"]))
    o.append(t(954, 168, "直し方を選ぶ", 16, C["text"], 700))
    opts = [("応急処置（おすすめ）", "起動・ホーム・設定・入力・Wi-Fi に", "かかわるものだけ戻す", True),
            ("記録どおりに戻す", "cleanyourtv・CleanMyTV の記録が", "1 件見つかりました（10/30）", False),
            ("全部戻す", "機種別リストのアプリを", "全部もう一度使えるようにする", False)]
    y = 186
    for a, b, c, on in opts:
        o.append(r(946, y, 278, 92, C["panel2"] if on else C["panel"], rx=10, stroke=C["mint"] if on else C["border"], sw=1.6 if on else 1))
        o.append(f'<circle cx="966" cy="{y+24}" r="8" fill="none" stroke="{C["mint"] if on else C["dim"]}" stroke-width="2"/>')
        if on:
            o.append(f'<circle cx="966" cy="{y+24}" r="4" fill="{C["mint"]}"/>')
        o.append(t(984, y + 29, a, 14, C["text"], 700))
        o.append(t(966, y + 56, b, 12, C["sub"]))
        o.append(t(966, y + 74, c, 12, C["sub"]))
        y += 104
    o.append(button(946, 500, 278, "直す（戻すだけ）"))
    # つながらないとき
    o.append(r(300, 576, 940, 124, C["panel"], rx=14, stroke=C["border"]))
    o.append(t(324, 606, "テレビにつながらないとき", 15, C["text"], 700))
    tips = [("Wi-Fi が消えた", "有線 LAN をつなぐか、", "設定から Wi-Fi に入り直す"),
            ("画面が真っ黒", "リモコンの設定ボタンで設定を開き、", "デバッグをオンにしてやり直す"),
            ("起動ロゴのまま", "どのツールも届きません。", "リカバリーから初期化（説明書を確認）")]
    x = 324
    for a, b, c in tips:
        o.append(t(x, 636, a, 13, C["warn"], 700))
        o.append(t(x, 658, b, 12, C["sub"]))
        o.append(t(x, 676, c, 12, C["sub"]))
        x += 306
    o.append(f'<line x1="260" y1="716" x2="{W}" y2="716" stroke="{C["border"]}"/>')
    o.append(t(300, 754, "▲ 実行ログ", 13, C["sub"], 600))
    o.append(button(470, 732, 200, "診断だけする", "secondary"))
    o.append(t(1240, 759, "直したらテレビを再起動して確かめる", 12, C["dim"], anchor="end"))
    page("6-rescue", -1, {0}, "".join(o))


# ---------- 7・8. 自動アップデート（1.0）。ホーム画面の手順は 1.1 まで隠す ----------
def with_update(fn):
    g = globals()
    g["STEPS"] = [s for s in STEPS_FULL if s[0] != "ホーム画面"]
    g["UPDATE"], g["VERSION"] = "1.0.1", "hushtv v1.0.0 · MIT"
    try:
        fn()
    finally:
        g["STEPS"], g["UPDATE"], g["VERSION"] = STEPS_FULL, None, "hushtv v0.1.0 · MIT"


def screen_update_card():
    # 片付ける画面のまま、サイドバーにお知らせだけが出る（作業の邪魔をしない）
    o = [header("テレビにつなぐ", "同じ Wi-Fi にあるテレビを探しています。見つからないときは IP アドレスを入れてください。")]
    o.append(r(300, 136, 940, 420, C["panel"], rx=14, stroke=C["border"]))
    o.append(t(324, 168, "見つかったテレビ", 16, C["text"], 700))
    y = 190
    for name, model, ip in [("リビングのテレビ", "ACME 55U8 · Android TV 12", "192.168.10.23"), ("寝室のテレビ", "Nimbus TV Box · Android TV 11", "192.168.10.41")]:
        o.append(r(316, y, 908, 76, C["panel"], rx=12, stroke=C["border"]))
        o.append(tv_icon(334, y + 24, 1.0, C["sub"]))
        o.append(t(380, y + 31, name, 15, C["text"], 700))
        o.append(t(380, y + 52, model, 12, C["sub"]))
        o.append(t(380, y + 68, ip, 11, C["dim"], mono=True))
        y += 88
    # 注記（設計の説明用。実際の画面には出ない）
    o.append(r(300, 590, 940, 170, "none", rx=14, stroke=C["dim"], sw=1))
    o.append(t(324, 620, "この案で変わるところ（説明用の注記）", 14, C["warn"], 700))
    notes = ["① 左下に「新しいバージョン」のお知らせ。起動のたびに 1 回だけ確かめ、ないときは何も出さない",
             "② 押すと、変更点と［更新して再起動］の確認が出る（次の画面）。勝手には入れ替えない",
             "③ 手順は 4 つ（ホーム画面の手順は 1.1 で戻す）。テレビの作業中は更新を押せない",
             "④ 確かめに行く先は GitHub の公開ファイルだけ。パソコンやテレビの情報は送らない"]
    for i, n in enumerate(notes):
        o.append(t(324, 650 + i * 26, n, 13, C["sub"]))
    page("7-update", 0, set(), "".join(o))


def screen_update_dialog():
    screen_update_card()
    svg_path = OUT / f"7-update{'' if LANG == 'ja' else '-' + LANG}.svg"
    base = svg_path.read_text(encoding="utf-8")
    body = base[base.index(">", base.index("<svg")) + 1: base.rindex("</svg>")]
    # 説明用の注記は消してから重ねる
    body = body[: body.index(r(300, 590, 940, 170, "none", rx=14, stroke=C["dim"], sw=1))]
    o = [f'<rect width="{W}" height="{H}" fill="#000" opacity="0.55"/>']
    x, y, w, h = 400, 190, 480, 380
    o.append(r(x, y, w, h, C["panel"], rx=16, stroke=C["border"]))
    o.append(f'<circle cx="{x+40}" cy="{y+44}" r="16" fill="{C["mintbg"]}"/>'
             f'<path d="M{x+40} {y+35}v15M{x+33} {y+44}l7 7 7-7" stroke="{C["mint"]}" stroke-width="2.2" fill="none" stroke-linecap="round" stroke-linejoin="round"/>')
    o.append(t(x + 68, y + 40, "hushtv 1.0.1 に更新", 20, C["text"], 700))
    o.append(t(x + 68, y + 60, "いまは 1.0.0 · 約 5 MB", 12, C["sub"]))
    o.append(t(x + 24, y + 100, "変わったところ", 13, C["sub"], 600))
    for i, n in enumerate(["新しい機種のリストを 12 件追加", "点検で、戻ってきたアプリの見落としを修正", "英語の言い回しを直した"]):
        o.append(f'<circle cx="{x+30}" cy="{y+124+i*26}" r="3" fill="{C["mint"]}"/>' + t(x + 42, y + 129 + i * 26, n, 13, C["text"]))
    o.append(r(x + 24, y + 214, w - 48, 52, C["panel2"], rx=10))
    o.append(t(x + 40, y + 236, "テレビの設定や戻す記録はそのまま残ります。", 12, C["sub"]))
    o.append(t(x + 40, y + 255, "更新ファイルは開発者の鍵で署名を確かめてから入れます。", 12, C["sub"]))
    o.append(button(x + 24, y + 296, 200, "あとで", "secondary"))
    o.append(button(x + w - 24 - 216, y + 296, 216, "更新して再起動"))
    svg = (f'<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H}" viewBox="0 0 {W} {H}">'
           + body + "".join(o) + "</svg>")
    (OUT / f"8-update-dialog{'' if LANG == 'ja' else '-' + LANG}.svg").write_text(svg, encoding="utf-8")


# ---------- 9. 何台もまとめて（1.0） ----------
def screen_fleet():
    g = globals()
    g["FLEET"], g["VERSION"] = True, "hushtv v1.0.0 · MIT"
    try:
        o = [header("何台もまとめて", "同じやり方を、テレビごとに一番近いリストで順番に反映します。1 台ずつ戻す記録を保存します。")]
        # 左：テレビの一覧
        o.append(r(300, 126, 560, 470, C["panel"], rx=14, stroke=C["border"]))
        o.append(t(324, 158, "テレビ", 16, C["text"], 700))
        o.append(t(394, 158, "4 台中 3 台を選択", 12, C["sub"]))
        o.append(button(748, 140, 92, "再検索", "secondary", h=32, size=12))
        rows = [("リビングのテレビ", "192.168.10.23", "ACME 55U8", True, "done", "止めた 18 · 守った 3"),
                ("寝室のテレビ", "192.168.10.41", "Nimbus TV Box", True, "run", "安全モード 3/5 組目"),
                ("会議室のテレビ", "192.168.10.62", "ACME 43U6", True, "wait", "順番待ち"),
                ("書斎のプロジェクター", "192.168.10.57", "Lumen P2", False, "off", "デバッグ オフ（選べません）")]
        y = 176
        for name, ip, model, sel, st, msg in rows:
            dim = st == "off"
            o.append(r(316, y, 528, 64, C["panel2"] if st == "run" else C["panel"], rx=12, stroke=C["mint"] if st == "run" else C["border"]))
            if dim:
                o.append(r(334, y + 23, 18, 18, "none", rx=4, stroke=C["border"]))
            else:
                o.append(check(334, y + 23, on=sel))
            o.append(t(366, y + 28, name, 14, C["dim"] if dim else C["text"], 700))
            o.append(t(366, y + 48, f"{model} · {ip}", 11, C["dim"], mono=False))
            col = {"done": C["ok"], "run": C["mint"], "wait": C["sub"], "off": C["dim"]}[st]
            if st == "done":
                o.append(f'<circle cx="{824 - text_w(msg, 12) - 14}" cy="{y+32}" r="7" fill="{C["ok"]}"/>'
                         f'<path d="M{824 - text_w(msg, 12) - 18} {y+32}l3 3 5-6" stroke="{C["ink"]}" stroke-width="1.8" fill="none" stroke-linecap="round"/>')
            elif st == "run":
                x0 = 824 - text_w(msg, 12) - 14
                o.append(f'<circle cx="{x0}" cy="{y+32}" r="7" fill="none" stroke="{C["mint"]}" stroke-width="2" stroke-dasharray="22 8"/>')
            o.append(t(824, y + 36, msg, 12, col, 600 if st in ("done", "run") else 400, "end"))
            y += 74
        o.append(f'<line x1="316" y1="478" x2="844" y2="478" stroke="{C["border"]}"/>')
        o.append(t(324, 504, "一覧にないテレビ", 13, C["sub"], 600))
        o.append(r(324, 516, 400, 38, C["bg"], rx=8, stroke=C["border"]))
        o.append(t(338, 540, "例 192.168.10.80:5555（複数はカンマ区切り）", 13, C["dim"], mono=False))
        o.append(button(736, 516, 104, "追加", "secondary", h=38, size=13))
        o.append(t(324, 580, "ワイヤレスデバッグのテレビは、先に「つなぐ」で 1 台ずつペアリングしておきます。", 12, C["dim"]))
        # 右：やること
        o.append(r(880, 126, 360, 470, C["panel"], rx=14, stroke=C["border"]))
        o.append(t(904, 158, "やること", 16, C["text"], 700))
        jobs = [("片付ける", "戻す記録を保存してから止める", True), ("点検", "ファーム更新のあとのずれを数える", False),
                ("点検して直す", "戻ってきたアプリを止め直す", False), ("元に戻す", "記録どおりに戻す", False)]
        jy = 176
        for title, sub, on in jobs:
            o.append(r(904, jy, 312, 52, C["panel2"] if on else C["panel"], rx=10, stroke=C["mint"] if on else C["border"]))
            o.append(f'<circle cx="926" cy="{jy+26}" r="8" fill="none" stroke="{C["mint"] if on else C["dim"]}" stroke-width="1.6"/>')
            if on:
                o.append(f'<circle cx="926" cy="{jy+26}" r="4" fill="{C["mint"]}"/>')
            o.append(t(944, jy + 22, title, 14, C["text"], 700))
            o.append(t(944, jy + 40, sub, 11, C["sub"]))
            jy += 60
        o.append(t(904, 436, "止め方", 12, C["sub"]))
        o.append(r(904, 446, 312, 36, C["panel"], rx=10, stroke=C["border"]))
        o.append(r(908, 450, 150, 28, C["mintbg"], rx=7, stroke=C["mint"]) + t(983, 469, "軽め（広告と追跡）", 12, C["mint"], 700, "middle"))
        o.append(t(1137, 469, "全部", 12, C["sub"], 600, "middle"))
        o.append(check(904, 498, on=True) + t(932, 512, "安全モード（4 個ずつ確かめる）", 13, C["text"]))
        o.append(button(904, 534, 312, "3 台に反映する"))
        # 下：結果のまとめ
        o.append(r(300, 612, 940, 92, C["panel"], rx=14, stroke=C["border"]))
        o.append(t(324, 644, "進み具合", 15, C["text"], 700))
        o.append(r(324, 660, 640, 10, C["panel2"], rx=5) + r(324, 660, 300, 10, C["mint"], rx=5))
        o.append(t(980, 670, "1 / 3 台 完了", 12, C["sub"]))
        o.append(t(324, 692, "つながらないテレビは飛ばして次へ進みます。終わったら台ごとの結果を表で出します。", 12, C["sub"]))
        page("9-fleet", -2, {0, 1}, "".join(o))
    finally:
        g["FLEET"], g["VERSION"] = None, "hushtv v0.1.0 · MIT"


def build_all():
    screen_connect(); screen_clean(); screen_home(); screen_check(); screen_finish(); screen_rescue()
    with_update(screen_update_card); with_update(screen_update_dialog)
    screen_fleet()


if __name__ == "__main__":
    import json, sys
    EN.update(json.loads((Path(__file__).parent / "en.json").read_text(encoding="utf-8")))
    for LANG in ("ja", "en"):
        globals()["LANG"] = LANG
        build_all()
    if MISSING:
        print("MISSING:"); print("\n".join(sorted(MISSING)))
    print("ok")
