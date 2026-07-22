//! TUIのエントリポイント。端末のセットアップ・tickループ・後始末を担う。
//!
//! 描画内容は `view`、状態遷移は `app`、表示用データ整形は `format` を参照。

pub mod app;
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
use crate::weather_code::categorize;

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
            .draw(|frame| view::draw(frame, state, data, theme, layout, field.as_ref()))
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
