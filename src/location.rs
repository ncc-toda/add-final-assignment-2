//! 地名からJMAエリアコードへの解決、および曖昧一致時のサジェスト。
//!
//! 都道府県庁所在地レベル47地点の静的マッピングデータを同梱し、
//! 完全一致検索と `strsim` による類似地名サジェストを提供する。

use crate::error::AppError;

/// 地名解決の結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    /// 表示名(例: "東京")
    pub name: String,
    /// JMAのofficeコード(例: "130000")
    pub area_code: String,
}

/// 静的マッピングの1エントリ。
struct LocationEntry {
    /// 表示名
    name: &'static str,
    /// 別名(正式都道府県名・県庁所在地名・ひらがな・ローマ字など)
    aliases: &'static [&'static str],
    /// JMAのofficeコード(<https://www.jma.go.jp/bosai/common/const/area.json> の `offices` 由来)
    area_code: &'static str,
}

/// サジェスト採用の類似度しきい値(normalized Levenshtein)。
const SUGGESTION_THRESHOLD: f64 = 0.5;
/// サジェストの最大件数。
const MAX_SUGGESTIONS: usize = 3;

/// 都道府県庁所在地レベル47地点の静的マッピング。
///
/// 北海道はMVPでは札幌(石狩・空知・後志地方 = 016000)を代表とする。
/// 鹿児島は奄美地方を除く 460100、沖縄は沖縄本島地方 471000 を代表とする。
const LOCATIONS: [LocationEntry; 47] = [
    LocationEntry {
        name: "北海道",
        aliases: &[
            "札幌",
            "札幌市",
            "さっぽろ",
            "ほっかいどう",
            "hokkaido",
            "sapporo",
        ],
        area_code: "016000",
    },
    LocationEntry {
        name: "青森",
        aliases: &["青森県", "青森市", "あおもり", "aomori"],
        area_code: "020000",
    },
    LocationEntry {
        name: "岩手",
        aliases: &[
            "岩手県",
            "盛岡",
            "盛岡市",
            "いわて",
            "もりおか",
            "iwate",
            "morioka",
        ],
        area_code: "030000",
    },
    LocationEntry {
        name: "宮城",
        aliases: &[
            "宮城県",
            "仙台",
            "仙台市",
            "みやぎ",
            "せんだい",
            "miyagi",
            "sendai",
        ],
        area_code: "040000",
    },
    LocationEntry {
        name: "秋田",
        aliases: &["秋田県", "秋田市", "あきた", "akita"],
        area_code: "050000",
    },
    LocationEntry {
        name: "山形",
        aliases: &["山形県", "山形市", "やまがた", "yamagata"],
        area_code: "060000",
    },
    LocationEntry {
        name: "福島",
        aliases: &["福島県", "福島市", "ふくしま", "fukushima"],
        area_code: "070000",
    },
    LocationEntry {
        name: "茨城",
        aliases: &[
            "茨城県",
            "水戸",
            "水戸市",
            "いばらき",
            "みと",
            "ibaraki",
            "mito",
        ],
        area_code: "080000",
    },
    LocationEntry {
        name: "栃木",
        aliases: &[
            "栃木県",
            "宇都宮",
            "宇都宮市",
            "とちぎ",
            "うつのみや",
            "tochigi",
            "utsunomiya",
        ],
        area_code: "090000",
    },
    LocationEntry {
        name: "群馬",
        aliases: &[
            "群馬県",
            "前橋",
            "前橋市",
            "ぐんま",
            "まえばし",
            "gunma",
            "maebashi",
        ],
        area_code: "100000",
    },
    LocationEntry {
        name: "埼玉",
        aliases: &["埼玉県", "さいたま", "さいたま市", "saitama"],
        area_code: "110000",
    },
    LocationEntry {
        name: "千葉",
        aliases: &["千葉県", "千葉市", "ちば", "chiba"],
        area_code: "120000",
    },
    LocationEntry {
        name: "東京",
        aliases: &["東京都", "とうきょう", "tokyo"],
        area_code: "130000",
    },
    LocationEntry {
        name: "神奈川",
        aliases: &[
            "神奈川県",
            "横浜",
            "横浜市",
            "かながわ",
            "よこはま",
            "kanagawa",
            "yokohama",
        ],
        area_code: "140000",
    },
    LocationEntry {
        name: "新潟",
        aliases: &["新潟県", "新潟市", "にいがた", "niigata"],
        area_code: "150000",
    },
    LocationEntry {
        name: "富山",
        aliases: &["富山県", "富山市", "とやま", "toyama"],
        area_code: "160000",
    },
    LocationEntry {
        name: "石川",
        aliases: &[
            "石川県",
            "金沢",
            "金沢市",
            "いしかわ",
            "かなざわ",
            "ishikawa",
            "kanazawa",
        ],
        area_code: "170000",
    },
    LocationEntry {
        name: "福井",
        aliases: &["福井県", "福井市", "ふくい", "fukui"],
        area_code: "180000",
    },
    LocationEntry {
        name: "山梨",
        aliases: &[
            "山梨県",
            "甲府",
            "甲府市",
            "やまなし",
            "こうふ",
            "yamanashi",
            "kofu",
        ],
        area_code: "190000",
    },
    LocationEntry {
        name: "長野",
        aliases: &["長野県", "長野市", "ながの", "nagano"],
        area_code: "200000",
    },
    LocationEntry {
        name: "岐阜",
        aliases: &["岐阜県", "岐阜市", "ぎふ", "gifu"],
        area_code: "210000",
    },
    LocationEntry {
        name: "静岡",
        aliases: &["静岡県", "静岡市", "しずおか", "shizuoka"],
        area_code: "220000",
    },
    LocationEntry {
        name: "愛知",
        aliases: &[
            "愛知県",
            "名古屋",
            "名古屋市",
            "あいち",
            "なごや",
            "aichi",
            "nagoya",
        ],
        area_code: "230000",
    },
    LocationEntry {
        name: "三重",
        aliases: &["三重県", "津", "津市", "みえ", "mie", "tsu"],
        area_code: "240000",
    },
    LocationEntry {
        name: "滋賀",
        aliases: &[
            "滋賀県",
            "大津",
            "大津市",
            "しが",
            "おおつ",
            "shiga",
            "otsu",
        ],
        area_code: "250000",
    },
    LocationEntry {
        name: "京都",
        aliases: &["京都府", "京都市", "きょうと", "kyoto"],
        area_code: "260000",
    },
    LocationEntry {
        name: "大阪",
        aliases: &["大阪府", "大阪市", "おおさか", "osaka"],
        area_code: "270000",
    },
    LocationEntry {
        name: "兵庫",
        aliases: &[
            "兵庫県",
            "神戸",
            "神戸市",
            "ひょうご",
            "こうべ",
            "hyogo",
            "kobe",
        ],
        area_code: "280000",
    },
    LocationEntry {
        name: "奈良",
        aliases: &["奈良県", "奈良市", "なら", "nara"],
        area_code: "290000",
    },
    LocationEntry {
        name: "和歌山",
        aliases: &["和歌山県", "和歌山市", "わかやま", "wakayama"],
        area_code: "300000",
    },
    LocationEntry {
        name: "鳥取",
        aliases: &["鳥取県", "鳥取市", "とっとり", "tottori"],
        area_code: "310000",
    },
    LocationEntry {
        name: "島根",
        aliases: &[
            "島根県",
            "松江",
            "松江市",
            "しまね",
            "まつえ",
            "shimane",
            "matsue",
        ],
        area_code: "320000",
    },
    LocationEntry {
        name: "岡山",
        aliases: &["岡山県", "岡山市", "おかやま", "okayama"],
        area_code: "330000",
    },
    LocationEntry {
        name: "広島",
        aliases: &["広島県", "広島市", "ひろしま", "hiroshima"],
        area_code: "340000",
    },
    LocationEntry {
        name: "山口",
        aliases: &["山口県", "山口市", "やまぐち", "yamaguchi"],
        area_code: "350000",
    },
    LocationEntry {
        name: "徳島",
        aliases: &["徳島県", "徳島市", "とくしま", "tokushima"],
        area_code: "360000",
    },
    LocationEntry {
        name: "香川",
        aliases: &[
            "香川県",
            "高松",
            "高松市",
            "かがわ",
            "たかまつ",
            "kagawa",
            "takamatsu",
        ],
        area_code: "370000",
    },
    LocationEntry {
        name: "愛媛",
        aliases: &[
            "愛媛県",
            "松山",
            "松山市",
            "えひめ",
            "まつやま",
            "ehime",
            "matsuyama",
        ],
        area_code: "380000",
    },
    LocationEntry {
        name: "高知",
        aliases: &["高知県", "高知市", "こうち", "kochi"],
        area_code: "390000",
    },
    LocationEntry {
        name: "福岡",
        aliases: &["福岡県", "福岡市", "ふくおか", "fukuoka"],
        area_code: "400000",
    },
    LocationEntry {
        name: "佐賀",
        aliases: &["佐賀県", "佐賀市", "さが", "saga"],
        area_code: "410000",
    },
    LocationEntry {
        name: "長崎",
        aliases: &["長崎県", "長崎市", "ながさき", "nagasaki"],
        area_code: "420000",
    },
    LocationEntry {
        name: "熊本",
        aliases: &["熊本県", "熊本市", "くまもと", "kumamoto"],
        area_code: "430000",
    },
    LocationEntry {
        name: "大分",
        aliases: &["大分県", "大分市", "おおいた", "oita"],
        area_code: "440000",
    },
    LocationEntry {
        name: "宮崎",
        aliases: &["宮崎県", "宮崎市", "みやざき", "miyazaki"],
        area_code: "450000",
    },
    LocationEntry {
        name: "鹿児島",
        aliases: &["鹿児島県", "鹿児島市", "かごしま", "kagoshima"],
        area_code: "460100",
    },
    LocationEntry {
        name: "沖縄",
        aliases: &[
            "沖縄県",
            "那覇",
            "那覇市",
            "おきなわ",
            "なは",
            "okinawa",
            "naha",
        ],
        area_code: "471000",
    },
];

