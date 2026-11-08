# hushtv（仮）詳細設計書

作成日：2026-11-08　／　状態：確認待ち（承認されるまで実装しない）

Android TV / Google TV の広告・追跡・おすすめ欄などの不要アプリを、パソコンから Wi-Fi 経由で「止める（削除はしない）」ためのアプリ。
**cleanyourtv の中身（機種別リスト・安全装置・点検）** と **CleanMyTV の使いやすさ（インストール不要・画面で操作・ホーム画面の導入）** を、Rust で 1 本にまとめる。

---

## 1. 名前（候補）

| 候補 | 意味 | GitHub 同名 | crates.io |
|---|---|---|---|
| **hushtv**（仮で採用） | テレビを静かにする | 0 件 | 空き |
| tvsweep | テレビを掃除する | 0 件 | 空き |
| stilltv | 静かなテレビ | 0 件 | 空き |

※ 2026-11-08 時点で GitHub のリポ名検索と crates.io を確認。最終決定は梶山さん。

---

## 2. 元の 2 つとの関係と、勝てるところ

### 事実（2 つのリポを中まで読んで確認）

| | cleanyourtv | CleanMyTV | hushtv（本作） |
|---|---|---|---|
| 作り | Python のスクリプト群（約 9,600 行）＋ Java のホーム画面アプリ | Electron（TypeScript＋React）。Kotlin のホーム画面アプリ | Rust（画面は Tauri＋Svelte） |
| 必要なもの | Python、JDK 17〜22、Android SDK 34、自分でビルド | なし（ワイヤレスデバッグのペアリング時だけ Google の adb を自動ダウンロード） | **なし（ペアリングも含めて全部内蔵）** |
| 対応 OS | Mac / Win / Linux（コマンド） | Mac / Win | **Mac / Win / Linux（画面＋コマンド）** |
| 機種別リスト | 456 機種・122 ブランド | 元のリストを同梱 | 元のリストを同梱＋アプリ内で更新 |
| ファーム更新後の点検 | あり（check.py。コマンドのみ） | なし | **あり（画面で 1 クリック・定期点検）** |
| 日本語 | README のみ | なし | **画面ごと日本語・英語** |
| ライセンス | MIT | MIT | MIT（元 2 つの表記を同梱） |
| 現状 | 約 5.4 万スター、毎日更新 | v0.7.2、187 スター | — |

### 勝てるところ（推測を含む）
1. **完全に単体で動く**：ワイヤレスデバッグのペアリング（6 桁コード）を Rust で自前実装する。Google の adb を一切ダウンロードしない。
   - 根拠：同じことを Dart（flutter_adb）と Java（spake2-java）が実現済み。Rust で同じ作りにできる見込み（**実機での確認が必要**）。
2. **軽い**：Tauri は OS 内蔵のブラウザ部品を使うので、アプリ本体は 10MB 前後が目標（**推測**。Electron は一般に 100MB 前後）。
3. **点検が画面でできる**：ファーム更新で戻った不要アプリ・入れ替わったホーム画面を見つけて、1 クリックで直す。
4. **コマンド操作（CLI）も同じ中身**：`hushtv check` などを Linux サーバーや NAS から定期実行できる。
5. **日本語**：日本で売られている機種（シャープ・ソニー・パナソニック・TCL など）の利用者に届く。

### 正直な弱点
- CleanMyTV にはホーム画面の細かい編集（並び替え・壁紙・暖炉など）がある。v0.1 では追いつかないので、**v0.1 は「片付け・点検・元に戻す」の確実さで勝負**し、編集機能は後回しにする。

---

## 3. 要件

### v0.1.0 でやること
| # | 機能 | 内容 |
|---|---|---|
| R1 | テレビを探す | mDNS（LAN 内の機器が名前を知らせる仕組み）で自動検出。見つからなければ IP を手入力。5555 番ポートも確認 |
| R2 | つなぐ | ① 旧来のネットワークデバッグ（5555 番、暗号化なし） ② ワイヤレスデバッグ（暗号化あり。6 桁コードでペアリング） |
| R3 | 機種を当てる | テレビの機種名・入っているアプリ一覧から、同梱リストの候補を点数順に出す（元の match.py と同じ考え方） |
| R4 | 変更前の確認 | 止めるもの／元から止まっている／入っていない／止めてはいけない（安全装置で除外）を一覧表示。1 個ずつオン・オフ可 |
| R5 | 戻す地点を保存 | 変更の前に、テレビの状態を記録。**元の cleanyourtv と同じ形式**（`~/.cleanyourtv/recovery-*.json`）で、どちらの道具でも戻せる |
| R6 | 止める | `pm disable-user --user 0` で無効化（削除しない）。実行後にテレビへ問い合わせて、本当に止まったか読み戻す |
| R7 | ホーム画面を入れる | 元の cleanyourtv のホーム画面アプリ（Java 製、約 572KB）を同梱して導入。ホームが切り替わらなければ自動で元に戻す |
| R8 | 点検と修復 | ファーム更新の後、戻ってきたアプリ・新しく増えたアプリ・ホームの入れ替わりを表示。「直す」で再適用 |
| R9 | 元に戻す | 記録した状態に 1 クリックで戻す |
| R10 | 終わる | ネットワークデバッグを閉じる手順を案内し、閉じたことをポートで確認 |
| R11 | コマンド操作（CLI） | `hushtv scan / connect / pair / plan / apply / check / fix / restore` |
| R12 | 言語 | 日本語・英語（OS の言語で自動選択。設定で変更可） |

