use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// The current vim-like editing mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    /// Default mode: navigation keys active (j/k/gg/G/Tab/i/:).
    Normal,
    /// Free-text input mode (entered via `i`).
    Insert,
    /// Ex-command mode (entered via `:`).
    Command,
}

impl Default for AppMode {
    fn default() -> Self {
        Self::Normal
    }
}

/// Which pane currently has focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusedPane {
    Chat,
    Artifact,
}

impl Default for FocusedPane {
    fn default() -> Self {
        Self::Chat
    }
}

/// All state mutations produced by processing a single key event.
#[derive(Debug, Default)]
pub struct KeyAction {
    pub mode_change: Option<AppMode>,
    pub scroll_delta: i32,
    pub go_top: bool,
    pub go_bottom: bool,
    pub switch_pane: bool,
    pub submit_input: bool,
    pub quit: bool,
    pub append_char: Option<char>,
    pub backspace: bool,
    pub command_text: Option<String>,
}

/// Stateful key handler that accumulates pending key sequences (e.g. `gg`).
#[derive(Debug, Default)]
pub struct InputHandler {
    pub mode: AppMode,
    pub pane: FocusedPane,
    /// Buffer for the current text being typed in Insert mode.
    pub input_buf: String,
    /// Buffer for the command being typed in Command mode.
    pub command_buf: String,
    /// Pending 'g' for `gg` detection.
    pending_g: bool,
}

impl InputHandler {
    pub fn new() -> Self {
        Self::default()
    }

    /// Process a raw key event and return a `KeyAction` describing what should
    /// happen as a result.  The handler mutates its own mode and buffer state.
    pub fn handle_key(&mut self, key: KeyEvent) -> KeyAction {
        let mut action = KeyAction::default();

        match self.mode {
            AppMode::Insert => self.handle_insert(key, &mut action),
            AppMode::Command => self.handle_command(key, &mut action),
            AppMode::Normal => self.handle_normal(key, &mut action),
        }

        // Propagate mode back into action so callers can read it.
        if let Some(m) = action.mode_change {
            self.mode = m;
        }

        action
    }

