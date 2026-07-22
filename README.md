# weather

気象庁(JMA)非公式JSON APIを利用した、ターミナル上で完結する天気予報TUIツール(Rust製)。

天気に応じたASCIIアニメーション(晴れ・曇り・雨・雪・雷)を背景に、今日・明日/週間予報と気象警報・注意報を表示します。

- 要件定義: `docs/requirements.md`
- 実装計画: `docs/plans/`

## 必要環境

- Rust(edition 2024 が使える安定版ツールチェイン)
- macOS / Linux
- True Color対応ターミナル(iTerm2, Terminal.app, Alacritty など)

## ビルドとインストール

```sh
# ビルド
cargo build --release

# そのまま実行する場合
./target/release/weather 東京

# パスの通った場所にインストールする場合
cargo install --path .
```

## 使い方

```sh
weather 東京          # 指定地点の天気を表示
weather --set 東京    # デフォルト地点を保存(以後は引数なしでOK)
weather               # デフォルト地点の天気を表示
weather --demo        # 動作確認用: ダミーデータでアニメーションを確認
```

- 対応地点は都道府県庁所在地レベル(約47件)。「名古屋」→「愛知」のようなエイリアスも解決されます。
- 地名を打ち間違えると類似地名をサジェストして終了します(例: `とうきよう` → `もしかして: 東京`)。
- 常駐はしません。実行するたびに取得するワンショット型です。

### TUIのキー操作

| キー | 動作 |
| --- | --- |
| `Tab` / `←` `→` / `h` `l` | タブ切り替え(今日・明日 ⇔ 週間予報) |
| `1` / `2` | タブ直接選択(1=今日・明日、2=週間予報) |
| `q` / `Esc` | 終了 |
| `n` / `Space` | (`--demo` 中のみ)アニメーションパターン切り替え |

- 気象警報・注意報の発令中は、パネル最上部に帯で強調表示されます(特別警報=マゼンタ帯、警報=赤帯、注意報=黄文字)。

## 設定ファイル

初回の `--set` 実行時などに以下へ作成されます(手で作成しても構いません)。

- macOS: `~/Library/Application Support/weather/config.toml`
- Linux: `~/.config/weather/config.toml`

```toml
# デフォルト地点(--set で保存される。解決後の正規名)
default_location = "東京"

# レイアウトプリセット: "fullscreen"(既定) | "dashboard"
# fullscreen: 背景アニメ全面 + 中央に小さめの情報パネル
# dashboard : 画面の大部分をパネルが占め、余白にアニメが見える
layout = "fullscreen"

# カラーテーマ: "dark"(既定) | "light" | "vivid"
theme = "dark"

[animation]
enabled = true   # 背景アニメーションのON/OFF
speed = 1.0      # 再生速度の倍率(0.1〜5.0にクランプ)
density = 1.0    # 描画密度の倍率(0.0〜3.0にクランプ)
```

- 未知のレイアウト名・テーマ名を書いた場合は、起動前に有効値を提示してエラー終了します。
- 設定ファイルが壊れている(TOML解析不能)場合は、上書きせずファイルパスを示してエラー終了します。

## データソースとキャッシュ

- データは気象庁の非公式JSON API(`www.jma.go.jp/bosai/`)から取得します。
  - 非公式APIのため、連絡先入りの `User-Agent` を送り、10分TTLのキャッシュで連続実行時のリクエストを抑制しています。
  - APIのJSON構造が変わった場合はクラッシュせずエラーメッセージを表示して終了します。
- キャッシュの場所:
  - macOS: `~/Library/Caches/weather/`
  - Linux: `~/.cache/weather/`
- ネットワークに繋がらないときは、期限切れキャッシュがあれば警告付きでフォールバック表示します(キャッシュもなければエラー終了)。

## 開発者向け

```sh
cargo test      # 単体テスト(ロジック部分。TUIの見た目は手動確認)
cargo clippy    # リント
weather --demo  # ネットワーク不要でアニメ5パターン・警報帯・テーマを確認
```

- 開発の進め方・タスク分割は `docs/plans/` を参照してください。

## ライセンス

未定(現状は私的利用のみ)。
