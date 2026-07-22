//! ratatuiによる画面描画。描画そのものは手動確認対象(自動テスト対象外)。

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, Tabs, Wrap};

use crate::jma_client::WarningKind;

use super::app::{AppState, Tab};
use super::format::{DayPane, UiData};
use super::layout::LayoutPreset;
use super::theme::Theme;

/// 1フレーム分の描画。
pub fn draw(
    frame: &mut Frame,
    state: &AppState,
    data: &UiData,
    theme: &Theme,
    layout: LayoutPreset,
) {
    // 背景(Task 7でアニメーション描画に差し替える。現状は単色プレースホルダ)
    let screen = frame.area();
    frame.render_widget(
        Block::default().style(Style::default().bg(theme.screen_bg)),
        screen,
    );

    let panel = layout.panel_area(screen);
    frame.render_widget(Clear, panel);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border))
        .title(format!(" {} の天気 ", data.location_name))
        .title_style(
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )
        .style(Style::default().bg(theme.panel_bg).fg(theme.panel_fg));
    let inner = block.inner(panel);
    frame.render_widget(block, panel);

    // パネル内の縦分割: 警報帯 / stale警告 / タブ / 本文 / フッター
    let warning_h = u16::from(data.warning_band.is_some());
    let stale_h = u16::from(data.stale);
    let rows = Layout::vertical([
        Constraint::Length(warning_h),
        Constraint::Length(stale_h),
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .split(inner);

    if let Some(band) = &data.warning_band {
        draw_warning_band(frame, rows[0], band.kind, &band.text, theme);
    }
    if data.stale {
        let line = Line::from(Span::styled(
            "⚠ ネットワークエラー: 期限切れキャッシュのデータを表示しています",
            Style::default().fg(theme.stale_fg),
        ));
        frame.render_widget(Paragraph::new(line), rows[1]);
    }

    draw_tabs(frame, rows[2], state.tab, theme);

    match state.tab {
        Tab::TodayTomorrow => draw_today_tomorrow(frame, rows[3], data, theme),
        Tab::Weekly => draw_weekly(frame, rows[3], data, theme),
    }

    let hint = Line::from(Span::styled(
        "Tab/←→: タブ切替  1/2: 直接選択  q: 終了",
        Style::default().fg(theme.muted),
    ));
    frame.render_widget(Paragraph::new(hint).centered(), rows[4]);
}

/// 警報・注意報の帯。種別で配色を変える(特別警報/警報は帯、注意報は文字色のみ)。
fn draw_warning_band(frame: &mut Frame, area: Rect, kind: WarningKind, text: &str, theme: &Theme) {
    let style = match kind {
        WarningKind::Emergency => Style::default()
            .bg(theme.emergency_bg)
            .fg(theme.band_fg)
            .add_modifier(Modifier::BOLD),
        WarningKind::Warning => Style::default()
            .bg(theme.warning_bg)
            .fg(theme.band_fg)
            .add_modifier(Modifier::BOLD),
        WarningKind::Advisory | WarningKind::Unknown => Style::default()
            .fg(theme.advisory_fg)
            .add_modifier(Modifier::BOLD),
    };
    let band = Paragraph::new(Line::from(format!("⚠ {text}"))).style(style);
    frame.render_widget(band, area);
}

fn draw_tabs(frame: &mut Frame, area: Rect, tab: Tab, theme: &Theme) {
    let selected = match tab {
        Tab::TodayTomorrow => 0,
        Tab::Weekly => 1,
    };
    let tabs = Tabs::new(vec!["今日・明日", "週間予報"])
        .select(selected)
        .style(Style::default().fg(theme.muted))
        .highlight_style(
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        );
    frame.render_widget(tabs, area);
}

/// 「今日・明日」タブ: ヘッダー1行 + 左右2ペイン。
fn draw_today_tomorrow(frame: &mut Frame, area: Rect, data: &UiData, theme: &Theme) {
    let rows = Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).split(area);

    let header = Line::from(Span::styled(
        format!("{}({})", data.area_name, data.report_time_label),
        Style::default().fg(theme.muted),
    ));
    frame.render_widget(Paragraph::new(header), rows[0]);

    let cols = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
        .spacing(1)
        .split(rows[1]);
    for (i, pane) in data.day_panes.iter().take(2).enumerate() {
        draw_day_pane(frame, cols[i], pane, theme);
    }
}

