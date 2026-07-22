//! TUIの状態(表示中タブ・終了フラグ)とキー入力による状態遷移。
//!
//! 描画に依存しない純粋なロジックのため単体テスト対象。

use crossterm::event::KeyCode;

/// 表示中のタブ。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    /// 今日・明日の予報
    TodayTomorrow,
    /// 週間予報
    Weekly,
}

impl Tab {
    /// もう一方のタブを返す(タブは2つなのでトグルで十分)。
    fn toggled(self) -> Self {
        match self {
            Tab::TodayTomorrow => Tab::Weekly,
            Tab::Weekly => Tab::TodayTomorrow,
        }
    }
}

/// TUIアプリケーションの状態。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppState {
    pub tab: Tab,
    pub should_quit: bool,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            tab: Tab::TodayTomorrow,
            should_quit: false,
        }
    }

    /// キー入力を状態遷移に反映する。
    ///
    /// - `q` / `Esc`: 終了
    /// - `Tab` / `←` `→` / `h` `l`: タブ切り替え(トグル)
    /// - `1` / `2`: タブ直接選択
    pub fn on_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Tab
            | KeyCode::Left
            | KeyCode::Right
            | KeyCode::Char('h')
            | KeyCode::Char('l') => self.tab = self.tab.toggled(),
            KeyCode::Char('1') => self.tab = Tab::TodayTomorrow,
            KeyCode::Char('2') => self.tab = Tab::Weekly,
            _ => {}
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 初期状態は今日明日タブで終了フラグなし() {
        let state = AppState::new();
        assert_eq!(state.tab, Tab::TodayTomorrow);
        assert!(!state.should_quit);
    }

    #[test]
    fn qキーとescキーで終了フラグが立つ() {
        for code in [KeyCode::Char('q'), KeyCode::Esc] {
            let mut state = AppState::new();
            state.on_key(code);
            assert!(state.should_quit, "code: {code:?}");
        }
    }

    #[test]
    fn タブ切替キーでタブがトグルする() {
        for code in [
            KeyCode::Tab,
            KeyCode::Left,
            KeyCode::Right,
            KeyCode::Char('h'),
            KeyCode::Char('l'),
        ] {
            let mut state = AppState::new();
            state.on_key(code);
            assert_eq!(state.tab, Tab::Weekly, "code: {code:?}");
            state.on_key(code);
            assert_eq!(state.tab, Tab::TodayTomorrow, "code: {code:?}");
        }
    }

    #[test]
    fn 数字キーでタブを直接選択できる() {
        let mut state = AppState::new();
        state.on_key(KeyCode::Char('2'));
        assert_eq!(state.tab, Tab::Weekly);
        // 同じタブを選び直しても変わらない
        state.on_key(KeyCode::Char('2'));
        assert_eq!(state.tab, Tab::Weekly);
        state.on_key(KeyCode::Char('1'));
        assert_eq!(state.tab, Tab::TodayTomorrow);
    }

    #[test]
    fn 無関係なキーでは状態が変わらない() {
        let mut state = AppState::new();
        state.on_key(KeyCode::Char('x'));
        state.on_key(KeyCode::Enter);
        assert_eq!(state, AppState::new());
    }
}
