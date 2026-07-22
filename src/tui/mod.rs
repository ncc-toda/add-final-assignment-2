//! TUIのエントリポイント。端末のセットアップ・tickループ・後始末を担う。
//!
//! 描画内容は `view`、状態遷移は `app`、表示用データ整形は `format` を参照。

pub mod app;
pub mod format;
pub mod layout;
pub mod theme;
mod view;

use std::time::Duration;

use crossterm::event::{self, Event, KeyEventKind};
use ratatui::DefaultTerminal;

use crate::error::AppError;
use crate::jma_client::WeatherReport;

use app::AppState;
use layout::LayoutPreset;
use theme::Theme;

/// tick間隔。キー入力が無くてもこの周期で再描画する
/// (Task 7の背景アニメーションがこのループに乗る前提の設計)。
const TICK_INTERVAL: Duration = Duration::from_millis(50);

/// TUIを起動し、`q`/`Esc` で終了するまでブロックする。
pub fn run(
    location_name: &str,
    report: &WeatherReport,
    theme: &Theme,
    layout: LayoutPreset,
) -> Result<(), AppError> {
    let data = format::build_ui_data(location_name, report);
    let mut state = AppState::new();

    // ratatui::init はpanic時に端末を復元するフックも設定する
    let mut terminal = ratatui::init();
    let result = event_loop(&mut terminal, &mut state, &data, theme, layout);
    ratatui::restore();
    result
}

fn event_loop(
    terminal: &mut DefaultTerminal,
    state: &mut AppState,
    data: &format::UiData,
    theme: &Theme,
    layout: LayoutPreset,
) -> Result<(), AppError> {
    loop {
        terminal
            .draw(|frame| view::draw(frame, state, data, theme, layout))
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
