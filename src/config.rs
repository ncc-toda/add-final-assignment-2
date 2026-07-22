//! TOML設定ファイルの読み書き(デフォルト地点・レイアウト・カラーテーマ・アニメーション設定)。
//!
//! パース/シリアライズのロジック(`from_toml_str` / `to_toml_string`)と
//! ファイルI/Oの薄いラッパー(`load_or_default` / `save`)を分離している。

use std::fs;
use std::path::{Path, PathBuf};

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

use crate::error::AppError;

/// アニメーション関連の設定。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AnimationConfig {
    /// アニメーションのON/OFF(既定: true)
    pub enabled: bool,
    /// 再生速度の倍率(既定: 1.0)
    pub speed: f64,
    /// 描画密度の倍率(既定: 1.0)
    pub density: f64,
}

impl Default for AnimationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            speed: 1.0,
            density: 1.0,
        }
    }
}

/// アプリケーション全体の設定。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// デフォルト地点名(未設定なら `None`)
    pub default_location: Option<String>,
    /// レイアウトプリセット名(既定: "fullscreen")
    pub layout: String,
    /// カラーテーマ名(既定: "dark")
    pub theme: String,
    /// アニメーション設定
    pub animation: AnimationConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            default_location: None,
            layout: "fullscreen".to_string(),
            theme: "dark".to_string(),
            animation: AnimationConfig::default(),
        }
    }
}

impl Config {
    /// TOML文字列から設定を生成する。欠けている項目はデフォルト値で補完する。
    pub fn from_toml_str(s: &str) -> Result<Self, AppError> {
        toml::from_str(s).map_err(|e| AppError::ConfigParse(e.to_string()))
    }

    /// 設定をTOML文字列にシリアライズする。
    pub fn to_toml_string(&self) -> Result<String, AppError> {
        toml::to_string_pretty(self).map_err(|e| AppError::ConfigParse(e.to_string()))
    }

    /// 設定ファイルを読み込む。ファイルが存在しなければデフォルト設定を返す。
    /// 壊れたTOMLは `AppError::ConfigParse` を返す。
    pub fn load_or_default(path: &Path) -> Result<Self, AppError> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = fs::read_to_string(path).map_err(|e| AppError::Io(e.to_string()))?;
        Self::from_toml_str(&content)
    }

    /// 設定をファイルに保存する。親ディレクトリが存在しなければ作成する。
    pub fn save(&self, path: &Path) -> Result<(), AppError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| AppError::Io(e.to_string()))?;
        }
        let content = self.to_toml_string()?;
        fs::write(path, content).map_err(|e| AppError::Io(e.to_string()))
    }
}

/// OS標準の設定ディレクトリ配下の設定ファイルパス(`<config_dir>/config.toml`)を返す。
pub fn default_config_path() -> Option<PathBuf> {
    ProjectDirs::from("", "", "weather").map(|dirs| dirs.config_dir().join("config.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// テスト用の一意な一時ディレクトリを作成する。
    fn make_temp_dir(label: &str) -> PathBuf {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "weather_config_test_{}_{}_{}",
            std::process::id(),
            label,
            n
        ));
        fs::create_dir_all(&dir).expect("一時ディレクトリの作成に失敗");
        dir
    }

    // --- デフォルト値 ---

    #[test]
    fn デフォルト設定の値が仕様どおりである() {
        let config = Config::default();
        assert_eq!(config.default_location, None);
        assert_eq!(config.layout, "fullscreen");
        assert_eq!(config.theme, "dark");
        assert!(config.animation.enabled);
        assert_eq!(config.animation.speed, 1.0);
        assert_eq!(config.animation.density, 1.0);
    }

    // --- from_toml_str ---

    #[test]
    fn 空のtoml文字列からはデフォルト設定が得られる() {
        let config = Config::from_toml_str("").expect("パース失敗");
        assert_eq!(config, Config::default());
    }

    #[test]
    fn 一部項目のみのtomlは残りをデフォルトで補完する() {
        let toml_str = r#"
            theme = "light"

            [animation]
            speed = 2.0
        "#;
        let config = Config::from_toml_str(toml_str).expect("パース失敗");
        assert_eq!(config.theme, "light");
        assert_eq!(config.animation.speed, 2.0);
        // 未指定項目はデフォルト
        assert_eq!(config.layout, "fullscreen");
        assert_eq!(config.default_location, None);
        assert!(config.animation.enabled);
        assert_eq!(config.animation.density, 1.0);
    }

    #[test]
    fn 全項目指定のtomlを正しく読み込める() {
        let toml_str = r#"
            default_location = "東京"
            layout = "dashboard"
            theme = "vivid"

            [animation]
            enabled = false
            speed = 0.5
            density = 1.5
        "#;
        let config = Config::from_toml_str(toml_str).expect("パース失敗");
        assert_eq!(config.default_location.as_deref(), Some("東京"));
        assert_eq!(config.layout, "dashboard");
        assert_eq!(config.theme, "vivid");
        assert!(!config.animation.enabled);
        assert_eq!(config.animation.speed, 0.5);
        assert_eq!(config.animation.density, 1.5);
    }

    #[test]
    fn 壊れたtomlはconfig_parseエラーになる() {
        let result = Config::from_toml_str("this is [ not toml =");
        assert!(matches!(result, Err(AppError::ConfigParse(_))));
    }

    #[test]
    fn toml文字列との往復変換で内容が保たれる() {
        let original = Config {
            default_location: Some("大阪".to_string()),
            layout: "dashboard".to_string(),
            theme: "light".to_string(),
            animation: AnimationConfig {
                enabled: false,
                speed: 2.0,
                density: 0.3,
            },
        };
        let toml_str = original.to_toml_string().expect("シリアライズ失敗");
        let restored = Config::from_toml_str(&toml_str).expect("パース失敗");
        assert_eq!(restored, original);
    }

    // --- load_or_default / save ---

    #[test]
    fn 存在しないファイルからはデフォルト設定が返る() {
        let dir = make_temp_dir("missing");
        let path = dir.join("config.toml");
        let config = Config::load_or_default(&path).expect("読み込み失敗");
        assert_eq!(config, Config::default());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 保存した設定を読み込むと同じ内容になる() {
        let dir = make_temp_dir("roundtrip");
        // 親ディレクトリ自動作成も同時に確認するため深いパスにする
        let path = dir.join("nested").join("config.toml");
        let original = Config {
            default_location: Some("札幌".to_string()),
            ..Config::default()
        };
        original.save(&path).expect("保存失敗");
        let loaded = Config::load_or_default(&path).expect("読み込み失敗");
        assert_eq!(loaded, original);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 壊れた設定ファイルの読み込みはconfig_parseエラーになる() {
        let dir = make_temp_dir("broken");
        let path = dir.join("config.toml");
        fs::write(&path, "broken = [ toml").expect("書き込み失敗");
        let result = Config::load_or_default(&path);
        assert!(matches!(result, Err(AppError::ConfigParse(_))));
        fs::remove_dir_all(&dir).ok();
    }
}
