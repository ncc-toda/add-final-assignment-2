//! TUIのエントリポイント。端末のセットアップ・tickループ・後始末を担う。
//!
//! 描画内容は `view`、状態遷移は `app`、表示用データ整形は `format` を参照。

pub mod app;
pub mod demo;
pub mod format;
pub mod layout;
pub mod theme;
mod view;

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crossterm::event::{self, Event, KeyEventKind};
use ratatui::DefaultTerminal;

use crate::animation::ParticleField;
use crate::config::AnimationConfig;
use crate::error::AppError;
use crate::jma_client::WeatherReport;
use crate::weather_code::{WeatherCategory, categorize};

use app::AppState;
use layout::LayoutPreset;
use theme::Theme;

/// tick間隔。キー入力が無くてもこの周期で再描画し、背景アニメーションを進める。
const TICK_INTERVAL: Duration = Duration::from_millis(50);

/// TUIを起動し、`q`/`Esc` で終了するまでブロックする。
pub fn run(
    location_name: &str,
    report: &WeatherReport,
    theme: &Theme,
    layout: LayoutPreset,
    animation: &AnimationConfig,
) -> Result<(), AppError> {
    let data = format::build_ui_data(location_name, report);
    let mut state = AppState::new();

    // ratatui::init はpanic時に端末を復元するフックも設定する
    let mut terminal = ratatui::init();
    let mut field = animation
        .enabled
        .then(|| build_field(report, animation, &terminal));
    let result = event_loop(&mut terminal, &mut state, &data, theme, layout, &mut field);
    ratatui::restore();
    result
}

/// 予報データから背景アニメーションを構築する。
/// 天気は今日(先頭区間)の天気コード、密度は今日の降水確率の最大値を使う。
fn build_field(
    report: &WeatherReport,
    animation: &AnimationConfig,
    terminal: &DefaultTerminal,
) -> ParticleField {
    let short = &report.bundle.short_term;
    let category = categorize(
        short
            .weather_periods
            .first()
            .map(|p| p.code.as_str())
            .unwrap_or(""),
    );
    let today = short.weather_periods.first().map(|p| date_part(&p.time));
    let max_pop = short
        .pops
        .iter()
        .filter(|p| today.is_none_or(|d| date_part(&p.time) == d))
        .map(|p| p.percent)
        .max();
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64 ^ d.as_secs())
        .unwrap_or(0x5EED);
    let size = terminal.size().unwrap_or(ratatui::layout::Size {
        width: 80,
        height: 24,
    });
    ParticleField::new(seed, category, max_pop, animation, size.width, size.height)
}

/// ISO 8601文字列の日付部分("YYYY-MM-DD")。
fn date_part(iso: &str) -> &str {
    iso.get(..10).unwrap_or(iso)
}

/// `--demo`: ダミーデータでTUIを起動し、`n`/`Space` で5パターンを順に切り替える。
/// アニメーションの確認が目的のため `enabled = false` でも背景を描画する。
pub fn run_demo(
    start: WeatherCategory,
    theme: &Theme,
    layout: LayoutPreset,
    animation: &AnimationConfig,
) -> Result<(), AppError> {
    let mut terminal = ratatui::init();
    let result = demo_loop(&mut terminal, start, theme, layout, animation);
    ratatui::restore();
    result
}

fn demo_loop(
    terminal: &mut DefaultTerminal,
    start: WeatherCategory,
    theme: &Theme,
    layout: LayoutPreset,
    animation: &AnimationConfig,
) -> Result<(), AppError> {
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64 ^ d.as_secs())
        .unwrap_or(0x5EED);
    let size = terminal.size().unwrap_or(ratatui::layout::Size {
        width: 80,
        height: 24,
    });

    let mut state = AppState::new();
    let mut category = start;
    let mut data = demo::demo_ui_data(category);
    let make_field = |category: WeatherCategory, seed: u64| {
        ParticleField::new(
            seed,
            category,
            demo::demo_max_pop(category),
            animation,
            size.width,
            size.height,
        )
    };
    let mut field = make_field(category, seed);

    loop {
        if let Ok(size) = terminal.size() {
            field.resize(size.width, size.height);
        }
        field.tick();

        terminal
            .draw(|frame| {
                view::draw(
                    frame,
                    &state,
                    &data,
                    theme,
                    layout,
                    Some(&field),
                    view::HINT_DEMO,
                )
            })
            .map_err(|e| AppError::Io(format!("TUI描画に失敗しました: {e}")))?;

        if event::poll(TICK_INTERVAL).map_err(|e| AppError::Io(e.to_string()))?
            && let Event::Key(key) = event::read().map_err(|e| AppError::Io(e.to_string()))?
            && key.kind == KeyEventKind::Press
        {
            match key.code {
                crossterm::event::KeyCode::Char('n' | 'N' | ' ') => {
                    category = category.next();
                    data = demo::demo_ui_data(category);
                    field = make_field(category, seed ^ category as u64);
                }
                other => state.on_key(other),
            }
        }

        if state.should_quit {
            return Ok(());
        }
    }
}

fn event_loop(
    terminal: &mut DefaultTerminal,
    state: &mut AppState,
    data: &format::UiData,
    theme: &Theme,
    layout: LayoutPreset,
    field: &mut Option<ParticleField>,
) -> Result<(), AppError> {
    loop {
        if let Some(field) = field.as_mut()
            && let Ok(size) = terminal.size()
        {
            field.resize(size.width, size.height);
            field.tick();
        }

        terminal
            .draw(|frame| {
                view::draw(
                    frame,
                    state,
                    data,
                    theme,
                    layout,
                    field.as_ref(),
                    view::HINT_NORMAL,
                )
            })
            .map_err(|e| AppError::Io(format!("TUI描画に失敗しました: {e}")))?;

        if event::poll(TICK_INTERVAL).map_err(|e| AppError::Io(e.to_string()))?
            && let Event::Key(key) = event::read().map_err(|e| AppError::Io(e.to_string()))?
            && key.kind == KeyEventKind::Press
        {
            state.on_key(key.code);
        }

        if state.should_quit {
            return Ok(());
        }
    }
}