    fn handle_normal(&mut self, key: KeyEvent, action: &mut KeyAction) {
        match key.code {
            KeyCode::Char('i') => {
                self.pending_g = false;
                action.mode_change = Some(AppMode::Insert);
            }
            KeyCode::Char(':') => {
                self.pending_g = false;
                self.command_buf.clear();
                action.mode_change = Some(AppMode::Command);
            }
            KeyCode::Char('j') => {
                self.pending_g = false;
                action.scroll_delta = 1;
            }
            KeyCode::Char('k') => {
                self.pending_g = false;
                action.scroll_delta = -1;
            }
            KeyCode::Char('g') => {
                if self.pending_g {
                    // `gg` — go to top
                    self.pending_g = false;
                    action.go_top = true;
                } else {
                    self.pending_g = true;
                }
            }
            KeyCode::Char('G') => {
                self.pending_g = false;
                action.go_bottom = true;
            }
            KeyCode::Tab => {
                self.pending_g = false;
                action.switch_pane = true;
                self.pane = match self.pane {
                    FocusedPane::Chat => FocusedPane::Artifact,
                    FocusedPane::Artifact => FocusedPane::Chat,
                };
            }
            KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.pending_g = false;
                action.quit = true;
            }
            _ => {
                self.pending_g = false;
            }
        }
    }

    fn handle_insert(&mut self, key: KeyEvent, action: &mut KeyAction) {
        match key.code {
            KeyCode::Esc => {
                action.mode_change = Some(AppMode::Normal);
            }
            KeyCode::Enter => {
                action.submit_input = true;
                action.mode_change = Some(AppMode::Normal);
            }
            KeyCode::Backspace => {
                self.input_buf.pop();
                action.backspace = true;
            }
            KeyCode::Char(c) => {
                self.input_buf.push(c);
                action.append_char = Some(c);
            }
            _ => {}
        }
    }

    fn handle_command(&mut self, key: KeyEvent, action: &mut KeyAction) {
        match key.code {
            KeyCode::Esc => {
                self.command_buf.clear();
                action.mode_change = Some(AppMode::Normal);
            }
            KeyCode::Enter => {
                let cmd = self.command_buf.trim().to_string();
                action.command_text = Some(cmd.clone());
                action.mode_change = Some(AppMode::Normal);
                // Handle :q / :quit
                if cmd == "q" || cmd == "quit" {
                    action.quit = true;
                }
                self.command_buf.clear();
            }
            KeyCode::Backspace => {
                self.command_buf.pop();
                action.backspace = true;
            }
            KeyCode::Char(c) => {
                self.command_buf.push(c);
                action.append_char = Some(c);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::empty())
    }

    fn ctrl(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::CONTROL)
    }

    #[test]
    fn initial_mode_is_normal() {
        assert_eq!(InputHandler::new().mode, AppMode::Normal);
    }

    #[test]
    fn i_enters_insert_mode() {
        let mut h = InputHandler::new();
        let a = h.handle_key(key(KeyCode::Char('i')));
        assert_eq!(a.mode_change, Some(AppMode::Insert));
        assert_eq!(h.mode, AppMode::Insert);
    }

    #[test]
    fn esc_returns_to_normal_from_insert() {
        let mut h = InputHandler::new();
        h.handle_key(key(KeyCode::Char('i')));
        let a = h.handle_key(key(KeyCode::Esc));
        assert_eq!(a.mode_change, Some(AppMode::Normal));
        assert_eq!(h.mode, AppMode::Normal);
    }

    #[test]
    fn colon_enters_command_mode() {
        let mut h = InputHandler::new();
        let a = h.handle_key(key(KeyCode::Char(':')));
        assert_eq!(a.mode_change, Some(AppMode::Command));
    }

    #[test]
    fn j_scrolls_down_in_normal() {
        let mut h = InputHandler::new();
        let a = h.handle_key(key(KeyCode::Char('j')));
        assert_eq!(a.scroll_delta, 1);
    }

    #[test]
    fn k_scrolls_up_in_normal() {
        let mut h = InputHandler::new();
        let a = h.handle_key(key(KeyCode::Char('k')));
        assert_eq!(a.scroll_delta, -1);
    }

    #[test]
    fn gg_goes_to_top() {
        let mut h = InputHandler::new();
        h.handle_key(key(KeyCode::Char('g')));
        let a = h.handle_key(key(KeyCode::Char('g')));
        assert!(a.go_top, "gg should set go_top");
    }

    #[test]
    fn single_g_does_not_go_top() {
        let mut h = InputHandler::new();
        let a = h.handle_key(key(KeyCode::Char('g')));
        assert!(!a.go_top);
    }

    #[test]
    fn capital_g_goes_to_bottom() {
        let mut h = InputHandler::new();
        let a = h.handle_key(key(KeyCode::Char('G')));
        assert!(a.go_bottom);
    }

    #[test]
    fn tab_switches_pane() {
        let mut h = InputHandler::new();
        assert_eq!(h.pane, FocusedPane::Chat);
        let a = h.handle_key(key(KeyCode::Tab));
        assert!(a.switch_pane);
        assert_eq!(h.pane, FocusedPane::Artifact);
        h.handle_key(key(KeyCode::Tab));
        assert_eq!(h.pane, FocusedPane::Chat);
    }

    #[test]
    fn insert_mode_accumulates_text() {
        let mut h = InputHandler::new();
        h.handle_key(key(KeyCode::Char('i')));
        h.handle_key(key(KeyCode::Char('h')));
        h.handle_key(key(KeyCode::Char('i')));
        assert_eq!(h.input_buf, "hi");
    }

    #[test]
    fn backspace_removes_last_char() {
        let mut h = InputHandler::new();
        h.handle_key(key(KeyCode::Char('i')));
        h.handle_key(key(KeyCode::Char('a')));
        h.handle_key(key(KeyCode::Char('b')));
        h.handle_key(key(KeyCode::Backspace));
        assert_eq!(h.input_buf, "a");
    }

    #[test]
    fn enter_in_insert_submits_and_returns_to_normal() {
        let mut h = InputHandler::new();
        h.handle_key(key(KeyCode::Char('i')));
        let a = h.handle_key(key(KeyCode::Enter));
        assert!(a.submit_input);
        assert_eq!(a.mode_change, Some(AppMode::Normal));
    }

    #[test]
    fn command_q_sets_quit() {
        let mut h = InputHandler::new();
        h.handle_key(key(KeyCode::Char(':')));
        h.handle_key(key(KeyCode::Char('q')));
        let a = h.handle_key(key(KeyCode::Enter));
        assert!(a.quit);
        assert_eq!(a.command_text.as_deref(), Some("q"));
    }

    #[test]
    fn command_mode_esc_cancels() {
        let mut h = InputHandler::new();
        h.handle_key(key(KeyCode::Char(':')));
        h.handle_key(key(KeyCode::Char('x')));
        let a = h.handle_key(key(KeyCode::Esc));
        assert_eq!(a.mode_change, Some(AppMode::Normal));
        assert!(h.command_buf.is_empty());
    }

    #[test]
    fn ctrl_q_quits_from_normal() {
        let mut h = InputHandler::new();
        let a = h.handle_key(ctrl(KeyCode::Char('q')));
        assert!(a.quit);
    }
}
