//! アプリケーション共通のエラー型。

use std::fmt;

#[derive(Debug)]
pub enum AppError {
    /// ファイルI/Oエラー(設定・キャッシュの読み書き失敗など)
    Io(String),
    /// 設定ファイル(TOML)のパース・シリアライズ失敗
    ConfigParse(String),
    /// 設定値が不正(未知のレイアウト名・テーマ名など)
    ConfigInvalid(String),
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
    /// 引数なし実行時にデフォルト地点が未設定
    DefaultLocationNotSet,
    /// 設定ファイルのデフォルト地点が解決できない
    DefaultLocationInvalid {
        input: String,
        suggestions: Vec<String>,
    },
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Io(msg) => write!(f, "ファイル入出力エラー: {msg}"),
            AppError::ConfigParse(msg) => write!(f, "設定ファイルの解析に失敗しました: {msg}"),
            AppError::ConfigInvalid(msg) => write!(f, "設定値が不正です: {msg}"),
            AppError::CacheParse(msg) => write!(f, "キャッシュデータの解析に失敗しました: {msg}"),
            AppError::Network(msg) => write!(f, "ネットワークエラー: {msg}"),
            AppError::ApiFormat(msg) => {
                write!(f, "APIレスポンスの形式が想定と異なります: {msg}")
            }
            AppError::LocationNotFound { input, suggestions } => {
                write_location_not_found(f, input, suggestions)
            }
            AppError::DefaultLocationNotSet => {
                writeln!(f, "デフォルト地点が未設定です。")?;
                writeln!(f, "  weather <地名>        … 地点を指定して表示")?;
                write!(f, "  weather --set <地名>  … デフォルト地点を保存")
            }
            AppError::DefaultLocationInvalid { input, suggestions } => {
                write_location_not_found(f, input, suggestions)?;
                write!(
                    f,
                    "\n(設定のデフォルト地点が不正です。weather --set <地名> で設定し直してください)"
                )
            }
        }
    }
}

/// 「地名「X」が見つかりません(もしかして: …)」の共通整形。
fn write_location_not_found(
    f: &mut fmt::Formatter<'_>,
    input: &str,
    suggestions: &[String],
) -> fmt::Result {
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

impl std::error::Error for AppError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn デフォルト地点未設定エラーは設定方法を案内する() {
        let msg = AppError::DefaultLocationNotSet.to_string();
        assert!(msg.contains("デフォルト地点が未設定です"), "msg: {msg}");
        assert!(msg.contains("weather <地名>"), "msg: {msg}");
        assert!(msg.contains("weather --set <地名>"), "msg: {msg}");
    }

    #[test]
    fn デフォルト地点不正エラーはサジェストと再設定案内を含む() {
        let msg = AppError::DefaultLocationInvalid {
            input: "とうきよう".to_string(),
            suggestions: vec!["東京".to_string()],
        }
        .to_string();
        assert!(msg.contains("「とうきよう」が見つかりません"), "msg: {msg}");
        assert!(msg.contains("もしかして: 東京"), "msg: {msg}");
        assert!(msg.contains("weather --set <地名>"), "msg: {msg}");
    }

    #[test]
    fn デフォルト地点不正エラーはサジェストなしでも再設定案内を含む() {
        let msg = AppError::DefaultLocationInvalid {
            input: "zzz".to_string(),
            suggestions: vec![],
        }
        .to_string();
        assert!(msg.contains("「zzz」が見つかりません"), "msg: {msg}");
        assert!(!msg.contains("もしかして"), "msg: {msg}");
        assert!(msg.contains("weather --set <地名>"), "msg: {msg}");
    }
}
