//! `weather` TUI天気予報ツールのロジック本体。
//!
//! バイナリ本体(`main.rs`)はここに定義されたモジュールを呼び出すだけの薄いエントリーポイントとする。

pub mod animation;
pub mod cache;
pub mod cli;
pub mod config;
pub mod error;
pub mod jma_client;
pub mod location;
pub mod tui;
