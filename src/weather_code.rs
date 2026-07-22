//! JMA天気コード→表示カテゴリ(晴れ/曇り/雨/雪/雷)のマッピング。
//!
//! TUIの天気アイコン表示(Task 6)と背景アニメーションの選択(Task 7)で共用する。

/// 天気の表示カテゴリ(基本5種)。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeatherCategory {
    Sunny,
    Cloudy,
    Rain,
    Snow,
    Thunder,
}

impl WeatherCategory {
    /// 全カテゴリ(デモ表示の切り替え順)。
    pub const ALL: [WeatherCategory; 5] = [
        WeatherCategory::Sunny,
        WeatherCategory::Cloudy,
        WeatherCategory::Rain,
        WeatherCategory::Snow,
        WeatherCategory::Thunder,
    ];

    /// カテゴリに対応する絵文字アイコン。
    pub fn emoji(self) -> &'static str {
        match self {
            WeatherCategory::Sunny => "☀",
            WeatherCategory::Cloudy => "☁",
            WeatherCategory::Rain => "☔",
            WeatherCategory::Snow => "❄",
            WeatherCategory::Thunder => "⚡",
        }
    }

    /// 日本語名(デモのパターン指定・表示に使う)。
    pub fn jp_name(self) -> &'static str {
        match self {
            WeatherCategory::Sunny => "晴れ",
            WeatherCategory::Cloudy => "曇り",
            WeatherCategory::Rain => "雨",
            WeatherCategory::Snow => "雪",
            WeatherCategory::Thunder => "雷",
        }
    }

    /// 日本語名からカテゴリを解決する(「晴」「曇」の短縮形も許容)。
    pub fn from_jp_name(name: &str) -> Option<Self> {
        match name {
            "晴れ" | "晴" => Some(WeatherCategory::Sunny),
            "曇り" | "曇" => Some(WeatherCategory::Cloudy),
            "雨" => Some(WeatherCategory::Rain),
            "雪" => Some(WeatherCategory::Snow),
            "雷" => Some(WeatherCategory::Thunder),
            _ => None,
        }
    }

    /// 次のカテゴリ(末尾は先頭に戻る)。デモの切り替え順。
    pub fn next(self) -> Self {
        let idx = Self::ALL.iter().position(|c| *c == self).unwrap_or(0);
        Self::ALL[(idx + 1) % Self::ALL.len()]
    }
}

/// 雷を伴う天気コード(JMA天気コード表で「雷」を含むもの)。
/// 先頭桁による大分類より優先して雷カテゴリに割り当てる。
const THUNDER_CODES: &[&str] = &["108", "140", "208", "240", "250", "350", "450"];

/// JMA天気コードをカテゴリに分類する。
///
/// 先頭桁が 1=晴れ / 2=曇り / 3=雨 / 4=雪。雷を伴うコードは雷を優先する。
/// 未知のコードは中立な曇りにフォールバックする(クラッシュさせない)。
pub fn categorize(code: &str) -> WeatherCategory {
    if THUNDER_CODES.contains(&code) {
        return WeatherCategory::Thunder;
    }
    match code.chars().next() {
        Some('1') => WeatherCategory::Sunny,
        Some('2') => WeatherCategory::Cloudy,
        Some('3') => WeatherCategory::Rain,
        Some('4') => WeatherCategory::Snow,
        _ => WeatherCategory::Cloudy,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 先頭桁で晴れ曇り雨雪に分類される() {
        assert_eq!(categorize("100"), WeatherCategory::Sunny);
        assert_eq!(categorize("112"), WeatherCategory::Sunny);
        assert_eq!(categorize("200"), WeatherCategory::Cloudy);
        assert_eq!(categorize("201"), WeatherCategory::Cloudy);
        assert_eq!(categorize("300"), WeatherCategory::Rain);
        assert_eq!(categorize("313"), WeatherCategory::Rain);
        assert_eq!(categorize("400"), WeatherCategory::Snow);
        assert_eq!(categorize("411"), WeatherCategory::Snow);
    }

    #[test]
    fn 雷を伴うコードは先頭桁より雷を優先する() {
        for code in ["108", "140", "208", "240", "250", "350", "450"] {
            assert_eq!(categorize(code), WeatherCategory::Thunder, "code: {code}");
        }
    }

    #[test]
    fn 未知のコードは曇りにフォールバックする() {
        assert_eq!(categorize("999"), WeatherCategory::Cloudy);
        assert_eq!(categorize(""), WeatherCategory::Cloudy);
        assert_eq!(categorize("abc"), WeatherCategory::Cloudy);
    }

    #[test]
    fn 日本語名とカテゴリを相互変換できる() {
        for cat in WeatherCategory::ALL {
            assert_eq!(WeatherCategory::from_jp_name(cat.jp_name()), Some(cat));
        }
        assert_eq!(
            WeatherCategory::from_jp_name("晴"),
            Some(WeatherCategory::Sunny)
        );
        assert_eq!(
            WeatherCategory::from_jp_name("曇"),
            Some(WeatherCategory::Cloudy)
        );
        assert_eq!(WeatherCategory::from_jp_name("台風"), None);
    }

    #[test]
    fn nextは全カテゴリを一巡して先頭に戻る() {
        let mut cat = WeatherCategory::Sunny;
        let mut seen = vec![cat];
        for _ in 0..4 {
            cat = cat.next();
            seen.push(cat);
        }
        assert_eq!(seen, WeatherCategory::ALL.to_vec());
        assert_eq!(cat.next(), WeatherCategory::Sunny);
    }

    #[test]
    fn 各カテゴリに絵文字が割り当てられている() {
        assert_eq!(WeatherCategory::Sunny.emoji(), "☀");
        assert_eq!(WeatherCategory::Cloudy.emoji(), "☁");
        assert_eq!(WeatherCategory::Rain.emoji(), "☔");
        assert_eq!(WeatherCategory::Snow.emoji(), "❄");
        assert_eq!(WeatherCategory::Thunder.emoji(), "⚡");
    }
}
