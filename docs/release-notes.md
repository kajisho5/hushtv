**Test release (command line).** Not yet tried on a real TV: read `hushtv plan` before `hushtv apply`, and keep the restore record.

New in beta.2:
- **Light / Full** (`--preset`). Light, the default, switches off only ads and tracking.
- **Safe mode** (on by default). 4 apps at a time, then a check that the TV answers and Home and Settings still open; a bad batch is undone and the run stops. `--fast` turns it off.
- **Before/after share image** (1600×900 PNG) after `apply`. `--no-share` skips it.

テスト用リリース（コマンド操作のみ）。実機のテレビではまだ試していません。

beta.2 で追加：
- 「軽め／全部」の選択（既定は軽め＝広告と追跡だけ）
- 安全モード（既定でオン）：4 個ずつ止めて確かめ、だめならその組を戻して止まる
- 止める前後のホーム画面からビフォー・アフター画像を作る
