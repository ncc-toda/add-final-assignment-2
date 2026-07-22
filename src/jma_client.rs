//! JMA非公式APIクライアント(短期予報・週間予報・警報注意報の取得とパース)。
//!
//! - 予報は `forecast/{office}.json` 1本で短期([0])と週間([1])を同時取得する
//! - レスポンス内の複数区域は常に先頭(`areas[0]`)を代表として使う
//!   (MVPの都道府県庁所在地レベルでは先頭が代表区域・代表観測点になるため)
//! - 生JSONは内部のserde構造体で受け、表示向けに正規化したモデルへ変換して公開する
//! - 欠損があり得る値(週間予報の初日など)は `Option` で表現する

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::cache::{Cache, now_unix};
use crate::error::AppError;

/// JMA非公式APIのベースURL(テストではmockitoのURLに差し替える)。
pub const JMA_BASE_URL: &str = "https://www.jma.go.jp/bosai";

/// 非公式APIへの配慮として、ツール名・バージョン・連絡先を明示する(requirements 3.1)。
const USER_AGENT: &str = concat!(
    "weather-tui/",
    env!("CARGO_PKG_VERSION"),
    " (contact: oda.tetsuo@nsg.gr.jp)"
);

/// HTTPタイムアウト(ワンショットCLIのため無限待ちさせない)。
const HTTP_TIMEOUT: Duration = Duration::from_secs(10);

// ===== 公開モデル(正規化済み) =====

/// 取得結果一式。`from_stale_cache` が真なら期限切れキャッシュ由来(TUIで警告表示する)。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeatherReport {
    pub bundle: WeatherBundle,
    pub from_stale_cache: bool,
}

/// 表示に必要な天気データ一式。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeatherBundle {
    pub short_term: ShortTermForecast,
    pub weekly: Vec<WeeklyDay>,
    pub warnings: Vec<Warning>,
}

/// 短期予報(今日・明日・明後日、時間帯別)。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShortTermForecast {
    /// 発表時刻(ISO 8601文字列)
    pub report_datetime: String,
    /// 代表区域名(例: "東京地方")
    pub area_name: String,
    /// 時刻別の天気(天気コード+天気文)
    pub weather_periods: Vec<WeatherPeriod>,
    /// 時間帯別の降水確率
    pub pops: Vec<PopPeriod>,
    /// 時刻別の気温(代表アメダス観測点)
    pub temps: Vec<TempPeriod>,
}

/// ある時刻の天気(コードはアニメーションのマッピングにも使う)。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeatherPeriod {
    pub time: String,
    pub code: String,
    pub text: String,
}

/// ある時間帯の降水確率(%)。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PopPeriod {
    pub time: String,
    pub percent: u8,
}

/// ある時刻の気温(℃)。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TempPeriod {
    pub time: String,
    pub celsius: i32,
}

/// 週間予報の1日分。初日は気温・降水確率が欠損することがある。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeeklyDay {
    pub date: String,
    pub weather_code: String,
    pub pop: Option<u8>,
    pub temp_min: Option<i32>,
    pub temp_max: Option<i32>,
}

/// 発令中の警報・注意報。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Warning {
    pub code: String,
    pub name: String,
    pub kind: WarningKind,
}

/// 警報・注意報の種別(TUIの配色分けに使う)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WarningKind {
    /// 特別警報
    Emergency,
    /// 警報
    Warning,
    /// 注意報
    Advisory,
    /// コード表にない未知の種別
    Unknown,
}

/// 予報エンドポイント1回分のパース結果(キャッシュ保存の単位)。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct ForecastData {
    pub short_term: ShortTermForecast,
    pub weekly: Vec<WeeklyDay>,
}

// ===== 警報・注意報コード表(JMA公式コード表より) =====

const WARNING_CODE_TABLE: &[(&str, &str, WarningKind)] = &[
    ("02", "暴風雪警報", WarningKind::Warning),
    ("03", "大雨警報", WarningKind::Warning),
    ("04", "洪水警報", WarningKind::Warning),
    ("05", "暴風警報", WarningKind::Warning),
    ("06", "大雪警報", WarningKind::Warning),
    ("07", "波浪警報", WarningKind::Warning),
    ("08", "高潮警報", WarningKind::Warning),
    ("10", "大雨注意報", WarningKind::Advisory),
    ("12", "大雪注意報", WarningKind::Advisory),
    ("13", "風雪注意報", WarningKind::Advisory),
    ("14", "雷注意報", WarningKind::Advisory),
    ("15", "強風注意報", WarningKind::Advisory),
    ("16", "波浪注意報", WarningKind::Advisory),
    ("17", "融雪注意報", WarningKind::Advisory),
    ("18", "洪水注意報", WarningKind::Advisory),
    ("19", "高潮注意報", WarningKind::Advisory),
    ("20", "濃霧注意報", WarningKind::Advisory),
    ("21", "乾燥注意報", WarningKind::Advisory),
    ("22", "なだれ注意報", WarningKind::Advisory),
    ("23", "低温注意報", WarningKind::Advisory),
    ("24", "霜注意報", WarningKind::Advisory),
    ("25", "着氷注意報", WarningKind::Advisory),
    ("26", "着雪注意報", WarningKind::Advisory),
    ("32", "暴風雪特別警報", WarningKind::Emergency),
    ("33", "大雨特別警報", WarningKind::Emergency),
    ("35", "暴風特別警報", WarningKind::Emergency),
    ("36", "大雪特別警報", WarningKind::Emergency),
    ("37", "波浪特別警報", WarningKind::Emergency),
    ("38", "高潮特別警報", WarningKind::Emergency),
];