impl LocationEntry {
    fn to_location(&self) -> Location {
        Location {
            name: self.name.to_string(),
            area_code: self.area_code.to_string(),
        }
    }

    /// 表示名 + 全エイリアスを列挙する。
    fn candidates(&self) -> impl Iterator<Item = &'static str> {
        std::iter::once(self.name).chain(self.aliases.iter().copied())
    }
}

/// 比較用の正規化: 前後空白をtrimし、ASCII英字を小文字化する。
fn normalize(input: &str) -> String {
    input.trim().to_ascii_lowercase()
}

/// 入力地名をJMAエリアコードに解決する。
///
/// 1. 表示名またはエイリアスとの完全一致(前後空白trim、ASCII英字は小文字化)
/// 2. 見つからなければ類似度上位(最大3件)をサジェストとして
///    [`AppError::LocationNotFound`] に詰めて返す。
pub fn resolve(input: &str) -> Result<Location, AppError> {
    let normalized = normalize(input);

    if let Some(entry) = LOCATIONS
        .iter()
        .find(|entry| entry.candidates().any(|c| normalize(c) == normalized))
    {
        return Ok(entry.to_location());
    }

    Err(AppError::LocationNotFound {
        input: input.trim().to_string(),
        suggestions: suggest(&normalized),
    })
}

