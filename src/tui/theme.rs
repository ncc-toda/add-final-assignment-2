//! カラーテーマのプリセット定義(neon / dark / light / vivid)と名前解決。

use ratatui::style::Color;
use ratatui::widgets::BorderType;

use crate::error::AppError;

/// 有効なテーマ名の一覧(エラーメッセージの案内にも使う)。
pub const THEME_NAMES: &[&str] = &["neon", "dark", "light", "vivid"];

/// TUI全体で使うカラーパレット。
#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    /// パネル枠線の描画スタイル(角丸・二重線など)
    pub border_type: BorderType,
    /// 枠線・アクセントを太字(ターミナルによっては発光的に見える)にするか
    pub glow: bool,
    /// 画面全体(アニメーション領域)の背景色
    pub screen_bg: Color,
    /// 情報パネルの背景色
    pub panel_bg: Color,
    /// 情報パネルの文字色
    pub panel_fg: Color,
    /// パネル枠線の色
    pub border: Color,
    /// タブ選択などの強調色
    pub accent: Color,
    /// フッターヒントなど控えめな文字色
    pub muted: Color,
    /// 特別警報の帯(背景)
    pub emergency_bg: Color,
    /// 警報の帯(背景)
    pub warning_bg: Color,
    /// 警報帯の文字色
    pub band_fg: Color,
    /// 注意報の文字色(帯は敷かず文字色のみ)
    pub advisory_fg: Color,
    /// 期限切れキャッシュ警告の文字色
    pub stale_fg: Color,
}

impl Theme {
    /// テーマ名からプリセットを解決する。未知の名前は `ConfigInvalid` エラー。
    pub fn from_name(name: &str) -> Result<Self, AppError> {
        match name {
            "neon" => Ok(Self::neon()),
            "dark" => Ok(Self::dark()),
            "light" => Ok(Self::light()),
            "vivid" => Ok(Self::vivid()),
            _ => Err(AppError::ConfigInvalid(format!(
                "未知のカラーテーマ名「{name}」。有効値: {}",
                THEME_NAMES.join(", ")
            ))),
        }
    }

    /// サイバー/ネオン基調。純黒に近い背景に電光カラー(シアン/マゼンタ)を載せる。
    fn neon() -> Self {
        Self {
            border_type: BorderType::Rounded,
            glow: true,
            screen_bg: Color::Rgb(4, 6, 12),
            panel_bg: Color::Rgb(10, 12, 22),
            panel_fg: Color::Rgb(215, 240, 255),
            border: Color::Rgb(0, 240, 220),
            accent: Color::Rgb(255, 70, 210),
            muted: Color::Rgb(96, 120, 150),
            emergency_bg: Color::Rgb(255, 40, 170),
            warning_bg: Color::Rgb(240, 40, 90),
            band_fg: Color::Rgb(10, 12, 22),
            advisory_fg: Color::Rgb(120, 255, 180),
            stale_fg: Color::Rgb(120, 255, 180),
        }
    }

    fn dark() -> Self {
        Self {
            border_type: BorderType::Plain,
            glow: false,
            screen_bg: Color::Rgb(12, 16, 24),
            panel_bg: Color::Rgb(28, 33, 44),
            panel_fg: Color::Rgb(220, 223, 228),
            border: Color::Rgb(90, 100, 120),
            accent: Color::Cyan,
            muted: Color::DarkGray,
            emergency_bg: Color::Magenta,
            warning_bg: Color::Red,
            band_fg: Color::White,
            advisory_fg: Color::Yellow,
            stale_fg: Color::Yellow,
        }
    }

    fn light() -> Self {
        Self {
            border_type: BorderType::Plain,
            glow: false,
            screen_bg: Color::Rgb(230, 236, 243),
            panel_bg: Color::Rgb(250, 250, 252),
            panel_fg: Color::Rgb(30, 34, 40),
            border: Color::Rgb(140, 150, 165),
            accent: Color::Blue,
            muted: Color::Gray,
            emergency_bg: Color::Magenta,
            warning_bg: Color::Red,
            band_fg: Color::White,
            advisory_fg: Color::Rgb(160, 110, 0),
            stale_fg: Color::Rgb(160, 110, 0),
        }
    }

    fn vivid() -> Self {
        Self {
            border_type: BorderType::Double,
            glow: true,
            screen_bg: Color::Rgb(24, 8, 48),
            panel_bg: Color::Rgb(48, 20, 84),
            panel_fg: Color::Rgb(240, 240, 255),
            border: Color::Cyan,
            accent: Color::Magenta,
            muted: Color::Rgb(150, 130, 190),
            emergency_bg: Color::Rgb(200, 0, 200),
            warning_bg: Color::Rgb(230, 30, 30),
            band_fg: Color::White,
            advisory_fg: Color::Rgb(255, 220, 60),
            stale_fg: Color::Rgb(255, 220, 60),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 有効なテーマ名は全て解決できる() {
        for name in THEME_NAMES {
            assert!(Theme::from_name(name).is_ok(), "name: {name}");
        }
    }

    #[test]
    fn 未知のテーマ名はエラーになり有効値を案内する() {
        let err = Theme::from_name("drak").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("「drak」"), "msg: {msg}");
        assert!(msg.contains("有効値"), "msg: {msg}");
        assert!(msg.contains("neon, dark, light, vivid"), "msg: {msg}");
    }

    #[test]
    fn 各テーマは異なるパレットを持つ() {
        let neon = Theme::from_name("neon").unwrap();
        let dark = Theme::from_name("dark").unwrap();
        let light = Theme::from_name("light").unwrap();
        let vivid = Theme::from_name("vivid").unwrap();
        assert_ne!(neon, dark);
        assert_ne!(dark, light);
        assert_ne!(dark, vivid);
        assert_ne!(light, vivid);
    }
}
