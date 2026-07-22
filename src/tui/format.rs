//! `WeatherReport` から描画用の表示モデルへの整形。
//!
//! 描画(view)に渡す直前のデータ加工をここに集約し、単体テスト対象とする。

use crate::jma_client::{ShortTermForecast, Warning, WarningKind, WeatherReport, WeeklyDay};
use crate::weather_code::{WeatherCategory, categorize};

/// 描画に必要なデータ一式(セッション中は不変)。
#[derive(Debug, Clone, PartialEq)]
pub struct UiData {
    /// 表示地点名(例: "東京")
    pub location_name: String,
    /// JMAの代表区域名(例: "東京地方")
    pub area_name: String,
    /// 発表時刻ラベル(例: "07/22 11:00 発表")
    pub report_time_label: String,
    /// 期限切れキャッシュ表示中か
    pub stale: bool,
    /// 警報・注意報の帯(発令なしなら `None`)
    pub warning_band: Option<WarningBand>,
    /// 今日・明日のペイン(最大2件)
    pub day_panes: Vec<DayPane>,
    /// 週間予報の行
    pub weekly_rows: Vec<WeeklyRow>,
}

/// 警報・注意報の帯表示。
#[derive(Debug, Clone, PartialEq)]
pub struct WarningBand {
    /// 連結済み表示文(例: "大雨警報・洪水警報")
    pub text: String,
    /// 最も重い種別(帯の配色に使う)
    pub kind: WarningKind,
}

/// 「今日・明日」タブの1日分ペイン。
#[derive(Debug, Clone, PartialEq)]
pub struct DayPane {
    /// ペインのタイトル(例: "今日 07/22")
    pub title: String,
    /// 天気文(例: "晴れ時々曇り")
    pub weather_text: String,
    /// 天気カテゴリ(絵文字アイコン用)
    pub category: WeatherCategory,
    /// 最低気温(取得できない時間帯は `None`)
    pub temp_min: Option<i32>,
    /// 最高気温
    pub temp_max: Option<i32>,
    /// 時間帯別降水確率
    pub pops: Vec<PopRow>,
}

/// 時間帯別降水確率の1行。
#[derive(Debug, Clone, PartialEq)]
pub struct PopRow {
    /// 時間帯ラベル(例: "12-18")
    pub label: String,
    pub percent: u8,
}

/// 週間予報タブの1行。
#[derive(Debug, Clone, PartialEq)]
pub struct WeeklyRow {
    /// 日付ラベル(例: "07/23(木)")
    pub date_label: String,
    /// 天気カテゴリ(絵文字アイコン用)
    pub category: WeatherCategory,
    /// 降水確率ラベル(例: "20%"、欠損は "--")
    pub pop_label: String,
    /// 気温ラベル(例: "25/31℃"、欠損は "--")
    pub temp_label: String,
}

/// `WeatherReport` 全体から表示モデルを構築する。
pub fn build_ui_data(location_name: &str, report: &WeatherReport) -> UiData {
    let bundle = &report.bundle;
    UiData {
        location_name: location_name.to_string(),
        area_name: bundle.short_term.area_name.clone(),
        report_time_label: format_report_datetime(&bundle.short_term.report_datetime),
        stale: report.from_stale_cache,
        warning_band: warning_band(&bundle.warnings),
        day_panes: day_panes(&bundle.short_term),
        weekly_rows: weekly_rows(&bundle.weekly),
    }
}

/// 発表時刻を "MM/DD HH:MM 発表" 形式に整形する。解析できなければ原文を返す。
pub fn format_report_datetime(iso: &str) -> String {
    match (parse_date(iso), iso.get(11..16)) {
        (Some((_, m, d)), Some(hm)) => format!("{m:02}/{d:02} {hm} 発表"),
        _ => iso.to_string(),
    }
}

/// 発令中の警報・注意報から帯表示を作る。発令なしなら `None`。
pub fn warning_band(warnings: &[Warning]) -> Option<WarningBand> {
    if warnings.is_empty() {
        return None;
    }
    let text = warnings
        .iter()
        .map(|w| w.name.as_str())
        .collect::<Vec<_>>()
        .join("・");
    let kind = warnings
        .iter()
        .map(|w| w.kind)
        .max_by_key(|k| severity_rank(*k))
        .expect("warnings is not empty");
    Some(WarningBand { text, kind })
}

/// 警報種別の重さ(大きいほど重い)。
fn severity_rank(kind: WarningKind) -> u8 {
    match kind {
        WarningKind::Emergency => 3,
        WarningKind::Warning => 2,
        WarningKind::Advisory => 1,
        WarningKind::Unknown => 0,
    }
}