fn draw_day_pane(frame: &mut Frame, area: Rect, pane: &DayPane, theme: &Theme) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border))
        .title(format!(" {} ", pane.title));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines = vec![
        Line::from(vec![
            Span::styled(
                format!("{} ", pane.category.emoji()),
                Style::default().fg(theme.accent),
            ),
            Span::raw(pane.weather_text.clone()),
        ]),
        Line::from(format!(
            "気温: {} / {}",
            pane.temp_min
                .map(|t| format!("{t}℃"))
                .unwrap_or_else(|| "--".to_string()),
            pane.temp_max
                .map(|t| format!("{t}℃"))
                .unwrap_or_else(|| "--".to_string()),
        )),
    ];
    if !pane.pops.is_empty() {
        lines.push(Line::from(Span::styled(
            "降水確率",
            Style::default().fg(theme.muted),
        )));
        for pop in &pane.pops {
            lines.push(Line::from(format!("  {}  {:>3}%", pop.label, pop.percent)));
        }
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

/// 「週間予報」タブ: 日付・天気・降水確率・気温のテーブル。
fn draw_weekly(frame: &mut Frame, area: Rect, data: &UiData, theme: &Theme) {
    let header = Row::new(["日付", "天気", "降水", "気温(最低/最高)"]).style(
        Style::default()
            .fg(theme.muted)
            .add_modifier(Modifier::BOLD),
    );
    let rows = data.weekly_rows.iter().map(|row| {
        Row::new(vec![
            Cell::from(row.date_label.clone()),
            Cell::from(Line::from(vec![Span::styled(
                format!("{} ", row.category.emoji()),
                Style::default().fg(theme.accent),
            )])),
            Cell::from(row.pop_label.clone()),
            Cell::from(row.temp_label.clone()),
        ])
    });
    let table = Table::new(
        rows,
        [
            Constraint::Length(10),
            Constraint::Length(6),
            Constraint::Length(6),
            Constraint::Min(10),
        ],
    )
    .header(header)
    .column_spacing(2);
    frame.render_widget(table, area);
}

// NOTE: 見た目(配色・配置)は手動確認対象。ここでは描画がパニックしないこと、
// 主要な文言がバッファに含まれることのみをスモークテストで保証する。
#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use crate::jma_client::{WeatherBundle, WeatherReport};
    use crate::tui::format::{PopRow, WarningBand, WeeklyRow, build_ui_data};
    use crate::weather_code::WeatherCategory;

    use super::*;

    fn ui_data_fixture() -> UiData {
        UiData {
            location_name: "東京".to_string(),
            area_name: "東京地方".to_string(),
            report_time_label: "07/22 11:00 発表".to_string(),
            stale: true,
            warning_band: Some(WarningBand {
                text: "大雨警報・雷注意報".to_string(),
                kind: WarningKind::Warning,
            }),
            day_panes: vec![
                DayPane {
                    title: "今日 07/22".to_string(),
                    weather_text: "晴れ".to_string(),
                    category: WeatherCategory::Sunny,
                    temp_min: None,
                    temp_max: Some(34),
                    pops: vec![PopRow {
                        label: "12-18".to_string(),
                        percent: 10,
                    }],
                },
                DayPane {
                    title: "明日 07/23".to_string(),
                    weather_text: "曇り時々晴れ".to_string(),
                    category: WeatherCategory::Cloudy,
                    temp_min: Some(25),
                    temp_max: Some(31),
                    pops: vec![],
                },
            ],
            weekly_rows: vec![WeeklyRow {
                date_label: "07/23(木)".to_string(),
                category: WeatherCategory::Rain,
                pop_label: "20%".to_string(),
                temp_label: "25/31℃".to_string(),
            }],
        }
    }

    fn render(state: &AppState, data: &UiData, layout: LayoutPreset, w: u16, h: u16) -> String {
        let theme = Theme::from_name("dark").unwrap();
        let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
        terminal
            .draw(|frame| draw(frame, state, data, &theme, layout))
            .unwrap();
        format!("{:?}", terminal.backend().buffer())
    }

    #[test]
    fn 今日明日タブの描画に主要な文言が含まれる() {
        let rendered = render(
            &AppState::new(),
            &ui_data_fixture(),
            LayoutPreset::Fullscreen,
            100,
            40,
        );
        for expected in [
            "東京",
            "大雨警報・雷注意報",
            "期限切れキャッシュ",
            "今日・明日",
            "晴れ",
            "34℃",
            "12-18",
        ] {
            assert!(rendered.contains(expected), "missing: {expected}");
        }
    }

    #[test]
    fn 週間タブの描画に週間予報の行が含まれる() {
        let mut state = AppState::new();
        state.on_key(crossterm::event::KeyCode::Tab);
        let rendered = render(&state, &ui_data_fixture(), LayoutPreset::Dashboard, 100, 40);
        for expected in ["07/23(木)", "20%", "25/31℃"] {
            assert!(rendered.contains(expected), "missing: {expected}");
        }
    }

    #[test]
    fn 警報なし通常データでも各レイアウトと極小端末で描画がパニックしない() {
        let report = WeatherReport {
            bundle: WeatherBundle {
                short_term: crate::jma_client::ShortTermForecast {
                    report_datetime: "2026-07-22T11:00:00+09:00".to_string(),
                    area_name: "東京地方".to_string(),
                    weather_periods: vec![],
                    pops: vec![],
                    temps: vec![],
                },
                weekly: vec![],
                warnings: vec![],
            },
            from_stale_cache: false,
        };
        let data = build_ui_data("東京", &report);
        for layout in [LayoutPreset::Fullscreen, LayoutPreset::Dashboard] {
            for (w, h) in [(100, 40), (20, 6), (5, 2)] {
                render(&AppState::new(), &data, layout, w, h);
            }
        }
    }
}
