# hushtv

**Quiet your Android TV.** Disable the ads, trackers and promo rows that ship on Android TV and Google TV, over Wi-Fi, from one small file. Nothing is uninstalled, and every change can be undone.

[日本語](#日本語) · [Design](docs/DESIGN.md)

> **Status: v0.1.0-beta.3 (command line only).** The safety checks are ported from cleanyourtv and covered by tests, but this release **has not been tried on a real TV yet**. Read `plan` before you `apply`, and keep the restore record. A desktop app with a GUI comes next.

## Why

[cleanyourtv](https://github.com/Gathin23/cleanyourtv) has the knowledge: tested lists for **456 TV models from 122 brands** and the safety checks that keep a TV bootable. Using it needs Python, a JDK and the Android SDK. [CleanMyTV](https://github.com/wingedpanther/cleanmytv) made it a friendly desktop app, but it is Electron and downloads Google's `adb` for pairing.

hushtv is one Rust binary (about 7 MB) with every list inside:

- **No Python, no Java, no `adb`.** It speaks ADB itself ([adb_client](https://github.com/cocool97/adb_client)).
- **The same safety checks, in the same order**, as cleanyourtv's `disable.sh`: core Android packages, the setup wizard that owns your Wi-Fi, the app that owns Home, the HDMI input drivers, and the apps that open inputs and Settings.
- **The same restore record** (`~/.cleanyourtv/recovery-*.json`, version 1), so cleanyourtv's `restore.sh` can read what hushtv saved.
- **Light or Full.** Light (the default) switches off only ads and tracking. Full applies the whole tested list.
- **Safe mode, on by default.** Apps are switched off 4 at a time; after each batch hushtv checks that the TV still answers, Home still opens and Settings still opens. If not, that batch is undone and the run stops.
- **A before/after image to share.** hushtv photographs the home screen before and after and makes a 1600×900 card.
- **`rescue` for TVs another tool broke.** It finds core packages, a missing Home, Settings that won't open, input drivers, the setup wizard and apps uninstalled for the user, and only turns things back on. It also replays cleanyourtv and CleanMyTV restore records. See [docs/RESCUE.md](docs/RESCUE.md).
- **`check --fix` after a firmware update**: finds apps that came back, new preloads and a reset Home.
- macOS, Windows and Linux. English and Japanese.

## Use

1. On the TV: Settings → Device Preferences → About → press **Build** 7 times. Then Developer options → turn on **USB debugging** and **Network debugging** (or Wireless debugging).
2. Download the file for your computer from [Releases](https://github.com/kajisho5/hushtv/releases).

```sh
hushtv scan                      # find TVs on your network
hushtv info 192.168.1.50         # model, Android version, the closest lists
hushtv plan 192.168.1.50         # what would be disabled, and what is protected (changes nothing)
hushtv apply 192.168.1.50        # save the restore record, then disable (Light, safe mode)
hushtv apply 192.168.1.50 --preset full   # the whole tested list
hushtv check 192.168.1.50 --fix  # after a firmware update
hushtv restore 192.168.1.50      # put everything back
hushtv rescue 192.168.1.50       # repair a TV another tool broke (only turns things back on)
```

The first time, the TV asks "Allow USB debugging?". Choose **Allow**. Pick a list with `--list <folder>` (see `hushtv lists sony`) or your own file with `--file`.

When you are done, turn debugging off on the TV. While it is on, any device on your network may be able to control the TV.

## Not yet

- Pairing with a 6-digit code (Wireless debugging) without Google's `adb`. In v0.1 you can use Network debugging on port 5555.
- Installing the ad-free home screen, and the GUI.
- TVs without ADB (Samsung Tizen, LG webOS, Roku).

## Credits and license

MIT. The per-model lists and the safety rules come from [cleanyourtv](https://github.com/Gathin23/cleanyourtv) by Gathin (MIT); the lists are refreshed every week. The desktop flow is inspired by [CleanMyTV](https://github.com/wingedpanther/cleanmytv). See [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md).

---

## 日本語

Android TV / Google TV に最初から入っている広告・追跡・おすすめ欄のアプリを、パソコンから Wi-Fi 経由で止めます。アプリは削除しません。全部元に戻せます。

> **今の状態：v0.1.0-beta.3（コマンド操作のみ）。** 安全装置は cleanyourtv から移してテストで確かめていますが、**実機のテレビではまだ試していません**。`apply` の前に必ず `plan` で中身を見て、戻す記録を残してください。画面のあるパソコン用アプリは次のバージョンで出します。

- Python・Java・Google の adb は要りません。約 7MB のファイル 1 つに、456 機種のリストが入っています
- cleanyourtv と同じ安全装置を同じ順番で通します（Android の中核、Wi-Fi の持ち主の初期設定画面、今のホーム画面、HDMI の入力の土台、入力切替と設定を開くアプリ）
- 戻す記録は cleanyourtv と同じ形式です。cleanyourtv の `restore.sh` でも戻せます
- 「軽め」（既定）は広告と追跡だけ、「全部」はリストのすべてを止めます
- 安全モード（既定でオン）：4 個ずつ止め、そのたびにテレビが答えるか・ホームと設定が開くかを確かめ、だめならその組だけ戻して止まります
- 止める前と後のホーム画面を撮って、共有用の画像（1600×900）を作ります
- `rescue`：ほかのツールで止めすぎて調子が悪くなったテレビを診断して直します（戻すだけ）。cleanyourtv・CleanMyTV の戻す記録も使えます（[docs/RESCUE.md](docs/RESCUE.md)）
- ファーム更新のあとは `check --fix` で、戻ってきたアプリ・新しく増えたアプリ・元に戻ったホームを見つけて直します

### 使い方

1. テレビで：設定 → デバイス設定 → 端末情報 →［ビルド］を 7 回押す → 開発者向けオプション →［USB デバッグ］と［ネットワークデバッグ］をオン
2. [Releases](https://github.com/kajisho5/hushtv/releases) から、自分のパソコン用のファイルをダウンロード
3. 上のコマンドを順に実行（初回はテレビに確認が出るので［許可］）

終わったら、テレビの設定でデバッグをオフにしてください。オンの間は、同じネットワークの機器からテレビを操作できます。
