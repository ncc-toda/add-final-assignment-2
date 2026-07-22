//! `--demo` 用のダミー表示データ。ネットワークを使わずに
//! 5パターンのアニメーションとパネル表示を確認できる。

use crate::jma_client::{Warning, WarningKind};
use crate::weather_code::WeatherCategory;

use super::format::{DayPane, PopRow, UiData, WarningBand, WeeklyRow};

/// デモで背景アニメの密度計算に使う降水確率(実データの代わり)。
pub fn demo_max_pop(category: WeatherCategory) -> Option<u8> {
    match category {
        WeatherCategory::Rain => Some(80),
        WeatherCategory::Snow => Some(70),
        WeatherCategory::Thunder => Some(90),
        WeatherCategory::Sunny | WeatherCategory::Cloudy => None,
    }
}

/// 指定カテゴリのダミー表示データを作る。
/// 雷では警報帯の表示確認のため警報付きのデータになる。
pub fn demo_ui_data(category: WeatherCategory) -> UiData {
    let warning_band = (category == WeatherCategory::Thunder).then(|| WarningBand {
        text: demo_warnings()
            .iter()
            .map(|w| w.name.clone())
            .collect::<Vec<_>>()
            .join("・"),
        kind: WarningKind::Warning,
    });

    let pops: Vec<PopRow> = [("00-06", 10u8), ("06-12", 20), ("12-18", 30), ("18-24", 40)]
        .into_iter()
        .map(|(label, percent)| PopRow {
            label: label.to_string(),
            percent: percent
                + demo_max_pop(category)
                    .map(|p| p.saturating_sub(40))
                    .unwrap_or(0),
        })
        .collect();

    let next = category.next();
    let day_panes = vec![
        DayPane {
            title: "今日 07/22".to_string(),
            weather_text: format!("{}(デモ)", category.jp_name()),
            category,
            temp_min: Some(25),
            temp_max: Some(31),
            pops: pops.clone(),
        },
        DayPane {
            title: "明日 07/23".to_string(),
            weather_text: format!("{}(デモ)", next.jp_name()),
            category: next,
            temp_min: Some(24),
            temp_max: Some(30),
            pops,
        },
    ];

    // 週間予報は5カテゴリを一巡させて全絵文字を確認できるようにする
    let weekly_rows = (0..7)
        .map(|i| {
            let cat = WeatherCategory::ALL[i % WeatherCategory::ALL.len()];
            WeeklyRow {
                date_label: format!("07/{:02}(デモ)", 23 + i),
                category: cat,
                pop_label: format!("{}0%", i + 1),
                temp_label: format!("{}/{}℃", 22 + i, 29 + i),
            }
        })
        .collect();

    UiData {
        location_name: format!("アニメデモ: {}", category.jp_name()),
        area_name: "デモ地方".to_string(),
        report_time_label: "ダミーデータ".to_string(),
        stale: false,
        warning_band,
        day_panes,
        weekly_rows,
    }
}

fn demo_warnings() -> Vec<Warning> {
    vec![
        Warning {
            code: "03".to_string(),
            name: "大雨警報".to_string(),
            kind: WarningKind::Warning,
        },
        Warning {
            code: "14".to_string(),
            name: "雷注意報".to_string(),
            kind: WarningKind::Advisory,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn デモデータはカテゴリ名を反映する() {
        let data = demo_ui_data(WeatherCategory::Snow);
        assert!(data.location_name.contains("雪"), "{}", data.location_name);
        assert_eq!(data.day_panes[0].category, WeatherCategory::Snow);
        assert_eq!(data.day_panes.len(), 2);
        assert_eq!(data.weekly_rows.len(), 7);
        assert!(!data.stale);
    }

    #[test]
    fn 雷のデモだけ警報帯が付く() {
        for cat in WeatherCategory::ALL {
            let data = demo_ui_data(cat);
            if cat == WeatherCategory::Thunder {
                let band = data.warning_band.expect("雷には警報帯があるはず");
                assert!(band.text.contains("大雨警報"), "{}", band.text);
                assert_eq!(band.kind, WarningKind::Warning);
            } else {
                assert_eq!(data.warning_band, None, "cat: {cat:?}");
            }
        }
    }

    #[test]
    fn 降水系カテゴリはデモ用降水確率を持つ() {
        assert_eq!(demo_max_pop(WeatherCategory::Rain), Some(80));
        assert_eq!(demo_max_pop(WeatherCategory::Snow), Some(70));
        assert_eq!(demo_max_pop(WeatherCategory::Thunder), Some(90));
        assert_eq!(demo_max_pop(WeatherCategory::Sunny), None);
        assert_eq!(demo_max_pop(WeatherCategory::Cloudy), None);
    }
}