/// 警報コードから `Warning` を組み立てる。未知コードはフォールバック表記にする。
fn warning_from_code(code: &str) -> Warning {
    match WARNING_CODE_TABLE.iter().find(|(c, _, _)| *c == code) {
        Some((_, name, kind)) => Warning {
            code: code.to_string(),
            name: (*name).to_string(),
            kind: *kind,
        },
        None => Warning {
            code: code.to_string(),
            name: format!("警報・注意報(コード: {code})"),
            kind: WarningKind::Unknown,
        },
    }
}

// ===== 生JSON構造(serde受け皿。欠損はOptionで受けて変換時に検証する) =====

#[derive(Debug, Deserialize)]
struct RawForecastReport {
    #[serde(rename = "reportDatetime")]
    report_datetime: Option<String>,
    #[serde(rename = "timeSeries")]
    time_series: Option<Vec<RawTimeSeries>>,
}

#[derive(Debug, Deserialize)]
struct RawTimeSeries {
    #[serde(rename = "timeDefines")]
    time_defines: Vec<String>,
    areas: Vec<RawForecastArea>,
}

#[derive(Debug, Deserialize)]
struct RawForecastArea {
    #[serde(default)]
    area: RawAreaInfo,
    #[serde(rename = "weatherCodes")]
    weather_codes: Option<Vec<String>>,
    weathers: Option<Vec<String>>,
    pops: Option<Vec<String>>,
    temps: Option<Vec<String>>,
    #[serde(rename = "tempsMin")]
    temps_min: Option<Vec<String>>,
    #[serde(rename = "tempsMax")]
    temps_max: Option<Vec<String>>,
}

#[derive(Debug, Default, Deserialize)]
struct RawAreaInfo {
    #[serde(default)]
    name: String,
}

#[derive(Debug, Deserialize)]
struct RawWarningReport {
    #[serde(rename = "areaTypes")]
    area_types: Option<Vec<RawWarningAreaType>>,
}

#[derive(Debug, Deserialize)]
struct RawWarningAreaType {
    areas: Vec<RawWarningArea>,
}

#[derive(Debug, Deserialize)]
struct RawWarningArea {
    warnings: Option<Vec<RawWarningItem>>,
}

#[derive(Debug, Deserialize)]
struct RawWarningItem {
    code: Option<String>,
    status: Option<String>,
}

// ===== パース関数(生JSON → 正規化モデル) =====

fn api_format(msg: impl Into<String>) -> AppError {
    AppError::ApiFormat(msg.into())
}

/// 時刻列と値列の長さが一致することを検証する(構造異常の検出)。
fn ensure_same_len(times: &[String], values_len: usize, what: &str) -> Result<(), AppError> {
    if times.len() == values_len {
        Ok(())
    } else {
        Err(api_format(format!(
            "{what}の値の数({values_len})が時刻の数({})と一致しません",
            times.len()
        )))
    }
}

/// 空文字は`None`、それ以外は数値としてパースする(週間予報初日の欠損対応)。
fn parse_opt_number<T: std::str::FromStr>(value: &str, what: &str) -> Result<Option<T>, AppError> {
    if value.is_empty() {
        Ok(None)
    } else {
        value
            .parse()
            .map(Some)
            .map_err(|_| api_format(format!("{what}の値「{value}」を数値として解釈できません")))
    }
}

/// 代表区域(先頭)を取り出す。空なら構造異常。
fn first_area<'a>(ts: &'a RawTimeSeries, what: &str) -> Result<&'a RawForecastArea, AppError> {
    ts.areas
        .first()
        .ok_or_else(|| api_format(format!("{what}のareasが空です")))
}

/// 予報レスポンス(短期+週間の2要素配列)をパースする。
pub(crate) fn parse_forecast_json(json: &str) -> Result<ForecastData, AppError> {
    let reports: Vec<RawForecastReport> = serde_json::from_str(json)
        .map_err(|e| api_format(format!("予報JSONを解析できません: {e}")))?;
    let [short_raw, weekly_raw] = &reports[..] else {
        return Err(api_format(format!(
            "予報レスポンスは短期・週間の2要素の配列を想定していますが{}要素でした",
            reports.len()
        )));
    };
    Ok(ForecastData {
        short_term: convert_short_term(short_raw)?,
        weekly: convert_weekly(weekly_raw)?,
    })
}

