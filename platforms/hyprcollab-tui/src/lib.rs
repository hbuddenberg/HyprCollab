pub mod app;
pub mod artifact_pane;
pub mod chat_pane;
pub mod input;

pub use app::App;
pub use artifact_pane::ArtifactPane;
pub use chat_pane::{ChatMessage, ChatPane, MessageRole};
pub use input::{AppMode, InputHandler};

/// Launch the full-screen TUI event loop. Returns when the user quits (`:q` or `Ctrl+Q`).
pub async fn run() -> anyhow::Result<()> {
    use crossterm::{
        execute,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    };
    use ratatui::{backend::CrosstermBackend, Terminal};
    use std::io;

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;

    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let mut app = App::new(format!("tui-{ts}"));

    let result = run_loop(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn run_loop<B: ratatui::backend::Backend>(
    terminal: &mut ratatui::Terminal<B>,
    app: &mut App,
) -> anyhow::Result<()> {
    use ratatui::{
        layout::{Constraint, Direction, Layout},
        widgets::{Block, Borders, Paragraph},
    };
    use std::time::Duration;

    loop {
        terminal.draw(|f| {
            let area = f.area();
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(3), Constraint::Length(3)])
                .split(area);

            let mode_label = match app.mode() {
                AppMode::Normal => "NORMAL",
                AppMode::Insert => "INSERT",
                AppMode::Command => "COMMAND",
            };

            let chat_block = Block::default()
                .title(format!("HyprCollab — {}", app.session_id()))
                .borders(Borders::ALL);
            f.render_widget(chat_block, chunks[0]);

            let status = Paragraph::new(format!(
                " [{mode_label}]  Ctrl+Q or :q to quit | i to type | Tab to switch pane"
            ))
            .block(Block::default().borders(Borders::ALL));
            f.render_widget(status, chunks[1]);
        })?;

        if crossterm::event::poll(Duration::from_millis(50))?
            && let crossterm::event::Event::Key(key) = crossterm::event::read()?
        {
            let action = app.input.handle_key(key);
            if action.quit {
                break;
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let j = crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('j'),
            crossterm::event::KeyModifiers::NONE,
        );
        let k = crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('k'),
            crossterm::event::KeyModifiers::NONE,
        );
        let a_j = handler.handle_key(j);
        assert_eq!(a_j.scroll_delta, 1);
        let a_k = handler.handle_key(k);
        assert_eq!(a_k.scroll_delta, -1);
    }

    #[test]
    fn input_i_enters_insert() {
        let mut handler = InputHandler::new();
        let i = crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('i'),
            crossterm::event::KeyModifiers::NONE,
        );
        let action = handler.handle_key(i);
        assert_eq!(action.mode_change, Some(AppMode::Insert));
    }

    #[test]
    fn input_escape_back_to_normal() {
        let mut handler = InputHandler::new();
        handler.mode = AppMode::Insert;
        let esc = crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Esc,
            crossterm::event::KeyModifiers::NONE,
        );
        let action = handler.handle_key(esc);
        assert_eq!(action.mode_change, Some(AppMode::Normal));
    }

    #[test]
    fn input_colon_enters_command() {
        let mut handler = InputHandler::new();
        let colon = crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char(':'),
            crossterm::event::KeyModifiers::NONE,
        );
        let action = handler.handle_key(colon);
        assert_eq!(action.mode_change, Some(AppMode::Command));
    }

    #[test]
    fn input_gg_goes_top() {
        let mut handler = InputHandler::new();
        let g = crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('g'),
            crossterm::event::KeyModifiers::NONE,
        );
        handler.handle_key(g);
        let action = handler.handle_key(g);
        assert!(action.go_top);
    }

    #[test]
    fn input_capital_g_bottom() {
        let mut handler = InputHandler::new();
        let g = crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('G'),
            crossterm::event::KeyModifiers::NONE,
        );
        let action = handler.handle_key(g);
        assert!(action.go_bottom);
    }

    #[test]
    fn input_tab_switches_pane() {
        let mut handler = InputHandler::new();
        let tab = crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Tab,
            crossterm::event::KeyModifiers::NONE,
        );
        let action = handler.handle_key(tab);
        assert!(action.switch_pane);
    }

    #[test]
    fn input_insert_types_char() {
        let mut handler = InputHandler::new();
        handler.mode = AppMode::Insert;
        let a = crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('a'),
            crossterm::event::KeyModifiers::NONE,
        );
        let action = handler.handle_key(a);
        assert_eq!(action.append_char, Some('a'));
    }

    #[test]
    fn input_command_execute() {
        let mut handler = InputHandler::new();
        handler.mode = AppMode::Command;
        let q = crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('q'),
            crossterm::event::KeyModifiers::NONE,
        );
        handler.handle_key(q);
        let enter = crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Enter,
            crossterm::event::KeyModifiers::NONE,
        );
        let action = handler.handle_key(enter);
        assert!(action.command_text.is_some());
    }

    #[test]
    fn artifact_pane_default_empty() {
        let pane = ArtifactPane::new();
        assert!(pane.current_id().is_none());
        assert!(pane.is_empty());
    }

    #[test]
    fn run_loop_quits_on_ctrl_q() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

        let mut app = App::new("test".into());
        let ctrl_q = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL);
        let action = app.input.handle_key(ctrl_q);
        assert!(action.quit, "Ctrl+Q should set quit=true");
    }

    #[test]
    fn run_loop_quits_on_colon_q() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

        let mut app = App::new("test".into());
        let colon = KeyEvent::new(KeyCode::Char(':'), KeyModifiers::NONE);
        let q = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
        let enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        app.input.handle_key(colon);
        app.input.handle_key(q);
        let action = app.input.handle_key(enter);
        assert!(action.quit, ":q<Enter> should set quit=true");
    }
}
