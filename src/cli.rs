//! コマンドライン引数定義(`clap`)と表示対象地点の解決ロジック。
//!
//! - `weather` : 設定のデフォルト地点を表示
//! - `weather <地名>` : 指定地点を表示
//! - `weather --set <地名>` : デフォルト地点を検証して保存し終了

use std::path::Path;

use clap::Parser;

use crate::config::Config;
use crate::error::AppError;
use crate::location::{self, Location};

/// コマンドライン引数。
#[derive(Debug, Parser)]
#[command(
    name = "weather",
    version,
    about = "ターミナルで天気予報を表示するTUIツール"
)]
pub struct Cli {
    /// 表示する地名(省略時は設定のデフォルト地点)
    pub location: Option<String>,

    /// デフォルト地点を設定して終了
    #[arg(long, value_name = "地名", conflicts_with = "location")]
    pub set: Option<String>,

    /// 背景アニメーションのデモ表示(パターン: 晴れ/曇り/雨/雪/雷。省略時は晴れから開始)
    #[arg(
        long,
        value_name = "パターン",
        num_args = 0..=1,
        default_missing_value = "晴れ",
        conflicts_with_all = ["location", "set"]
    )]
    pub demo: Option<String>,
}

/// `--demo` のパターン名を解決する。未知の名前は有効値を案内してエラー。
pub fn parse_demo_pattern(name: &str) -> Result<crate::weather_code::WeatherCategory, AppError> {
    crate::weather_code::WeatherCategory::from_jp_name(name).ok_or_else(|| {
        AppError::ConfigInvalid(format!(
            "未知のデモパターン「{name}」。有効値: 晴れ, 曇り, 雨, 雪, 雷"
        ))
    })
}

/// 表示対象の地点を解決する。
///
/// - 引数の地名があれば [`location::resolve`] で解決する(失敗は `LocationNotFound`)。
/// - なければ設定のデフォルト地点を使う。未設定なら `DefaultLocationNotSet`、
///   解決できなければ `DefaultLocationInvalid` を返す。
pub fn resolve_target(arg: Option<&str>, config: &Config) -> Result<Location, AppError> {
    match arg {
        Some(name) => location::resolve(name),
        None => {
            let default = config
                .default_location
                .as_deref()
                .ok_or(AppError::DefaultLocationNotSet)?;
            location::resolve(default).map_err(|e| match e {
                AppError::LocationNotFound { input, suggestions } => {
                    AppError::DefaultLocationInvalid { input, suggestions }
                }
                other => other,
            })
        }
    }
}

