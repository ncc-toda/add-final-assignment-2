//! アプリケーション共通のエラー型。
//!
//! 具体的なvariantは、それを必要とする機能(設定読み込み・API通信・地名解決など)の
//! 実装時にTDDで追加していく。現時点ではDisplay/Error実装の雛形のみを用意する。

use std::fmt;

#[derive(Debug)]
pub enum AppError {}

impl fmt::Display for AppError {
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {}
    }
}

impl std::error::Error for AppError {}
