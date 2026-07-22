//! レイアウトプリセット(fullscreen / dashboard)と情報パネル領域の計算。

use ratatui::layout::Rect;

use crate::error::AppError;

/// 有効なレイアウト名の一覧(エラーメッセージの案内にも使う)。
pub const LAYOUT_NAMES: &[&str] = &["fullscreen", "dashboard"];

/// fullscreen型パネルの標準サイズ(端末が小さければ端末サイズに収める)。
const FULLSCREEN_PANEL_WIDTH: u16 = 68;
const FULLSCREEN_PANEL_HEIGHT: u16 = 24;

/// dashboard型でパネルの周囲に空けるアニメーション用の余白。
const DASHBOARD_MARGIN_X: u16 = 6;
const DASHBOARD_MARGIN_Y: u16 = 3;

/// レイアウトプリセット。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutPreset {
    /// 背景アニメーション全面 + 中央に小さめの情報パネル
    Fullscreen,
    /// 画面の大部分をパネルが占め、周囲の余白にアニメーションが見える
    Dashboard,
}

impl LayoutPreset {
    /// レイアウト名からプリセットを解決する。未知の名前は `ConfigInvalid` エラー。
    pub fn from_name(name: &str) -> Result<Self, AppError> {
        match name {
            "fullscreen" => Ok(LayoutPreset::Fullscreen),
            "dashboard" => Ok(LayoutPreset::Dashboard),
            _ => Err(AppError::ConfigInvalid(format!(
                "未知のレイアウト名「{name}」。有効値: {}",
                LAYOUT_NAMES.join(", ")
            ))),
        }
    }

    /// 端末全体の領域から情報パネルの描画領域を計算する。
    pub fn panel_area(self, area: Rect) -> Rect {
        match self {
            LayoutPreset::Fullscreen => {
                let width = FULLSCREEN_PANEL_WIDTH.min(area.width);
                let height = FULLSCREEN_PANEL_HEIGHT.min(area.height);
                Rect::new(
                    area.x + (area.width - width) / 2,
                    area.y + (area.height - height) / 2,
                    width,
                    height,
                )
            }
            LayoutPreset::Dashboard => {
                // 余白を確保できないほど狭い端末では全面を使う
                let dx = if area.width > DASHBOARD_MARGIN_X * 2 + 20 {
                    DASHBOARD_MARGIN_X
                } else {
                    0
                };
                let dy = if area.height > DASHBOARD_MARGIN_Y * 2 + 8 {
                    DASHBOARD_MARGIN_Y
                } else {
                    0
                };
                Rect::new(
                    area.x + dx,
                    area.y + dy,
                    area.width - dx * 2,
                    area.height - dy * 2,
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 有効なレイアウト名は全て解決できる() {
        assert_eq!(
            LayoutPreset::from_name("fullscreen").unwrap(),
            LayoutPreset::Fullscreen
        );
        assert_eq!(
            LayoutPreset::from_name("dashboard").unwrap(),
            LayoutPreset::Dashboard
        );
    }

    #[test]
    fn 未知のレイアウト名はエラーになり有効値を案内する() {
        let err = LayoutPreset::from_name("fullscren").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("「fullscren」"), "msg: {msg}");
        assert!(msg.contains("fullscreen, dashboard"), "msg: {msg}");
    }

    #[test]
    fn fullscreenは標準サイズのパネルを中央に配置する() {
        let panel = LayoutPreset::Fullscreen.panel_area(Rect::new(0, 0, 100, 40));
        assert_eq!(panel, Rect::new(16, 8, 68, 24));
    }

    #[test]
    fn fullscreenは端末が狭ければ端末サイズに収める() {
        let panel = LayoutPreset::Fullscreen.panel_area(Rect::new(0, 0, 40, 10));
        assert_eq!(panel, Rect::new(0, 0, 40, 10));
    }

    #[test]
    fn dashboardは周囲に余白を残してパネルを配置する() {
        let panel = LayoutPreset::Dashboard.panel_area(Rect::new(0, 0, 100, 40));
        assert_eq!(panel, Rect::new(6, 3, 88, 34));
    }

    #[test]
    fn dashboardは端末が狭ければ全面を使う() {
        let panel = LayoutPreset::Dashboard.panel_area(Rect::new(0, 0, 20, 8));
        assert_eq!(panel, Rect::new(0, 0, 20, 8));
    }
}