/// 短期予報部分([0])を正規化する。timeSeriesは [0]=天気, [1]=降水確率, [2]=気温。
fn convert_short_term(raw: &RawForecastReport) -> Result<ShortTermForecast, AppError> {
    let ts = raw
        .time_series
        .as_deref()
        .ok_or_else(|| api_format("短期予報にtimeSeriesがありません"))?;

    let weather_ts = ts
        .first()
        .ok_or_else(|| api_format("短期予報のtimeSeriesが空です"))?;
    let weather_area = first_area(weather_ts, "短期予報(天気)")?;
    let codes = weather_area
        .weather_codes
        .as_deref()
        .ok_or_else(|| api_format("短期予報にweatherCodesがありません"))?;
    let texts = weather_area
        .weathers
        .as_deref()
        .ok_or_else(|| api_format("短期予報にweathersがありません"))?;
    ensure_same_len(&weather_ts.time_defines, codes.len(), "天気コード")?;
    ensure_same_len(&weather_ts.time_defines, texts.len(), "天気文")?;
    let weather_periods = weather_ts
        .time_defines
        .iter()
        .zip(codes)
        .zip(texts)
        .map(|((time, code), text)| WeatherPeriod {
            time: time.clone(),
            code: code.clone(),
            text: text.clone(),
        })
        .collect();

    let pop_ts = ts
        .get(1)
        .ok_or_else(|| api_format("短期予報に降水確率のtimeSeriesがありません"))?;
    let pop_area = first_area(pop_ts, "短期予報(降水確率)")?;
    let raw_pops = pop_area
        .pops
        .as_deref()
        .ok_or_else(|| api_format("短期予報にpopsがありません"))?;
    ensure_same_len(&pop_ts.time_defines, raw_pops.len(), "降水確率")?;
    let mut pops = Vec::new();
    for (time, value) in pop_ts.time_defines.iter().zip(raw_pops) {
        // まれに空文字が入ることがあるため、欠損スロットは黙って読み飛ばす
        if let Some(percent) = parse_opt_number(value, "降水確率")? {
            pops.push(PopPeriod {
                time: time.clone(),
                percent,
            });
        }
    }

    let temp_ts = ts
        .get(2)
        .ok_or_else(|| api_format("短期予報に気温のtimeSeriesがありません"))?;
    let temp_area = first_area(temp_ts, "短期予報(気温)")?;
    let raw_temps = temp_area
        .temps
        .as_deref()
        .ok_or_else(|| api_format("短期予報にtempsがありません"))?;
    ensure_same_len(&temp_ts.time_defines, raw_temps.len(), "気温")?;
    let mut temps = Vec::new();
    for (time, value) in temp_ts.time_defines.iter().zip(raw_temps) {
        if let Some(celsius) = parse_opt_number(value, "気温")? {
            temps.push(TempPeriod {
                time: time.clone(),
                celsius,
            });
        }
    }

    Ok(ShortTermForecast {
        report_datetime: raw
            .report_datetime
            .clone()
            .ok_or_else(|| api_format("短期予報にreportDatetimeがありません"))?,
        area_name: weather_area.area.name.clone(),
        weather_periods,
        pops,
        temps,
    })
}

/// 週間予報部分([1])を正規化する。timeSeriesは [0]=天気・降水確率, [1]=気温。
fn convert_weekly(raw: &RawForecastReport) -> Result<Vec<WeeklyDay>, AppError> {
    let ts = raw
        .time_series
        .as_deref()
        .ok_or_else(|| api_format("週間予報にtimeSeriesがありません"))?;

    let weather_ts = ts
        .first()
        .ok_or_else(|| api_format("週間予報のtimeSeriesが空です"))?;
    let weather_area = first_area(weather_ts, "週間予報(天気)")?;
    let codes = weather_area
        .weather_codes
        .as_deref()
        .ok_or_else(|| api_format("週間予報にweatherCodesがありません"))?;
    let raw_pops = weather_area
        .pops
        .as_deref()
        .ok_or_else(|| api_format("週間予報にpopsがありません"))?;
    ensure_same_len(&weather_ts.time_defines, codes.len(), "週間天気コード")?;
    ensure_same_len(&weather_ts.time_defines, raw_pops.len(), "週間降水確率")?;

    let temp_ts = ts
        .get(1)
        .ok_or_else(|| api_format("週間予報に気温のtimeSeriesがありません"))?;
    let temp_area = first_area(temp_ts, "週間予報(気温)")?;
    let temps_min = temp_area
        .temps_min
        .as_deref()
        .ok_or_else(|| api_format("週間予報にtempsMinがありません"))?;
    let temps_max = temp_area
        .temps_max
        .as_deref()
        .ok_or_else(|| api_format("週間予報にtempsMaxがありません"))?;
    ensure_same_len(&weather_ts.time_defines, temps_min.len(), "週間最低気温")?;
    ensure_same_len(&weather_ts.time_defines, temps_max.len(), "週間最高気温")?;

    let mut days = Vec::with_capacity(weather_ts.time_defines.len());
    for (i, date) in weather_ts.time_defines.iter().enumerate() {
        days.push(WeeklyDay {
            date: date.clone(),
            weather_code: codes[i].clone(),
            pop: parse_opt_number(&raw_pops[i], "週間降水確率")?,
            temp_min: parse_opt_number(&temps_min[i], "週間最低気温")?,
            temp_max: parse_opt_number(&temps_max[i], "週間最高気温")?,
        });
    }
    Ok(days)
}

