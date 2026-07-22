//! アプリケーション共通のエラー型。

use std::fmt;

#[derive(Debug)]
pub enum AppError {
    /// ファイルI/Oエラー(設定・キャッシュの読み書き失敗など)
    Io(String),
    /// 設定ファイル(TOML)のパース・シリアライズ失敗
    ConfigParse(String),
    /// キャッシュデータのパース・シリアライズ失敗
    CacheParse(String),
    /// ネットワークエラー(API接続失敗など)
    Network(String),
    /// JMA APIレスポンスの構造が想定と異なる
    ApiFormat(String),
    /// 地名が見つからない(類似地名のサジェスト付き)
    LocationNotFound {
        input: String,
        suggestions: Vec<String>,
    },
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Io(msg) => write!(f, "ファイル入出力エラー: {msg}"),
            AppError::ConfigParse(msg) => write!(f, "設定ファイルの解析に失敗しました: {msg}"),
            AppError::CacheParse(msg) => write!(f, "キャッシュデータの解析に失敗しました: {msg}"),
            AppError::Network(msg) => write!(f, "ネットワークエラー: {msg}"),
            AppError::ApiFormat(msg) => {
                write!(f, "APIレスポンスの形式が想定と異なります: {msg}")
            }
            AppError::LocationNotFound { input, suggestions } => {
                if suggestions.is_empty() {
                    write!(f, "地名「{input}」が見つかりません")
                } else {
                    write!(
                        f,
                        "地名「{input}」が見つかりません。もしかして: {}",
                        suggestions.join(", ")
                    )
                }
            }
        }
    }
}

impl std::error::Error for AppError {}