/// 短期予報から今日・明日のペイン(最大2件)を作る。
///
/// 時刻別の降水確率・気温は日付ごとにグルーピングして割り当てる。
/// 気温はその日の値の最小/最大を最低/最高とみなす。1件しかない場合は
/// JMAの仕様上「日中の最高気温」のみの掲載なので最高気温として扱う。
pub fn day_panes(short: &ShortTermForecast) -> Vec<DayPane> {
    const TITLES: [&str; 2] = ["今日", "明日"];
    short
        .weather_periods
        .iter()
        .take(2)
        .enumerate()
        .map(|(i, period)| {
            let date = date_part(&period.time);
            let title = match parse_date(&period.time) {
                Some((_, m, d)) => format!("{} {m:02}/{d:02}", TITLES[i]),
                None => TITLES[i].to_string(),
            };

            let pops = short
                .pops
                .iter()
                .filter(|p| date_part(&p.time) == date)
                .map(|p| PopRow {
                    label: pop_slot_label(&p.time),
                    percent: p.percent,
                })
                .collect();

            let temps: Vec<i32> = short
                .temps
                .iter()
                .filter(|t| date_part(&t.time) == date)
                .map(|t| t.celsius)
                .collect();
            let (temp_min, temp_max) = match temps.len() {
                0 => (None, None),
                1 => (None, Some(temps[0])),
                _ => (temps.iter().min().copied(), temps.iter().max().copied()),
            };

            DayPane {
                title,
                weather_text: period.text.clone(),
                category: categorize(&period.code),
                temp_min,
                temp_max,
                pops,
            }
        })
        .collect()
}

/// 週間予報の各日を表示行に整形する。
pub fn weekly_rows(days: &[WeeklyDay]) -> Vec<WeeklyRow> {
    days.iter()
        .map(|day| {
            let date_label = match parse_date(&day.date) {
                Some((y, m, d)) => format!("{m:02}/{d:02}({})", weekday_jp(y, m, d)),
                None => day.date.clone(),
            };
            let pop_label = day
                .pop
                .map(|p| format!("{p}%"))
                .unwrap_or_else(|| "--".to_string());
            let temp_label = match (day.temp_min, day.temp_max) {
                (None, None) => "--".to_string(),
                (min, max) => format!(
                    "{}/{}℃",
                    min.map(|t| t.to_string()).unwrap_or_else(|| "--".into()),
                    max.map(|t| t.to_string()).unwrap_or_else(|| "--".into()),
                ),
            };
            WeeklyRow {
                date_label,
                category: categorize(&day.weather_code),
                pop_label,
                temp_label,
            }
        })
        .collect()
}

/// ISO 8601文字列の日付部分("YYYY-MM-DD")を取り出す。
fn date_part(iso: &str) -> &str {
    iso.get(..10).unwrap_or(iso)
}

/// ISO 8601文字列から (年, 月, 日) を取り出す。
fn parse_date(iso: &str) -> Option<(i32, u32, u32)> {
    let date = iso.get(..10)?;
    let mut parts = date.split('-');
    let y = parts.next()?.parse().ok()?;
    let m = parts.next()?.parse().ok()?;
    let d = parts.next()?.parse().ok()?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    Some((y, m, d))
}

/// 降水確率の時刻(時間帯の開始時刻)を "HH-HH" のラベルにする。
/// JMAの短期予報は6時間区切り(00/06/12/18時開始)。
fn pop_slot_label(iso: &str) -> String {
    let hour: u32 = iso.get(11..13).and_then(|h| h.parse().ok()).unwrap_or(0);
    format!("{hour:02}-{:02}", hour + 6)
}