/// 警報レスポンスをパースし、発令中(発表・継続)の警報・注意報のみを返す。
pub(crate) fn parse_warning_json(json: &str) -> Result<Vec<Warning>, AppError> {
    let report: RawWarningReport = serde_json::from_str(json)
        .map_err(|e| api_format(format!("警報JSONを解析できません: {e}")))?;
    let area = report
        .area_types
        .as_deref()
        .and_then(|types| types.first())
        .and_then(|t| t.areas.first())
        .ok_or_else(|| api_format("警報レスポンスに代表区域(areaTypes[0].areas[0])がありません"))?;

    let warnings = area
        .warnings
        .as_deref()
        .unwrap_or_default()
        .iter()
        .filter(|item| matches!(item.status.as_deref(), Some("発表") | Some("継続")))
        .filter_map(|item| item.code.as_deref().map(warning_from_code))
        .collect();
    Ok(warnings)
}

// ===== クライアント(取得フロー: キャッシュ → HTTP → フォールバック) =====

/// JMA APIクライアント。キャッシュとの連携・staleフォールバックを内包する。
pub struct JmaClient {
    base_url: String,
    cache: Cache,
    agent: ureq::Agent,
}

impl JmaClient {
    /// 本番URLでクライアントを作成する。
    pub fn new(cache: Cache) -> Self {
        Self::with_base_url(JMA_BASE_URL.to_string(), cache)
    }

    /// ベースURLを差し替えてクライアントを作成する(テスト用)。
    pub fn with_base_url(base_url: String, cache: Cache) -> Self {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .user_agent(USER_AGENT)
            .timeout_global(Some(HTTP_TIMEOUT))
            .build()
            .new_agent();
        Self {
            base_url,
            cache,
            agent,
        }
    }

    /// エリアコードに対する予報・警報の一式を取得する。
    ///
    /// エンドポイントごとに「期限内キャッシュ → HTTP取得 → 期限切れキャッシュへの
    /// フォールバック」の順で解決し、どちらか一方でも期限切れキャッシュ由来なら
    /// `from_stale_cache` を真にする。
    pub fn fetch_weather(&self, area_code: &str) -> Result<WeatherReport, AppError> {
        let (forecast, forecast_stale) = self.fetch_cached(
            &format!("forecast_{area_code}"),
            &format!(
                "{}/forecast/data/forecast/{}.json",
                self.base_url, area_code
            ),
            parse_forecast_json,
        )?;
        let (warnings, warnings_stale) = self.fetch_cached(
            &format!("warnings_{area_code}"),
            &format!("{}/warning/data/warning/{}.json", self.base_url, area_code),
            parse_warning_json,
        )?;
        Ok(WeatherReport {
            bundle: WeatherBundle {
                short_term: forecast.short_term,
                weekly: forecast.weekly,
                warnings,
            },
            from_stale_cache: forecast_stale || warnings_stale,
        })
    }

    /// 1エンドポイント分の取得フロー。戻り値の`bool`は「期限切れキャッシュ由来か」。
    ///
    /// パース失敗(`ApiFormat`)はAPI仕様変更にユーザーが気づけるよう、
    /// キャッシュにフォールバックせずそのままエラーにする(requirements 3.7)。
    fn fetch_cached<T>(
        &self,
        key: &str,
        url: &str,
        parse: fn(&str) -> Result<T, AppError>,
    ) -> Result<(T, bool), AppError>
    where
        T: Serialize + serde::de::DeserializeOwned,
    {
        if let Some(data) = self.cache.load_fresh(key, now_unix()) {
            return Ok((data, false));
        }
        match self.http_get(url) {
            Ok(body) => {
                let data = parse(&body)?;
                // キャッシュ保存の失敗は表示継続を優先して無視する
                self.cache.store(key, &data).ok();
                Ok((data, false))
            }
            Err(network_err) => match self.cache.load_any(key) {
                Some(data) => Ok((data, true)),
                None => Err(network_err),
            },
        }
    }

