//! エントリーポイント。引数解析→設定読込→地点解決→予報取得までを結線する。
//!
//! 予報の表示は暫定のテキスト出力(TUIはTask 6で差し替え)。

use std::process::ExitCode;
use std::time::Duration;

use clap::Parser;

use weather::cache::{Cache, default_cache_dir};
use weather::cli::{self, Cli};
use weather::config::{Config, default_config_path};
use weather::error::AppError;
use weather::jma_client::{JmaClient, WeatherReport};

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

    let cache_dir = default_cache_dir()
        .ok_or_else(|| AppError::Io("キャッシュディレクトリを特定できません".to_string()))?;
    let client = JmaClient::new(Cache::new(cache_dir, CACHE_TTL));
    let report = client.fetch_weather(&loc.area_code)?;

    print_report(&loc.name, &report);
    Ok(())
}

/// 暫定のテキスト表示(Task 6でTUIに置き換える)。
fn print_report(location_name: &str, report: &WeatherReport) {
    let bundle = &report.bundle;
    let short = &bundle.short_term;

    println!("■ {location_name} の天気(暫定表示、TUIはTask 6で実装)");
    println!(
        "発表: {} / 区域: {}",
        short.report_datetime, short.area_name
    );
    if report.from_stale_cache {
        println!("⚠ ネットワークに接続できないため、期限切れキャッシュを表示しています");
    }

    if !bundle.warnings.is_empty() {
        println!("-- 警報・注意報 --");
        for warning in &bundle.warnings {
            println!("【{}】", warning.name);
        }
    }

    println!("-- 時間帯別の天気 --");
    for period in &short.weather_periods {
        println!("{}  {}", period.time, period.text);
    }
    if !short.pops.is_empty() {
        println!("-- 降水確率 --");
        for pop in &short.pops {
            println!("{}  {}%", pop.time, pop.percent);
        }
    }
    if !short.temps.is_empty() {
        println!("-- 気温 --");
        for temp in &short.temps {
            println!("{}  {}℃", temp.time, temp.celsius);
        }
    }

    println!("-- 週間予報 --");
    for day in &bundle.weekly {
        let pop = day
            .pop
            .map(|p| format!("{p}%"))
            .unwrap_or_else(|| "--".to_string());
        let min = day
            .temp_min
            .map(|t| format!("{t}℃"))
            .unwrap_or_else(|| "--".to_string());
        let max = day
            .temp_max
            .map(|t| format!("{t}℃"))
            .unwrap_or_else(|| "--".to_string());
        println!(
            "{}  天気コード:{}  降水:{}  {} / {}",
            day.date, day.weather_code, pop, min, max
        );
    }
}