### 安全装置（元の cleanyourtv から全部移す。ここが一番大事）
- **止めてはいけない部品**：Android の中核（`android`、`com.android.systemui`、`com.android.shell` など）とテレビの制御部品。止めるとテレビが起動しなくなる事例がある。元の正規表現（CORE_RE・CORE_OK）をそのまま移す
- **ホームの持ち主を止めない**：ほかにホームになれるアプリがない状態で、今のホームを止めない
- **初期設定画面（setupwraith）の扱い**：Google TV で落ち続けたり、保存済み Wi-Fi が消えたりする事例があるので、元と同じ条件で止めるのを見送る
- **ホームではないもの**：設定アプリ、FallbackHome、工場テスト画面をホームと見なさない
- **名前の検査**：テレビへ送るアプリ名は、パッケージ名の形をしたものだけを通す（シェルへの不正な文字の混入を防ぐ）
- **テストの移植**：元の `tests/`（268 ファイル）のうち安全装置にかかわるものを Rust のテストへ移し、同じ結果になることを確認する

### 後回し（v0.2 以降）
- ホーム画面の編集（並び・テーマ・壁紙。元の `config.json` を Wi-Fi で送る仕組みを使う）
- APK の導入、DNS 広告ブロックの設定、アニメーション速度
- 定期点検の通知（週 1 回）
- iPhone アプリ（中身の Rust 部分を共通化しておく）

### やらないこと
- アプリの削除（アンインストール）、root 化
- Samsung（Tizen）・LG（webOS）など ADB のないテレビ
- 他人のテレビの操作。同じ LAN の自分の機器だけを対象にする

---

## 4. 技術選定

### 画面の作り方の比較

| | **Tauri 2 ＋ Svelte 5**（おすすめ） | egui（eframe） | Slint |
|---|---|---|---|
| 動作の軽さ | ◎ OS 内蔵のブラウザ部品 | ◎ | ◎ |
| 配布サイズ | ○ 数 MB〜10MB 前後（推測） | ◎ 数 MB（日本語フォント同梱で +数 MB） | ◎ 数 MB |
| 作りやすさ | ◎ 表・チェックボックス・日本語が素直に作れる | △ 表や日本語の見た目を整えるのに手間 | ○ 独自の画面記述言語を覚える必要 |
| 対応 OS | Mac / Win / Linux（Linux は WebKitGTK が必要） | Mac / Win / Linux | Mac / Win / Linux |
| 将来性 | ◎ 2.x が安定。iPhone 対応もある | ○ | ○（商用条件のライセンスに注意） |
| AI での書きやすさ | ◎ 情報が多い | ○ | △ 情報が少ない |
| ライセンス | MIT / Apache-2.0 | MIT / Apache-2.0 | GPL-3.0 または独自の無料ライセンス |

**おすすめ：Tauri 2 ＋ Svelte 5。** 中身（ADB・ペアリング・安全装置）は 100% Rust のまま、画面だけ Web の技術で作れる。表の多い画面を短期間で作りやすく、軽さもほぼ落ちない。React ではなく Svelte にするのは、出来上がりが小さく軽いため。
※ 用途別の目安どおり「デスクトップアプリ＝Tauri」。

### 使うもののバージョン（2026-11-08 に公式で確認）

