**Test release.** Download the app for your computer below (`.dmg` for macOS, `-setup.exe` for Windows, `.AppImage` or `.deb` for Linux), or the one-file command line (`hushtv-*`). Not yet tried on a real TV.

New in 0.2.0-beta.2 — Repair now covers the rest of cleanyourtv's troubleshooting guide:
- Private DNS pinned to one server (one app won't play, "No internet" after a power cut) → back to automatic, automatic time on
- Setup marked unfinished (Home and voice buttons dead) → marked complete
- Accessibility pointing at a missing or disabled app, or switched off (dead remote buttons) → cleaned and switched back on
- Always-on VPN locked to an app that is gone → lock removed
- HDMI-CEC off → turned back on with "Everything" only

The installers are not signed yet: on macOS, right-click → Open the first time; on Windows, SmartScreen → More info → Run anyway.

テスト用リリースです。実機のテレビではまだ試していません。

0.2.0-beta.2 で「テレビを直す」に追加（cleanyourtv の不具合資料の後半から）：
- プライベート DNS の固定 → 自動に戻す（時刻の自動設定もオン）
- 初期設定が終わっていない扱い → 完了にする
- ユーザー補助が消えた・止まったアプリを指している／スイッチが切れている → 整理してオン
- ないアプリに固定された常時オン VPN → 固定を外す
- HDMI-CEC → 「全部戻す」のときだけオン
