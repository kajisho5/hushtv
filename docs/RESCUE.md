# Rescue a TV / テレビを直す

`hushtv rescue` repairs Android TVs that misbehave after a debloat, whether it was done with hushtv, [cleanyourtv](https://github.com/Gathin23/cleanyourtv), [CleanMyTV](https://github.com/wingedpanther/cleanmytv) or plain `adb`. It only ever turns things **back on** (`pm enable`, `cmd package install-existing`); it never disables or uninstalls anything.

```sh
hushtv rescue 192.168.1.50 --dry-run    # find problems, change nothing
hushtv rescue 192.168.1.50              # quick fix (recommended)
hushtv rescue 192.168.1.50 --level all  # also turn back on every app the debloat lists switch off
hushtv rescue 192.168.1.50 --record     # replay a restore record (cleanyourtv, CleanMyTV and hushtv share the format)
```

| What hushtv finds | Symptom | Quick fix does |
|---|---|---|
| Core Android packages off | Boot loop, stuck on logo, crashes | Turns them back on |
| No working Home | Black screen, "Select a Home app", setup screen | Turns launchers (and the setup wizard) back on, points Home at the stock launcher |
| Settings does not open | No way back to Developer options | Turns the Settings app back on or reinstalls it for the user |
| Input / tuner packages off | HDMI "No Signal", inputs or Live TV won't open | Turns them back on (picked by name; a guess) |
| Setup wizard off | Wi-Fi disappears, casting says "Untrusted device" | Turns it back on |
| Apps uninstalled for the user | An app or feature is gone | Reinstalls the copy still on the TV (vital ones only; `--level all` for every listed app) |
| Animations at 0 | UI looks frozen | Sets them back to 1.0 |

## If the TV cannot be reached

1. **Wi-Fi is gone**: plug in an Ethernet cable, or re-join Wi-Fi from the remote's Settings button → Network.
2. **Black screen, Home does nothing**: open Settings with the remote's Settings or Input button, or by voice. Turn debugging back on in Developer options, then run `hushtv rescue` again.
3. **Stuck on the boot logo or restarting**: no tool can reach a TV that does not boot. Factory reset from the recovery menu (usually: hold a button while powering on). This erases the TV, and the steps differ per model, so check the manual.

Reboot the TV after a rescue, then check Home, every input, Settings and Wi-Fi.

---

## 日本語

`hushtv rescue` は、hushtv・cleanyourtv・CleanMyTV・手作業の adb で止めすぎて調子が悪くなった Android TV を直します。直し方は「戻す」だけで、止めたり消したりは一切しません。

- `--dry-run`：診断だけ（何も変えない）
- 既定（応急処置）：起動・ホーム・設定・入力・Wi-Fi にかかわるものだけ戻す
- `--level all`：機種別リストに載っているアプリを全部戻す
- `--record`：残っている戻す記録どおりに戻す（3 つのツールで同じ形式）

**つながらないとき**

1. Wi-Fi が消えた：有線 LAN をつなぐか、リモコンの設定ボタン → ネットワークで Wi-Fi に入り直す
2. 画面が真っ黒・ホームが開かない：リモコンの［設定］［入力切替］か音声で設定を開き、開発者向けオプションでデバッグをオンにしてから、もう一度 `hushtv rescue`
3. 起動ロゴのまま・再起動を繰り返す：起動しないテレビにはどのツールも届きません。リカバリー（電源を入れるときにボタンを押し続ける、など）から初期化します。データが消えます。操作は機種ごとに違うので説明書を確認してください

直したら、テレビを再起動して、ホーム・全部の入力・設定・Wi-Fi を確かめてください。