    /// GETリクエストを送りボディを文字列で返す。接続失敗・タイムアウト・
    /// HTTPステータス異常はすべて`Network`として扱う(フォールバック対象)。
    fn http_get(&self, url: &str) -> Result<String, AppError> {
        let mut response = self
            .agent
            .get(url)
            .call()
            .map_err(|e| AppError::Network(e.to_string()))?;
        response
            .body_mut()
            .read_to_string()
            .map_err(|e| AppError::Network(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::now_unix;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    // --- フィクスチャ(実際のJMAレスポンスを縮小したもの) ---

    /// forecast/{office}.json 正常系: [0]=短期(3日分), [1]=週間(7日分)
    const FIXTURE_FORECAST: &str = r#"[
      {
        "publishingOffice": "気象庁",
        "reportDatetime": "2026-07-22T11:00:00+09:00",
        "timeSeries": [
          {
            "timeDefines": ["2026-07-22T11:00:00+09:00", "2026-07-23T00:00:00+09:00", "2026-07-24T00:00:00+09:00"],
            "areas": [
              {
                "area": {"name": "東京地方", "code": "130010"},
                "weatherCodes": ["200", "101", "300"],
                "weathers": ["くもり", "晴れ 時々 くもり", "雨"],
                "winds": ["北の風", "南の風", "南の風 強く"]
              },
              {
                "area": {"name": "伊豆諸島北部", "code": "130020"},
                "weatherCodes": ["200", "200", "200"],
                "weathers": ["くもり", "くもり", "くもり"],
                "winds": ["北の風", "北の風", "北の風"]
              }
            ]
          },
          {
            "timeDefines": ["2026-07-22T12:00:00+09:00", "2026-07-22T18:00:00+09:00", "2026-07-23T00:00:00+09:00", "2026-07-23T06:00:00+09:00"],
            "areas": [
              {"area": {"name": "東京地方", "code": "130010"}, "pops": ["20", "10", "0", "30"]},
              {"area": {"name": "伊豆諸島北部", "code": "130020"}, "pops": ["10", "10", "10", "10"]}
            ]
          },
          {
            "timeDefines": ["2026-07-22T09:00:00+09:00", "2026-07-23T00:00:00+09:00", "2026-07-23T09:00:00+09:00"],
            "areas": [
              {"area": {"name": "東京", "code": "44132"}, "temps": ["32", "25", "33"]},
              {"area": {"name": "大島", "code": "44172"}, "temps": ["30", "24", "31"]}
            ]
          }
        ]
      },
      {
        "publishingOffice": "気象庁",
        "reportDatetime": "2026-07-22T11:00:00+09:00",
        "timeSeries": [
          {
            "timeDefines": ["2026-07-23T00:00:00+09:00", "2026-07-24T00:00:00+09:00", "2026-07-25T00:00:00+09:00", "2026-07-26T00:00:00+09:00", "2026-07-27T00:00:00+09:00", "2026-07-28T00:00:00+09:00", "2026-07-29T00:00:00+09:00"],
            "areas": [
              {
                "area": {"name": "東京地方", "code": "130010"},
                "weatherCodes": ["101", "200", "201", "101", "100", "201", "202"],
                "pops": ["", "20", "30", "20", "10", "30", "40"],
                "reliabilities": ["", "", "A", "A", "A", "B", "B"]
              }
            ]
          },
          {
            "timeDefines": ["2026-07-23T00:00:00+09:00", "2026-07-24T00:00:00+09:00", "2026-07-25T00:00:00+09:00", "2026-07-26T00:00:00+09:00", "2026-07-27T00:00:00+09:00", "2026-07-28T00:00:00+09:00", "2026-07-29T00:00:00+09:00"],
            "areas": [
              {
                "area": {"name": "東京", "code": "44132"},
                "tempsMin": ["", "24", "23", "24", "25", "24", "23"],
                "tempsMax": ["", "33", "32", "33", "34", "32", "31"]
              }
            ]
          }
        ]
      }
    ]"#;

    /// timeSeries自体が無い構造異常
    const FIXTURE_FORECAST_NO_TIMESERIES: &str = r#"[
      {"publishingOffice": "気象庁", "reportDatetime": "2026-07-22T11:00:00+09:00"},
      {"publishingOffice": "気象庁", "reportDatetime": "2026-07-22T11:00:00+09:00"}
    ]"#;

    /// areasが空の構造異常
    const FIXTURE_FORECAST_EMPTY_AREAS: &str = r#"[
      {
        "publishingOffice": "気象庁",
        "reportDatetime": "2026-07-22T11:00:00+09:00",
        "timeSeries": [{"timeDefines": ["2026-07-22T11:00:00+09:00"], "areas": []}]
      },
      {
        "publishingOffice": "気象庁",
        "reportDatetime": "2026-07-22T11:00:00+09:00",
        "timeSeries": [{"timeDefines": ["2026-07-23T00:00:00+09:00"], "areas": []}]
      }
    ]"#;