| 用途 | 名前 | バージョン | 根拠 |
|---|---|---|---|
| 言語 | Rust | 1.99.1 | https://github.com/rust-lang/rust/releases |
| アプリの枠 | tauri | 2.12.5 | https://crates.io/crates/tauri |
| ビルド道具 | @tauri-apps/cli | 2.12.4 | https://www.npmjs.com/package/@tauri-apps/cli |
| 画面 | svelte | 5.57.3 | https://www.npmjs.com/package/svelte |
| 画面のビルド | vite | 8.3.6 | https://www.npmjs.com/package/vite |
| 〃 | @sveltejs/vite-plugin-svelte | 7.3.1 | https://www.npmjs.com/package/@sveltejs/vite-plugin-svelte |
| 型 | typescript | 7.0.2 | https://www.npmjs.com/package/typescript |
| 見た目 | tailwindcss | 4.3.5 | https://www.npmjs.com/package/tailwindcss |
| ADB 通信 | adb_client | 3.2.3（MIT） | https://crates.io/crates/adb_client |
| LAN 内の検出 | mdns-sd | 0.21.7 | https://crates.io/crates/mdns-sd |
| 暗号化通信 | rustls | 0.23.46 | https://crates.io/crates/rustls |
| ペアリング用の計算 | curve25519-dalek | 5.0.0 | https://crates.io/crates/curve25519-dalek |
| 〃 | hkdf 0.13.0 ／ sha2 0.11.1 ／ aes-gcm 0.11.1 | — | https://crates.io/crates/hkdf ほか |
| コマンド操作（CLI） | clap | 4.6.7 | https://crates.io/crates/clap |
| 非同期処理 | tokio | 1.54.1 | https://crates.io/crates/tokio |
| データ | serde | 1.0.229 | https://crates.io/crates/serde |

- 非推奨のもの・beta・RC のものは使わない（Tauri 3 はまだ alpha なので使わない）
- adb_client について（中身を読んで確認）：TCP 直結と暗号化（TLS）への切り替え、mDNS での検出には対応済み。**ペアリングは「Google の adb サーバー経由」しかない**ので、ここだけ自作する

### ペアリングの自作（一番の山場）
Android のワイヤレスデバッグのペアリングは、次の手順で行われる。
1. TLS で接続し、通信の鍵から 64 バイトの値を取り出す（exported keying material）
2. 6 桁コード＋その値で SPAKE2（パスワードから共通の鍵を作る方式。BoringSSL の Ed25519 を使う方式）を交換する
3. HKDF と AES-128-GCM で暗号化して、お互いの公開鍵（PeerInfo）を交換する

- 方式は AOSP（Android の公開ソース）の `pairing_auth.cpp` と、Dart / Java の実装例を参考に Rust で書く
- **リスク**：BoringSSL と 1 ビットでも違うと失敗する。最初の作業で実機（Android 11 以降の機器）でつながることを確認し、だめなら v0.1 では「ペアリングだけ Google の adb を使う」に切り替える

---

## 5. 画面構成（SVG 画面案 4 枚）

左に進み具合のサイドバー、右に作業の画面。上から順に進めば終わる作り。

| # | 画面 | 中身 |
|---|---|---|
| 1 | **つなぐ** | 見つかったテレビの一覧（名前・IP・つなぎ方の種類）。ワイヤレスデバッグなら 6 桁コードの入力欄。テレビ側でやることを手順で表示 |
| 2 | **片付ける** | 当てはまった機種リスト、件数のまとめ（止める／元から止まっている／守る／入っていない）、アプリの表（種類のラベル・理由・ほかの機種で止めた数）。「戻す地点を保存して反映」ボタン |
| 3 | **ホーム画面** | 新しいホーム画面の見本、導入ボタン、元のホームに戻すボタン |
| 4 | **点検** | 前回の片付けとの違い（戻ってきた・新しく増えた・ホームが入れ替わった）、「直す」ボタン、元に戻す、デバッグを閉じる確認 |

- 画面のデータはすべて架空（例：「リビングのテレビ」「ACME 55U8（架空の機種）」「com.example.adservice」）
- 色：濃い紺の背景＋ミント色の強調。危険は赤、注意は黄、完了は緑
- 文字：Noto Sans JP（ない場合は OS の日本語フォント）。数字とアプリ名は等幅

---

## 6. 部品の分け方

### Rust 側（クレート＝Rust の部品パッケージ）
| クレート | 役割 |
|---|---|
| `hushtv-core` | 安全装置・リストの読み込み・機種の照合・計画づくり・戻す記録。**テレビと通信しない純粋な計算だけ**（テストしやすくするため） |
| `hushtv-adb` | テレビとの通信。adb_client を包み、検出・接続・ペアリング・シェル実行・APK 導入をまとめる |
| `hushtv-cli` | コマンド操作（CLI） |
| `hushtv-app` | Tauri の本体。画面からの呼び出し（command）と進み具合の通知（event） |

### 画面側（Svelte）
- `Sidebar`（進み具合）、`TvPicker`（テレビ一覧）、`PairDialog`（6 桁コード）
- `PlanSummary`（件数のまとめ）、`PackageTable`（アプリの表）、`SafetyNote`（守られた理由）
- `HomePreview`（ホーム画面の見本）、`DriftReport`（点検結果）、`LogDrawer`（実行ログ。下から開く）

---

## 7. 状態管理

