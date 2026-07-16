# Task 1 詳細計画: プロジェクト基盤構築

- 親計画: `docs/plans/mvp_plans.md` の「Task 1: プロジェクト基盤構築」
- 本ファイルはgrillingスキルによる詳細設計の結果。Task 1完了後は `docs/plans/done/` に移動する。

## 目的

以降の全タスクの土台となるRustプロジェクトの雛形を整える。

## grillingで確定した設計判断

### 1. クレート構成: lib + thin bin

- `src/lib.rs` に全ロジックモジュールを配置する。
- `src/main.rs` はlibを呼び出すだけの薄いエントリーポイントとする。
- 理由: 以降のタスク(設定パース、キャッシュ期限判定、地名マッチング、APIレスポンスパース、CLI分岐など)で単体テストが大量に必要になるため、`cargo test` で直接ロジックをテストできる構成にしておく。

### 2. Rust edition: 2024

- ローカルの `rustc 1.95.0` で完全サポートされている。外部公開・互換性維持の予定がないMVPのため最新editionを採用する。

### 3. モジュール構成: フラットファイル構成

`src/lib.rs` から以下8モジュールを `mod xxx;` 宣言する。すべて `src/xxx.rs` のフラットファイル(`mod.rs`ディレクトリ方式は使わない)。

- `config`: TOML設定ファイルの読み書き(Task 2)
- `location`: 地名→JMAエリアコード解決(Task 3)
- `jma_client`: JMA APIクライアント・レスポンスパース(Task 4)
- `cache`: TTL付き汎用キャッシュ(Task 2)
- `animation`: 天気コード→ASCIIアニメーションカテゴリ・密度計算(Task 7)
- `cli`: `clap`によるコマンドライン引数定義・地点解決(Task 5)
- `tui`: `ratatui`情報パネル・タブ状態遷移(Task 6)
- `error`: 共通エラー型(本タスクで雛形作成)

Task 1時点では `error` 以外は中身が空のファイル(モジュールdocコメント程度)でよい。将来サブモジュールが必要になった時点で `src/xxx/` ディレクトリに分割する。

### 4. エラー型 (`src/error.rs`)

- `pub enum AppError {}` (variantゼロの雛形。具体的なvariantは各タスクで実際に必要になったタイミングでTDDにより追加する)
- `impl std::fmt::Display for AppError { fn fmt(&self, _f: &mut std::fmt::Formatter) -> std::fmt::Result { match *self {} } }`
- `impl std::error::Error for AppError {}`
- 本タスクではテストを書かない(具体ロジックがまだ存在しないため)。Task 2以降、実際のエラーケースが生じるタイミングでRed→Greenサイクルに乗せる。

### 5. 依存クレート: `cargo add` で最新安定版を取得

バージョンは固定せず `cargo add` 実行時点の最新安定版(caret指定)を採用する。

- 通常依存: `ratatui`, `crossterm`, `ureq`, `clap`(`derive` feature), `serde`(`derive` feature), `serde_json`, `toml`, `directories`, `strsim`
- 開発依存: `mockito`
- 注意: `ratatui` はデフォルトで `crossterm` バックエンドを内包するため、`cargo add` 後に `Cargo.lock` 上で両者のcrosstermバージョンが一致しているか確認する。

### 6. `.gitignore`

- `/target` を除外
- `.DS_Store` を除外(macOS開発のため)
- `Cargo.lock` はバイナリクレートのためコミット対象(除外しない)

### 7. README.md

- 現状の仮内容(`hoge`)を、ツール概要・ビルド/実行方法(後続タスクで追記していく前提)のプレースホルダー骨組みに置き換える。

## 作業手順

1. `Cargo.toml` 作成(package名 `weather`, `edition = "2024"`, `version = "0.1.0"`)
2. `src/lib.rs` 作成し、8モジュールを `mod` 宣言
3. 各モジュールファイルを作成(`error.rs`以外は空)
4. `src/error.rs` に `AppError` 雛形を実装
5. `src/main.rs` をlib呼び出しの薄いラッパーに変更
6. `cargo add` で通常依存・開発依存を追加
7. `.gitignore` 整備
8. `README.md` 骨組み更新
9. `cargo build` / `cargo test` で確認

## TDD適用範囲

Task 1はスキャフォールディングのみでテスト対象のロジックが存在しないため、t_wada式TDDのRed→Green→Refactorサイクルは適用しない(要件定義書4.4節およびmvp_plans.mdのTask1に「TDD対象」記載がないことと整合)。次タスク(Task 2: 設定・キャッシュ基盤)から本格的にTDDサイクルを開始する。

## 完了条件 (DoD)

- `cargo build` が通る
- `cargo test` が(0件でも)通る
- 8モジュールの雛形と `AppError` 雛形がリポジトリに存在する
- `.gitignore` ・README骨組みが整備されている