    /// 時刻列と値列の長さが食い違う構造異常(timeDefines 3つに対しweatherCodes 2つ)
    const FIXTURE_FORECAST_LENGTH_MISMATCH: &str = r#"[
      {
        "publishingOffice": "気象庁",
        "reportDatetime": "2026-07-22T11:00:00+09:00",
        "timeSeries": [
          {
            "timeDefines": ["2026-07-22T11:00:00+09:00", "2026-07-23T00:00:00+09:00", "2026-07-24T00:00:00+09:00"],
            "areas": [
              {
                "area": {"name": "東京地方", "code": "130010"},
                "weatherCodes": ["200", "101"],
                "weathers": ["くもり", "晴れ 時々 くもり"]
              }
            ]
          }
        ]
      },
      {"publishingOffice": "気象庁", "reportDatetime": "2026-07-22T11:00:00+09:00", "timeSeries": []}
    ]"#;

    /// warning/{office}.json 正常系: 発表・継続・解除が混在
    const FIXTURE_WARNING: &str = r#"{
      "publishingOffice": "気象庁",
      "reportDatetime": "2026-07-22T10:00:00+09:00",
      "headlineText": "",
      "areaTypes": [
        {
          "areas": [
            {
              "code": "130010",
              "warnings": [
                {"code": "14", "status": "継続"},
                {"code": "03", "status": "発表"},
                {"code": "10", "status": "解除"}
              ]
            },
            {"code": "130020", "warnings": [{"code": "05", "status": "発表"}]}
          ]
        },
        {"areas": [{"code": "1310100", "warnings": [{"code": "14", "status": "継続"}]}]}
      ]
    }"#;

    /// 発令中の警報・注意報がないケース(codeキー自体が無い)
    const FIXTURE_WARNING_NONE: &str = r#"{
      "publishingOffice": "気象庁",
      "reportDatetime": "2026-07-22T10:00:00+09:00",
      "headlineText": "",
      "areaTypes": [
        {"areas": [{"code": "130010", "warnings": [{"status": "発表警報・注意報はなし"}]}]},
        {"areas": []}
      ]
    }"#;

    /// コード表に無い未知コードの警報
    const FIXTURE_WARNING_UNKNOWN_CODE: &str = r#"{
      "publishingOffice": "気象庁",
      "reportDatetime": "2026-07-22T10:00:00+09:00",
      "areaTypes": [
        {"areas": [{"code": "130010", "warnings": [{"code": "99", "status": "発表"}]}]}
      ]
    }"#;

    // --- 短期予報パース ---

    #[test]
    fn 短期予報の天気と降水確率と気温が時刻に対応づく() {
        let data = parse_forecast_json(FIXTURE_FORECAST).expect("パース失敗");
        let st = &data.short_term;
        assert_eq!(st.report_datetime, "2026-07-22T11:00:00+09:00");
        assert_eq!(st.area_name, "東京地方");
        assert_eq!(
            st.weather_periods,
            vec![
                WeatherPeriod {
                    time: "2026-07-22T11:00:00+09:00".to_string(),
                    code: "200".to_string(),
                    text: "くもり".to_string(),
                },
                WeatherPeriod {
                    time: "2026-07-23T00:00:00+09:00".to_string(),
                    code: "101".to_string(),
                    text: "晴れ 時々 くもり".to_string(),
                },
                WeatherPeriod {
                    time: "2026-07-24T00:00:00+09:00".to_string(),
                    code: "300".to_string(),
                    text: "雨".to_string(),
                },
            ]
        );
        assert_eq!(
            st.pops,
            vec![
                PopPeriod {
                    time: "2026-07-22T12:00:00+09:00".to_string(),
                    percent: 20
                },
                PopPeriod {
                    time: "2026-07-22T18:00:00+09:00".to_string(),
                    percent: 10
                },
                PopPeriod {
                    time: "2026-07-23T00:00:00+09:00".to_string(),
                    percent: 0
                },
                PopPeriod {
                    time: "2026-07-23T06:00:00+09:00".to_string(),
                    percent: 30
                },
            ]
        );
        assert_eq!(
            st.temps,
            vec![
                TempPeriod {
                    time: "2026-07-22T09:00:00+09:00".to_string(),
                    celsius: 32
                },
                TempPeriod {
                    time: "2026-07-23T00:00:00+09:00".to_string(),
                    celsius: 25
                },
                TempPeriod {
                    time: "2026-07-23T09:00:00+09:00".to_string(),
                    celsius: 33
                },
            ]
        );
    }

    // --- 週間予報パース ---

    #[test]
    fn 週間予報が7日分日付順に取れる() {
        let data = parse_forecast_json(FIXTURE_FORECAST).expect("パース失敗");
        assert_eq!(data.weekly.len(), 7);
        assert_eq!(
            data.weekly[1],
            WeeklyDay {
                date: "2026-07-24T00:00:00+09:00".to_string(),
                weather_code: "200".to_string(),
                pop: Some(20),
                temp_min: Some(24),
                temp_max: Some(33),
            }
        );
        assert_eq!(data.weekly[6].date, "2026-07-29T00:00:00+09:00");
        assert_eq!(data.weekly[6].pop, Some(40));
    }

    #[test]
    fn 週間予報初日の空文字はnoneになる() {
        let data = parse_forecast_json(FIXTURE_FORECAST).expect("パース失敗");
        assert_eq!(
            data.weekly[0],
            WeeklyDay {
                date: "2026-07-23T00:00:00+09:00".to_string(),
                weather_code: "101".to_string(),
                pop: None,
                temp_min: None,
                temp_max: None,
            }
        );
    }

    // --- 構造異常系 ---

    #[test]
    fn 配列ですらないjsonはapiformatエラー() {
        let err = parse_forecast_json(r#"{"foo": 1}"#).unwrap_err();
        assert!(matches!(err, AppError::ApiFormat(_)), "実際: {err:?}");
    }

    #[test]
    fn timeseries欠落はapiformatエラー() {
        let err = parse_forecast_json(FIXTURE_FORECAST_NO_TIMESERIES).unwrap_err();
        assert!(matches!(err, AppError::ApiFormat(_)), "実際: {err:?}");
    }

    #[test]
    fn areasが空ならapiformatエラー() {
        let err = parse_forecast_json(FIXTURE_FORECAST_EMPTY_AREAS).unwrap_err();
        assert!(matches!(err, AppError::ApiFormat(_)), "実際: {err:?}");
    }

    #[test]
    fn 時刻列と値列の長さ不一致はapiformatエラー() {
        let err = parse_forecast_json(FIXTURE_FORECAST_LENGTH_MISMATCH).unwrap_err();
        assert!(matches!(err, AppError::ApiFormat(_)), "実際: {err:?}");
    }

    // --- 警報パース ---

    #[test]
    fn 発表と継続の警報のみ抽出され解除は除外される() {
        let warnings = parse_warning_json(FIXTURE_WARNING).expect("パース失敗");
        assert_eq!(
            warnings,
            vec![
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
            ]
        );
    }

    #[test]
    fn 発令中の警報がなければ空になる() {
        let warnings = parse_warning_json(FIXTURE_WARNING_NONE).expect("パース失敗");
        assert!(warnings.is_empty());
    }

    #[test]
    fn 未知の警報コードはフォールバック表記になる() {
        let warnings = parse_warning_json(FIXTURE_WARNING_UNKNOWN_CODE).expect("パース失敗");
        assert_eq!(
            warnings,
            vec![Warning {
                code: "99".to_string(),
                name: "警報・注意報(コード: 99)".to_string(),
                kind: WarningKind::Unknown,
            }]
        );
    }

    #[test]
    fn 警報jsonの構造異常はapiformatエラー() {
        let err = parse_warning_json(r#"{"foo": 1}"#).unwrap_err();
        assert!(matches!(err, AppError::ApiFormat(_)), "実際: {err:?}");
    }

    // --- 警報コード表 ---

    #[test]
    fn コード表から警報名と種別を引ける() {
        let w = warning_from_code("33");
        assert_eq!(w.name, "大雨特別警報");
        assert_eq!(w.kind, WarningKind::Emergency);
        let w = warning_from_code("05");
        assert_eq!(w.name, "暴風警報");
        assert_eq!(w.kind, WarningKind::Warning);
    }

    // --- 取得フロー(mockito + 一時ディレクトリキャッシュ) ---

    const TTL_SECS: u64 = 600;
    const FORECAST_PATH: &str = "/forecast/data/forecast/130000.json";
    const WARNING_PATH: &str = "/warning/data/warning/130000.json";

    fn make_temp_dir(label: &str) -> PathBuf {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        std::env::temp_dir().join(format!(
            "weather_jma_test_{}_{}_{}",
            std::process::id(),
            label,
            n
        ))
    }

    /// mockitoサーバー向きのクライアントと、事前投入用の同一ディレクトリCacheを作る。
    fn make_client(server_url: &str, label: &str) -> (JmaClient, Cache, PathBuf) {
        let dir = make_temp_dir(label);
        let cache = Cache::new(dir.clone(), Duration::from_secs(TTL_SECS));
        let client = JmaClient::with_base_url(server_url.to_string(), cache.clone());
        (client, cache, dir)
    }

    #[test]
    fn キャッシュミス時はhttp取得しパース結果をキャッシュに保存する() {
        let mut server = mockito::Server::new();
        let fmock = server
            .mock("GET", FORECAST_PATH)
            .with_status(200)
            .with_body(FIXTURE_FORECAST)
            .create();
        let wmock = server
            .mock("GET", WARNING_PATH)
            .with_status(200)
            .with_body(FIXTURE_WARNING)
            .create();
        let (client, cache, dir) = make_client(&server.url(), "miss");

        let report = client.fetch_weather("130000").expect("取得失敗");
        assert!(!report.from_stale_cache);
        assert_eq!(report.bundle.short_term.area_name, "東京地方");
        assert_eq!(report.bundle.weekly.len(), 7);
        assert_eq!(report.bundle.warnings.len(), 2);
        fmock.assert();
        wmock.assert();

        let cached_forecast: Option<ForecastData> = cache.load_fresh("forecast_130000", now_unix());
        let cached_warnings: Option<Vec<Warning>> = cache.load_fresh("warnings_130000", now_unix());
        assert!(cached_forecast.is_some(), "予報がキャッシュされていない");
        assert!(cached_warnings.is_some(), "警報がキャッシュされていない");
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 期限内キャッシュがあればhttpリクエストを送らない() {
        let mut server = mockito::Server::new();
        let fmock = server.mock("GET", FORECAST_PATH).expect(0).create();
        let wmock = server.mock("GET", WARNING_PATH).expect(0).create();
        let (client, cache, dir) = make_client(&server.url(), "fresh_hit");

        let forecast = parse_forecast_json(FIXTURE_FORECAST).expect("フィクスチャ不正");
        let warnings = parse_warning_json(FIXTURE_WARNING).expect("フィクスチャ不正");
        cache.store("forecast_130000", &forecast).expect("保存失敗");
        cache.store("warnings_130000", &warnings).expect("保存失敗");

        let report = client.fetch_weather("130000").expect("取得失敗");
        assert!(!report.from_stale_cache);
        assert_eq!(report.bundle.warnings, warnings);
        fmock.assert();
        wmock.assert();
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 通信失敗時は期限切れキャッシュにフォールバックしstaleフラグが立つ() {
        let mut server = mockito::Server::new();
        server.mock("GET", FORECAST_PATH).with_status(500).create();
        server.mock("GET", WARNING_PATH).with_status(500).create();
        let (client, cache, dir) = make_client(&server.url(), "stale_fallback");

        let forecast = parse_forecast_json(FIXTURE_FORECAST).expect("フィクスチャ不正");
        let warnings = parse_warning_json(FIXTURE_WARNING).expect("フィクスチャ不正");
        // 保存時刻を大昔にして期限切れキャッシュを作る
        cache
            .store_at("forecast_130000", &forecast, 1000)
            .expect("保存失敗");
        cache
            .store_at("warnings_130000", &warnings, 1000)
            .expect("保存失敗");

        let report = client.fetch_weather("130000").expect("フォールバック失敗");
        assert!(report.from_stale_cache);
        assert_eq!(report.bundle.short_term, forecast.short_term);
        assert_eq!(report.bundle.warnings, warnings);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 通信失敗かつキャッシュなしはネットワークエラー() {
        let mut server = mockito::Server::new();
        server.mock("GET", FORECAST_PATH).with_status(500).create();
        let (client, _cache, dir) = make_client(&server.url(), "no_cache");

        let err = client.fetch_weather("130000").unwrap_err();
        assert!(matches!(err, AppError::Network(_)), "実際: {err:?}");
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 構造異常レスポンスはキャッシュがあってもapiformatエラー() {
        let mut server = mockito::Server::new();
        server
            .mock("GET", FORECAST_PATH)
            .with_status(200)
            .with_body(r#"{"foo": 1}"#)
            .create();
        let (client, cache, dir) = make_client(&server.url(), "format_no_fallback");

        let forecast = parse_forecast_json(FIXTURE_FORECAST).expect("フィクスチャ不正");
        cache
            .store_at("forecast_130000", &forecast, 1000)
            .expect("保存失敗");

        let err = client.fetch_weather("130000").unwrap_err();
        assert!(matches!(err, AppError::ApiFormat(_)), "実際: {err:?}");
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn リクエストに連絡先入りのuser_agentが付く() {
        let mut server = mockito::Server::new();
        let fmock = server
            .mock("GET", FORECAST_PATH)
            .match_header("user-agent", USER_AGENT)
            .with_status(200)
            .with_body(FIXTURE_FORECAST)
            .create();
        let wmock = server
            .mock("GET", WARNING_PATH)
            .match_header("user-agent", USER_AGENT)
            .with_status(200)
            .with_body(FIXTURE_WARNING)
            .create();
        let (client, _cache, dir) = make_client(&server.url(), "user_agent");

        client.fetch_weather("130000").expect("取得失敗");
        fmock.assert();
        wmock.assert();
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 警報側だけ通信失敗ならそこだけフォールバックし全体はstaleになる() {
        let mut server = mockito::Server::new();
        server
            .mock("GET", FORECAST_PATH)
            .with_status(200)
            .with_body(FIXTURE_FORECAST)
            .create();
        server.mock("GET", WARNING_PATH).with_status(500).create();
        let (client, cache, dir) = make_client(&server.url(), "partial_stale");

        let warnings = parse_warning_json(FIXTURE_WARNING).expect("フィクスチャ不正");
        cache
            .store_at("warnings_130000", &warnings, 1000)
            .expect("保存失敗");

        let report = client.fetch_weather("130000").expect("取得失敗");
        assert!(report.from_stale_cache);
        assert_eq!(report.bundle.short_term.area_name, "東京地方");
        assert_eq!(report.bundle.warnings, warnings);
        fs::remove_dir_all(&dir).ok();
    }
}