/// ツェラーの公式による曜日計算(グレゴリオ暦)。
fn weekday_jp(year: i32, month: u32, day: u32) -> &'static str {
    let (y, m) = if month < 3 {
        (year - 1, month + 12)
    } else {
        (year, month)
    };
    let k = y.rem_euclid(100);
    let j = y.div_euclid(100);
    let h = (day as i32 + (13 * (m as i32 + 1)) / 5 + k + k / 4 + j / 4 + 5 * j).rem_euclid(7);
    // h: 0=土, 1=日, 2=月, ...
    ["土", "日", "月", "火", "水", "木", "金"][h as usize]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jma_client::{PopPeriod, TempPeriod, WeatherBundle, WeatherPeriod};

    fn short_term_fixture() -> ShortTermForecast {
        ShortTermForecast {
            report_datetime: "2026-07-22T11:00:00+09:00".to_string(),
            area_name: "東京地方".to_string(),
            weather_periods: vec![
                WeatherPeriod {
                    time: "2026-07-22T11:00:00+09:00".to_string(),
                    code: "100".to_string(),
                    text: "晴れ".to_string(),
                },
                WeatherPeriod {
                    time: "2026-07-23T00:00:00+09:00".to_string(),
                    code: "201".to_string(),
                    text: "曇り時々晴れ".to_string(),
                },
                WeatherPeriod {
                    time: "2026-07-24T00:00:00+09:00".to_string(),
                    code: "300".to_string(),
                    text: "雨".to_string(),
                },
            ],
            pops: vec![
                PopPeriod {
                    time: "2026-07-22T12:00:00+09:00".to_string(),
                    percent: 10,
                },
                PopPeriod {
                    time: "2026-07-22T18:00:00+09:00".to_string(),
                    percent: 20,
                },
                PopPeriod {
                    time: "2026-07-23T00:00:00+09:00".to_string(),
                    percent: 30,
                },
                PopPeriod {
                    time: "2026-07-23T06:00:00+09:00".to_string(),
                    percent: 40,
                },
            ],
            temps: vec![
                TempPeriod {
                    time: "2026-07-22T09:00:00+09:00".to_string(),
                    celsius: 34,
                },
                TempPeriod {
                    time: "2026-07-23T00:00:00+09:00".to_string(),
                    celsius: 25,
                },
                TempPeriod {
                    time: "2026-07-23T09:00:00+09:00".to_string(),
                    celsius: 31,
                },
            ],
        }
    }

    #[test]
    fn 発表時刻は月日と時分に整形される() {
        assert_eq!(
            format_report_datetime("2026-07-22T11:00:00+09:00"),
            "07/22 11:00 発表"
        );
    }

    #[test]
    fn 解析できない発表時刻は原文のまま返す() {
        assert_eq!(format_report_datetime("invalid"), "invalid");
    }

    #[test]
    fn 警報なしなら帯は作られない() {
        assert_eq!(warning_band(&[]), None);
    }

    #[test]
    fn 警報帯は名前を連結し最も重い種別を採用する() {
        let warnings = vec![
            Warning {
                code: "14".to_string(),
                name: "雷注意報".to_string(),
                kind: WarningKind::Advisory,
            },
            Warning {
                code: "03".to_string(),
                name: "大雨警報".to_string(),
                kind: WarningKind::Warning,
            },
        ];
        let band = warning_band(&warnings).unwrap();
        assert_eq!(band.text, "雷注意報・大雨警報");
        assert_eq!(band.kind, WarningKind::Warning);
    }

    #[test]
    fn 特別警報は警報より優先される() {
        let warnings = vec![
            Warning {
                code: "03".to_string(),
                name: "大雨警報".to_string(),
                kind: WarningKind::Warning,
            },
            Warning {
                code: "33".to_string(),
                name: "大雨特別警報".to_string(),
                kind: WarningKind::Emergency,
            },
        ];
        assert_eq!(
            warning_band(&warnings).unwrap().kind,
            WarningKind::Emergency
        );
    }

    #[test]
    fn 今日明日の2ペインが日付ごとのデータで作られる() {
        let panes = day_panes(&short_term_fixture());
        assert_eq!(panes.len(), 2);

        let today = &panes[0];
        assert_eq!(today.title, "今日 07/22");
        assert_eq!(today.weather_text, "晴れ");
        assert_eq!(today.category, WeatherCategory::Sunny);
        assert_eq!(
            today.pops,
            vec![
                PopRow {
                    label: "12-18".to_string(),
                    percent: 10
                },
                PopRow {
                    label: "18-24".to_string(),
                    percent: 20
                },
            ]
        );
        // 今日は気温1件のみ → 最高気温として扱い最低はなし
        assert_eq!(today.temp_min, None);
        assert_eq!(today.temp_max, Some(34));

        let tomorrow = &panes[1];
        assert_eq!(tomorrow.title, "明日 07/23");
        assert_eq!(tomorrow.category, WeatherCategory::Cloudy);
        assert_eq!(
            tomorrow.pops,
            vec![
                PopRow {
                    label: "00-06".to_string(),
                    percent: 30
                },
                PopRow {
                    label: "06-12".to_string(),
                    percent: 40
                },
            ]
        );
        assert_eq!(tomorrow.temp_min, Some(25));
        assert_eq!(tomorrow.temp_max, Some(31));
    }

    #[test]
    fn 週間予報行は曜日付き日付と欠損時のプレースホルダを持つ() {
        let days = vec![
            WeeklyDay {
                date: "2026-07-23T00:00:00+09:00".to_string(),
                weather_code: "201".to_string(),
                pop: Some(20),
                temp_min: Some(25),
                temp_max: Some(31),
            },
            WeeklyDay {
                date: "2026-07-24T00:00:00+09:00".to_string(),
                weather_code: "300".to_string(),
                pop: None,
                temp_min: None,
                temp_max: None,
            },
        ];
        let rows = weekly_rows(&days);
        assert_eq!(rows.len(), 2);

        // 2026-07-23 は木曜日
        assert_eq!(rows[0].date_label, "07/23(木)");
        assert_eq!(rows[0].category, WeatherCategory::Cloudy);
        assert_eq!(rows[0].pop_label, "20%");
        assert_eq!(rows[0].temp_label, "25/31℃");

        assert_eq!(rows[1].date_label, "07/24(金)");
        assert_eq!(rows[1].pop_label, "--");
        assert_eq!(rows[1].temp_label, "--");
    }

    #[test]
    fn ui_data全体が組み立てられる() {
        let report = WeatherReport {
            bundle: WeatherBundle {
                short_term: short_term_fixture(),
                weekly: vec![],
                warnings: vec![],
            },
            from_stale_cache: true,
        };
        let data = build_ui_data("東京", &report);
        assert_eq!(data.location_name, "東京");
        assert_eq!(data.area_name, "東京地方");
        assert_eq!(data.report_time_label, "07/22 11:00 発表");
        assert!(data.stale);
        assert_eq!(data.warning_band, None);
        assert_eq!(data.day_panes.len(), 2);
        assert!(data.weekly_rows.is_empty());
    }
}
