pub mod app;
pub mod artifact_pane;
pub mod chat_pane;
pub mod input;

pub use app::App;
pub use artifact_pane::ArtifactPane;
pub use chat_pane::{ChatMessage, ChatPane, MessageRole};
pub use input::{AppMode, InputHandler};

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    #[test]
    fn app_default_has_normal_mode() {
        let app = App::new("test-session".into());
        assert_eq!(app.mode(), AppMode::Normal);
    }

    #[test]
    fn app_session_id_preserved() {
        let app = App::new("my-session".into());
        assert_eq!(app.session_id(), "my-session");
    }

    #[test]
    fn chat_pane_add_messages() {
        let mut pane = ChatPane::new();
        pane.add_message(ChatMessage::user("hello"));
        pane.add_message(ChatMessage::assistant("world"));
        assert_eq!(pane.message_count(), 2);
        assert_eq!(pane.messages()[0].role, MessageRole::User);
        assert_eq!(pane.messages()[1].role, MessageRole::Assistant);
    }

    #[test]
    fn chat_pane_scroll_within_bounds() {
        let mut pane = ChatPane::new();
        for i in 0..20 {
            pane.add_message(ChatMessage::user(format!("msg {i}")));
        }
        pane.scroll_down(5);
        assert_eq!(pane.scroll_offset(), 5);
        pane.scroll_to_bottom();
        assert!(pane.scroll_offset() > 5);
    }

    #[test]
    fn input_normal_j_k_scroll() {
        let mut handler = InputHandler::new();
        let j = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);
        let k = KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE);
        let a_j = handler.handle_key(j);
        assert_eq!(a_j.scroll_delta, 1);
        let a_k = handler.handle_key(k);
        assert_eq!(a_k.scroll_delta, -1);
    }

    #[test]
    fn input_i_enters_insert() {
        let mut handler = InputHandler::new();
        let i = KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE);
        let action = handler.handle_key(i);
        assert_eq!(action.mode_change, Some(AppMode::Insert));
    }

    #[test]
    fn input_escape_back_to_normal() {
        let mut handler = InputHandler::new();
        handler.mode = AppMode::Insert;
        let esc = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        let action = handler.handle_key(esc);
        assert_eq!(action.mode_change, Some(AppMode::Normal));
    }

    #[test]
    fn input_colon_enters_command() {
        let mut handler = InputHandler::new();
        let colon = KeyEvent::new(KeyCode::Char(':'), KeyModifiers::NONE);
        let action = handler.handle_key(colon);
        assert_eq!(action.mode_change, Some(AppMode::Command));
    }

    #[test]
    fn input_gg_goes_top() {
        let mut handler = InputHandler::new();
        let g = KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE);
        handler.handle_key(g);
        let action = handler.handle_key(g);
        assert!(action.go_top);
    }

    #[test]
    fn input_capital_g_bottom() {
        let mut handler = InputHandler::new();
        let g = KeyEvent::new(KeyCode::Char('G'), KeyModifiers::NONE);
        let action = handler.handle_key(g);
        assert!(action.go_bottom);
    }

    #[test]
    fn input_tab_switches_pane() {
        let mut handler = InputHandler::new();
        let tab = KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE);
        let action = handler.handle_key(tab);
        assert!(action.switch_pane);
    }

    #[test]
    fn input_insert_types_char() {
        let mut handler = InputHandler::new();
        handler.mode = AppMode::Insert;
        let a = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
        let action = handler.handle_key(a);
        assert_eq!(action.append_char, Some('a'));
    }

    #[test]
    fn input_command_execute() {
        let mut handler = InputHandler::new();
        handler.mode = AppMode::Command;
        let q = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
        handler.handle_key(q);
        let enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        let action = handler.handle_key(enter);
        assert!(action.command_text.is_some());
    }

    #[test]
    fn artifact_pane_default_empty() {
        let pane = ArtifactPane::new();
        assert!(pane.current_id().is_none());
        assert!(pane.is_empty());
    }
}