- **正しい状態は Rust 側だけが持つ**。画面は Rust から受け取った状態を表示するだけ（二重管理で食い違うのを防ぐ）
- 画面 → Rust：`invoke("plan", …)` のような呼び出し
- Rust → 画面：`progress`（実行中の進み具合）、`tv-changed`（接続が切れた・戻った）のイベント
- 進む順番：`未接続 → 接続済み → 計画済み → 保存済み → 反映済み → 点検済み`。前の段階が終わらないと次のボタンは押せない
- 設定（言語・保存したテレビ）は OS の設定フォルダに JSON で保存。鍵（ADB の秘密鍵）は同じフォルダに、ほかの人が読めない権限で保存

---

## 8. データ構造

```rust
/// 機種別リスト（examples/<機種>/debloat.txt と tv.json）
struct DebloatList {
    id: String,              // フォルダ名 例 "acme-55u8"（架空）
    tv: String, os: String, brand: String,
    disable: Vec<Entry>,     // 止めるアプリ
    keep: Vec<Entry>,        // コメントアウト＝残すと書かれたアプリ
    header_notes: String,    // 冒頭の注意書き（画面に表示）
}
struct Entry { package: PackageName, note: Option<String> }

/// テレビの今の状態
struct TvSnapshot {
    host: String, model: String, android: String,
    packages: BTreeMap<PackageName, PkgState>, // Enabled / Disabled / Absent
    home: Option<Component>,                   // 今のホーム
    home_candidates: Vec<Component>,
}

/// 実行前の計画
struct Plan {
    to_disable: Vec<PackageName>,
    already_disabled: Vec<PackageName>,
    absent: Vec<PackageName>,
    protected: Vec<(PackageName, SafetyReason)>, // 安全装置で外したもの＋理由
}

/// 戻す記録（元の cleanyourtv と同じ JSON。version = 1）
struct RecoveryRecord {
    version: u32, created: String, host: String, list: String,
    home: Option<String>,
    settings: BTreeMap<String, String>,       // アニメーション速度 3 種
    packages: BTreeMap<String, String>,       // "enabled" / "disabled" / "absent"
}
```

- 記録のファイル名も元と同じ規則（`recovery-<アドレス>-<リストのパスの sha1 先頭 8 桁>.json`）にして、cleanyourtv の `restore.sh` でも戻せるようにする

---

## 9. フォルダ構成

```
hushtv/
├── Cargo.toml                 Rust のまとめ設定
├── crates/
│   ├── core/                  安全装置・照合・計画・記録（純粋な計算）
│   ├── adb/                   検出・接続・ペアリング・シェル
│   ├── cli/                   コマンド操作（CLI）
│   └── app/                   Tauri 本体（src-tauri 相当）
├── ui/                        Svelte の画面
│   └── src/{routes,components,lib,i18n}
├── data/
│   ├── lists/                 元の examples/ を同期したもの
│   ├── launcher/              ホーム画面アプリ（CI でビルドした APK）
│   └── upstream.json          同期した元のコミットと sha256
├── tests/                     元のテストから移した安全装置のテスト
├── docs/                      使い方（日本語・英語）
├── THIRD-PARTY-NOTICES.md     元 2 つと依存ライブラリのライセンス
└── .github/workflows/
    ├── ci.yml                 テスト（Mac / Win / Linux）
    ├── sync-upstream.yml      元のリストを毎週取り込み、PR を作る
    ├── launcher.yml           ホーム画面アプリを JDK 17＋SDK 34 でビルド
    └── release.yml            未リリースのバージョン番号なら自動でリリース
```

---

## 10. ビルドと配布

- ビルドはすべて GitHub Actions（Mac・Windows・Linux の実行環境）。Mac がなくても出せる
- `main` に入ったとき、`Cargo.toml` のバージョン番号がまだリリースされていなければ、自動でリリースを作る
- 最初は署名なし（Mac は初回起動時の許可、Windows は SmartScreen の警告あり）。署名・公証は後で追加
- リリースに含めるもの：Mac（.dmg）、Windows（.msi と単体 .exe）、Linux（.AppImage と .deb）、コマンド操作（CLI）（各 OS の単体ファイル）

---

## 11. 不明なこと・確認が必要なこと

| 項目 | 状態 |
|---|---|
| Rust でのペアリングが実機でつながるか | **不明**。最初に検証する |
| 実機テスト用の Android TV | 必要。手元の機器を確認したい（なければ Android TV のエミュレーターで代用。ただし最後は実機で確認） |
| 配布サイズ 10MB 前後 | **推測**。ビルドして実測する |
| 元のリストの更新頻度に追いつけるか | 毎日更新されている。週 1 回の自動取り込みで十分かは運用して判断 |
| 元の作者への連絡 | MIT なので許可は不要。ただし README に出どころを明記し、元のリポに一言知らせるのが望ましい（推測） |
