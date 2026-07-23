//! ratatuiによる画面描画。描画そのものは手動確認対象(自動テスト対象外)。

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, Tabs, Wrap};

use crate::animation::{ParticleColor, ParticleField};
use crate::jma_client::WarningKind;

use super::app::{AppState, Tab};
use super::format::{DayPane, UiData};
use super::layout::LayoutPreset;
use super::theme::Theme;

/// 通常表示のフッターヒント。
pub const HINT_NORMAL: &str = "Tab/←→: タブ切替  1/2: 直接選択  q: 終了";
/// デモ表示のフッターヒント。
pub const HINT_DEMO: &str = "n/Space: パターン切替  Tab/←→: タブ切替  q: 終了";

/// 1フレーム分の描画。`field` が `Some` なら背景アニメーションを描く。
pub fn draw(
    frame: &mut Frame,
    state: &AppState,
    data: &UiData,
    theme: &Theme,
    layout: LayoutPreset,
    field: Option<&ParticleField>,
    hint: &str,
) {
    // 背景: 単色で塗った上にパーティクルを重ね、さらに情報パネルを重ねる。
    // 落雷の瞬間は背景を一段明るくして画面フラッシュを演出する。
    let screen = frame.area();
    let flash = field.is_some_and(ParticleField::flash_active);
    let screen_bg = if flash {
        flash_bg(theme.screen_bg)
    } else {
        theme.screen_bg
    };
    frame.render_widget(
        Block::default().style(Style::default().bg(screen_bg)),
        screen,
    );
    if let Some(field) = field {
        draw_particles(frame, screen, field, screen_bg);
    }

    let panel = layout.panel_area(screen);
    frame.render_widget(Clear, panel);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(theme.border_type)
        .border_style(border_style(theme))
        .title(Line::from(vec![
            Span::styled("◈ ", Style::default().fg(theme.border)),
            Span::styled(
                format!("{} の天気", data.location_name),
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" ◈", Style::default().fg(theme.border)),
        ]))
        .title_alignment(ratatui::layout::Alignment::Center)
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

    let hint_line = Line::from(Span::styled(hint, Style::default().fg(theme.muted)));
    frame.render_widget(Paragraph::new(hint_line).centered(), rows[4]);
}

/// 枠線スタイル。`glow` テーマでは太字にしてネオンの発光感を出す。
fn border_style(theme: &Theme) -> Style {
    let base = Style::default().fg(theme.border);
    if theme.glow {
        base.add_modifier(Modifier::BOLD)
    } else {
        base
    }
}

/// 背景パーティクルの描画。色分類はテーマ非依存の固定配色。`bg` は
/// 落雷フラッシュを反映した実効背景色。
fn draw_particles(frame: &mut Frame, screen: Rect, field: &ParticleField, bg: ratatui::style::Color) {
    let buf = frame.buffer_mut();
    for (x, y, glyph, color) in field.glyphs() {
        if x >= screen.width || y >= screen.height {
            continue;
        }
        let style = Style::default().fg(particle_color(color)).bg(bg);
        // 稲妻ときらめきは太字で強く光らせる
        let style = if matches!(color, ParticleColor::Lightning | ParticleColor::Spark) {
            style.add_modifier(Modifier::BOLD)
        } else {
            style
        };
        buf.set_string(screen.x + x, screen.y + y, glyph, style);
    }
}

fn particle_color(color: ParticleColor) -> ratatui::style::Color {
    use ratatui::style::Color;
    match color {
        ParticleColor::Rain => Color::Rgb(90, 170, 255),
        ParticleColor::Snow => Color::Rgb(220, 235, 255),
        ParticleColor::Sun => Color::Rgb(255, 205, 60),
        ParticleColor::Spark => Color::Rgb(255, 255, 235),
        ParticleColor::Cloud => Color::Rgb(140, 150, 170),
        ParticleColor::Lightning => Color::Rgb(255, 250, 140),
    }
}

/// 落雷フラッシュ時の背景色。元の背景を白側へ持ち上げて発光感を出す。
fn flash_bg(base: ratatui::style::Color) -> ratatui::style::Color {
    use ratatui::style::Color;
    match base {
        Color::Rgb(r, g, b) => {
            let lift = |c: u8| c.saturating_add(48).max(56);
            Color::Rgb(lift(r), lift(g), lift(b).saturating_add(20))
        }
        other => other,
    }
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
    let tabs = Tabs::new(vec![" 今日・明日 ", " 週間予報 "])
        .select(selected)
        .divider(Span::styled("┃", Style::default().fg(theme.border)))
        .style(Style::default().fg(theme.muted))
        // 選択タブは背景をアクセント色で塗り、ネオンの点灯チップ風にする
        .highlight_style(
            Style::default()
                .bg(theme.accent)
                .fg(theme.panel_bg)
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
        .border_type(theme.border_type)
        .border_style(border_style(theme))
        .title(Span::styled(
            format!(" {} ", pane.title),
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ));
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
        render_with_field(state, data, layout, w, h, None)
    }

    fn render_with_field(
        state: &AppState,
        data: &UiData,
        layout: LayoutPreset,
        w: u16,
        h: u16,
        field: Option<&ParticleField>,
    ) -> String {
        let theme = Theme::from_name("neon").unwrap();
        let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
        terminal
            .draw(|frame| draw(frame, state, data, &theme, layout, field, HINT_NORMAL))
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

    #[test]
    fn 背景アニメーション付きでも全カテゴリで描画がパニックしない() {
        use crate::config::AnimationConfig;

        let data = ui_data_fixture();
        for category in [
            WeatherCategory::Sunny,
            WeatherCategory::Cloudy,
            WeatherCategory::Rain,
            WeatherCategory::Snow,
            WeatherCategory::Thunder,
        ] {
            let mut field =
                ParticleField::new(42, category, Some(70), &AnimationConfig::default(), 60, 20);
            for _ in 0..100 {
                field.tick();
            }
            // 端末サイズがフィールドより小さいケースも(クリップされて落ちないこと)
            for (w, h) in [(60, 20), (30, 10)] {
                render_with_field(
                    &AppState::new(),
                    &data,
                    LayoutPreset::Fullscreen,
                    w,
                    h,
                    Some(&field),
                );
            }
        }
    }
}
