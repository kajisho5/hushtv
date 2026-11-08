**Test release.** Download the app for your computer below (`_macos-universal.dmg` for macOS, `-setup.exe` for Windows, `.AppImage` or `.deb` for Linux), or the one-file command line (`hushtv-*`). Not yet tried on a real TV.

New in 0.2.0-beta.5:
- **Pair with the 6-digit code** (Android 11+ Wireless debugging) from the command line: `hushtv pair <address:port> <code>`. No Google adb needed. The app gets a screen for it next.
- **Homebrew and Scoop**: `brew tap kajisho5/hushtv https://github.com/kajisho5/hushtv` then `brew install kajisho5/hushtv/hushtv` (or `--cask kajisho5/hushtv/hushtv-app`); `scoop bucket add hushtv https://github.com/kajisho5/hushtv` then `scoop install hushtv/hushtv`.
- The macOS command line is now one universal file, signed and notarized.
- A form for telling us how hushtv went on your TV: https://github.com/kajisho5/hushtv/issues/new?template=tv-report.yml

The Windows installer is not signed yet: SmartScreen → More info → Run anyway.

テスト用リリースです。実機のテレビではまだ試していません。

0.2.0-beta.5 の変更：
- **6 桁のコードでペアリング**（Android 11 以降のワイヤレスデバッグ）をコマンドでできるようにしました：`hushtv pair <住所:ポート> <コード>`。Google の adb は要りません。アプリの画面は次のバージョンで付けます。
- **Homebrew と Scoop** で入れられるようにしました（入れ方は上の英語のとおり）。
- Mac のコマンド版を 1 つのファイル（Apple シリコンと Intel の両方）にまとめ、署名と公証を付けました。
- テレビでどうだったかを教えてもらうフォームを作りました。

Windows のインストーラーはまだ署名していません。SmartScreen が出たら「詳細情報」→「実行」を押してください。