/// 類似度上位(しきい値以上、最大 [`MAX_SUGGESTIONS`] 件)の表示名を返す。
fn suggest(normalized_input: &str) -> Vec<String> {
    let mut scored: Vec<(f64, &'static str)> = LOCATIONS
        .iter()
        .filter_map(|entry| {
            let best = entry
                .candidates()
                .map(|c| strsim::normalized_levenshtein(normalized_input, &normalize(c)))
                .fold(0.0_f64, f64::max);
            (best >= SUGGESTION_THRESHOLD).then_some((best, entry.name))
        })
        .collect();

    scored.sort_by(|a, b| b.0.total_cmp(&a.0));
    scored
        .into_iter()
        .take(MAX_SUGGESTIONS)
        .map(|(_, name)| name.to_string())
        .collect()
}

/// 収録している全地点の一覧を返す。
pub fn all_locations() -> Vec<Location> {
    LOCATIONS.iter().map(LocationEntry::to_location).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- 完全一致(表示名) ---

    #[test]
    fn resolve_exact_display_name() {
        let loc = resolve("東京").unwrap();
        assert_eq!(loc.name, "東京");
        assert_eq!(loc.area_code, "130000");
    }

    #[test]
    fn resolve_hokkaido_maps_to_sapporo_office() {
        let loc = resolve("北海道").unwrap();
        assert_eq!(loc.area_code, "016000");
    }

    // --- 完全一致(エイリアス) ---

    #[test]
    fn resolve_alias_prefecture_full_name() {
        let loc = resolve("東京都").unwrap();
        assert_eq!(loc.name, "東京");
        assert_eq!(loc.area_code, "130000");
    }

    #[test]
    fn resolve_alias_capital_city_name() {
        // 県名と県庁所在地名が異なるケース
        let loc = resolve("名古屋").unwrap();
        assert_eq!(loc.name, "愛知");
        assert_eq!(loc.area_code, "230000");
    }

    #[test]
    fn resolve_alias_romaji_case_insensitive() {
        let loc = resolve("Tokyo").unwrap();
        assert_eq!(loc.area_code, "130000");
        let loc = resolve("OSAKA").unwrap();
        assert_eq!(loc.area_code, "270000");
    }

    #[test]
    fn resolve_alias_hiragana() {
        let loc = resolve("おおさか").unwrap();
        assert_eq!(loc.name, "大阪");
        assert_eq!(loc.area_code, "270000");
    }

    // --- 空白混じり ---

    #[test]
    fn resolve_trims_surrounding_whitespace() {
        let loc = resolve("  東京  ").unwrap();
        assert_eq!(loc.area_code, "130000");
        let loc = resolve("\t福岡\n").unwrap();
        assert_eq!(loc.area_code, "400000");
    }

    // --- 該当なし ---

    #[test]
    fn resolve_unknown_returns_location_not_found() {
        let err = resolve("ロンドン").unwrap_err();
        match err {
            AppError::LocationNotFound { input, .. } => assert_eq!(input, "ロンドン"),
            other => panic!("expected LocationNotFound, got: {other:?}"),
        }
    }

    #[test]
    fn resolve_gibberish_returns_no_suggestions() {
        let err = resolve("zzzzzzzzzz").unwrap_err();
        match err {
            AppError::LocationNotFound { suggestions, .. } => {
                assert!(suggestions.is_empty(), "suggestions: {suggestions:?}")
            }
            other => panic!("expected LocationNotFound, got: {other:?}"),
        }
    }

    // --- タイプミス想定(サジェスト) ---

    #[test]
    fn resolve_typo_kanji_suggests_tokyo() {
        // 「東凶」→「東京」をサジェスト
        let err = resolve("東凶").unwrap_err();
        match err {
            AppError::LocationNotFound { suggestions, .. } => {
                assert!(
                    suggestions.contains(&"東京".to_string()),
                    "suggestions: {suggestions:?}"
                );
            }
            other => panic!("expected LocationNotFound, got: {other:?}"),
        }
    }

    #[test]
    fn resolve_typo_hiragana_suggests_osaka() {
        // 「おうさか」→「大阪」(ひらがなエイリアス「おおさか」に近い)
        let err = resolve("おうさか").unwrap_err();
        match err {
            AppError::LocationNotFound { suggestions, .. } => {
                assert!(
                    suggestions.contains(&"大阪".to_string()),
                    "suggestions: {suggestions:?}"
                );
            }
            other => panic!("expected LocationNotFound, got: {other:?}"),
        }
    }

    #[test]
    fn resolve_typo_romaji_suggests() {
        let err = resolve("tokio").unwrap_err();
        match err {
            AppError::LocationNotFound { suggestions, .. } => {
                assert!(
                    suggestions.contains(&"東京".to_string()),
                    "suggestions: {suggestions:?}"
                );
            }
            other => panic!("expected LocationNotFound, got: {other:?}"),
        }
    }

    #[test]
    fn suggestions_are_at_most_three() {
        let err = resolve("山口県県").unwrap_err();
        match err {
            AppError::LocationNotFound { suggestions, .. } => {
                assert!(suggestions.len() <= 3, "suggestions: {suggestions:?}")
            }
            other => panic!("expected LocationNotFound, got: {other:?}"),
        }
    }

    // --- 全件データ検証 ---

    #[test]
    fn all_locations_has_47_entries_with_area_codes() {
        let locs = all_locations();
        assert_eq!(locs.len(), 47);
        for loc in &locs {
            assert!(!loc.name.is_empty());
            assert_eq!(
                loc.area_code.len(),
                6,
                "{} のコードが6桁でない: {}",
                loc.name,
                loc.area_code
            );
            assert!(
                loc.area_code.chars().all(|c| c.is_ascii_digit()),
                "{} のコードが数字でない: {}",
                loc.name,
                loc.area_code
            );
        }
    }

    #[test]
    fn all_locations_area_codes_are_unique() {
        let locs = all_locations();
        let mut codes: Vec<_> = locs.iter().map(|l| l.area_code.as_str()).collect();
        codes.sort_unstable();
        codes.dedup();
        assert_eq!(codes.len(), 47);
    }

    #[test]
    fn every_location_name_resolves_to_itself() {
        for loc in all_locations() {
            let resolved = resolve(&loc.name).unwrap();
            assert_eq!(resolved, loc);
        }
    }
}
