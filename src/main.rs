//! エントリーポイント。引数解析→設定読込→地点解決→予報取得→TUI起動を結線する。

use std::process::ExitCode;
use std::time::Duration;

use clap::Parser;

use weather::cache::{Cache, default_cache_dir};
use weather::cli::{self, Cli};
use weather::config::{Config, default_config_path};
use weather::error::AppError;
use weather::jma_client::JmaClient;
use weather::tui;
use weather::tui::layout::LayoutPreset;
use weather::tui::theme::Theme;

/// 予報キャッシュのTTL(要件4.3: 10分)。
const CACHE_TTL: Duration = Duration::from_secs(10 * 60);

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            if matches!(e, AppError::ConfigParse(_))
                && let Some(path) = default_config_path()
            {
                eprintln!("ファイルを修正するか削除してください:");
                eprintln!("  {}", path.display());
            }
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), AppError> {
    let args = Cli::parse();

    let config_path = default_config_path()
        .ok_or_else(|| AppError::Io("設定ディレクトリを特定できません".to_string()))?;

    if let Some(input) = args.set.as_deref() {
        let loc = cli::set_default_location(input, &config_path)?;
        println!("デフォルト地点を「{}」に設定しました。", loc.name);
        return Ok(());
    }

    let config = Config::load_or_default(&config_path)?;
    let loc = cli::resolve_target(args.location.as_deref(), &config)?;

    // 設定のタイポはAPI取得やTUI起動より前に検出して案内する
    let theme = Theme::from_name(&config.theme)?;
    let layout = LayoutPreset::from_name(&config.layout)?;

    let cache_dir = default_cache_dir()
        .ok_or_else(|| AppError::Io("キャッシュディレクトリを特定できません".to_string()))?;
    let client = JmaClient::new(Cache::new(cache_dir, CACHE_TTL));
    let report = client.fetch_weather(&loc.area_code)?;

    tui::run(&loc.name, &report, &theme, layout, &config.animation)
}