/// `--set <地名>` の処理: 地名を検証し、解決後の正規名をデフォルト地点として保存する。
///
/// 設定ファイルが壊れている場合は上書きせず `ConfigParse` エラーを返す。
/// 他の設定項目(テーマ等)は保持される。
pub fn set_default_location(input: &str, config_path: &Path) -> Result<Location, AppError> {
    let mut config = Config::load_or_default(config_path)?;
    let loc = location::resolve(input)?;
    config.default_location = Some(loc.name.clone());
    config.save(config_path)?;
    Ok(loc)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// テスト用の一意な一時ディレクトリを作成する。
    fn make_temp_dir(label: &str) -> PathBuf {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "weather_cli_test_{}_{}_{}",
            std::process::id(),
            label,
            n
        ));
        fs::create_dir_all(&dir).expect("一時ディレクトリの作成に失敗");
        dir
    }

    // --- 引数パース ---

    #[test]
    fn 引数なしはlocationもsetも空になる() {
        let cli = Cli::try_parse_from(["weather"]).expect("パース失敗");
        assert_eq!(cli.location, None);
        assert_eq!(cli.set, None);
    }

    #[test]
    fn 地名のpositional引数を受け取れる() {
        let cli = Cli::try_parse_from(["weather", "東京"]).expect("パース失敗");
        assert_eq!(cli.location.as_deref(), Some("東京"));
        assert_eq!(cli.set, None);
    }

    #[test]
    fn setオプションで地名を受け取れる() {
        let cli = Cli::try_parse_from(["weather", "--set", "大阪"]).expect("パース失敗");
        assert_eq!(cli.location, None);
        assert_eq!(cli.set.as_deref(), Some("大阪"));
    }

    #[test]
    fn positionalとsetの同時指定はエラーになる() {
        let result = Cli::try_parse_from(["weather", "東京", "--set", "大阪"]);
        assert!(result.is_err());
    }

    #[test]
    fn set単独で値なしはエラーになる() {
        let result = Cli::try_parse_from(["weather", "--set"]);
        assert!(result.is_err());
    }

    // --- --demo ---

    #[test]
    fn demoフラグは値なしなら晴れになる() {
        let cli = Cli::try_parse_from(["weather", "--demo"]).expect("パース失敗");
        assert_eq!(cli.demo.as_deref(), Some("晴れ"));
    }

    #[test]
    fn demoフラグはパターン名を受け取れる() {
        let cli = Cli::try_parse_from(["weather", "--demo", "雪"]).expect("パース失敗");
        assert_eq!(cli.demo.as_deref(), Some("雪"));
    }

    #[test]
    fn demoと地名やsetの同時指定はエラーになる() {
        assert!(Cli::try_parse_from(["weather", "東京", "--demo"]).is_err());
        assert!(Cli::try_parse_from(["weather", "--set", "東京", "--demo"]).is_err());
    }

    #[test]
    fn 有効なデモパターン名は全て解決できる() {
        use crate::weather_code::WeatherCategory;
        for cat in WeatherCategory::ALL {
            assert_eq!(parse_demo_pattern(cat.jp_name()).unwrap(), cat);
        }
    }

    #[test]
    fn 未知のデモパターン名はエラーになり有効値を案内する() {
        let msg = parse_demo_pattern("台風").unwrap_err().to_string();
        assert!(msg.contains("「台風」"), "msg: {msg}");
        assert!(msg.contains("晴れ, 曇り, 雨, 雪, 雷"), "msg: {msg}");
    }

    // --- resolve_target ---

    #[test]
    fn 引数の地名が優先して解決される() {
        let config = Config {
            default_location: Some("大阪".to_string()),
            ..Config::default()
        };
        let loc = resolve_target(Some("東京"), &config).expect("解決失敗");
        assert_eq!(loc.name, "東京");
        assert_eq!(loc.area_code, "130000");
    }

    #[test]
    fn 引数の地名が不正ならlocation_not_foundになる() {
        let config = Config::default();
        let err = resolve_target(Some("ロンドン"), &config).unwrap_err();
        assert!(
            matches!(err, AppError::LocationNotFound { .. }),
            "err: {err:?}"
        );
    }

    #[test]
    fn 引数なしなら設定のデフォルト地点を使う() {
        let config = Config {
            default_location: Some("大阪".to_string()),
            ..Config::default()
        };
        let loc = resolve_target(None, &config).expect("解決失敗");
        assert_eq!(loc.name, "大阪");
        assert_eq!(loc.area_code, "270000");
    }

    #[test]
    fn 引数なしでデフォルト未設定ならdefault_location_not_set() {
        let config = Config::default();
        let err = resolve_target(None, &config).unwrap_err();
        assert!(
            matches!(err, AppError::DefaultLocationNotSet),
            "err: {err:?}"
        );
    }

    #[test]
    fn 引数なしでデフォルトが不正ならdefault_location_invalidになる() {
        let config = Config {
            default_location: Some("とうきよう".to_string()),
            ..Config::default()
        };
        let err = resolve_target(None, &config).unwrap_err();
        match err {
            AppError::DefaultLocationInvalid { input, suggestions } => {
                assert_eq!(input, "とうきよう");
                assert!(
                    suggestions.contains(&"東京".to_string()),
                    "suggestions: {suggestions:?}"
                );
            }
            other => panic!("expected DefaultLocationInvalid, got: {other:?}"),
        }
    }

    // --- set_default_location ---

    #[test]
    fn エイリアスは正規名に解決して保存される() {
        let dir = make_temp_dir("set_alias");
        let path = dir.join("config.toml");
        // 既存のカスタム設定が保持されることも確認する
        let existing = Config {
            theme: "light".to_string(),
            ..Config::default()
        };
        existing.save(&path).expect("事前保存失敗");

        let loc = set_default_location("名古屋", &path).expect("保存失敗");
        assert_eq!(loc.name, "愛知");
        assert_eq!(loc.area_code, "230000");

        let saved = Config::load_or_default(&path).expect("読み込み失敗");
        assert_eq!(saved.default_location.as_deref(), Some("愛知"));
        assert_eq!(saved.theme, "light");
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 設定ファイルがなくてもデフォルト設定に地点を足して新規作成する() {
        let dir = make_temp_dir("set_new");
        let path = dir.join("config.toml");

        let loc = set_default_location("東京", &path).expect("保存失敗");
        assert_eq!(loc.name, "東京");

        let saved = Config::load_or_default(&path).expect("読み込み失敗");
        assert_eq!(saved.default_location.as_deref(), Some("東京"));
        assert_eq!(saved.theme, Config::default().theme);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 不正な地名では保存されずlocation_not_foundになる() {
        let dir = make_temp_dir("set_invalid");
        let path = dir.join("config.toml");

        let err = set_default_location("ロンドン", &path).unwrap_err();
        assert!(
            matches!(err, AppError::LocationNotFound { .. }),
            "err: {err:?}"
        );
        assert!(!path.exists(), "設定ファイルが作られてはいけない");
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 壊れた設定ファイルは上書きせずconfig_parseエラーになる() {
        let dir = make_temp_dir("set_broken");
        let path = dir.join("config.toml");
        let broken = "broken = [ toml";
        fs::write(&path, broken).expect("書き込み失敗");

        let err = set_default_location("東京", &path).unwrap_err();
        assert!(matches!(err, AppError::ConfigParse(_)), "err: {err:?}");
        let content = fs::read_to_string(&path).expect("読み込み失敗");
        assert_eq!(content, broken, "壊れたファイルが書き換えられてはいけない");
        fs::remove_dir_all(&dir).ok();
    }
}
