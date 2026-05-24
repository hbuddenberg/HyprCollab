use serde::{Deserialize, Serialize};

/// Role of a chat message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

impl MessageRole {
    pub fn label(&self) -> &'static str {
        match self {
            MessageRole::User => "You",
            MessageRole::Assistant => "Assistant",
            MessageRole::System => "System",
        }
    }
}

/// A single message displayed in the chat pane.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
    /// True while the assistant is still streaming this message.
    pub streaming: bool,
}

impl ChatMessage {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::User,
            content: content.into(),
            streaming: false,
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Assistant,
            content: content.into(),
            streaming: false,
        }
    }

    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::System,
            content: content.into(),
            streaming: false,
        }
    }

    pub fn streaming_assistant() -> Self {
        Self {
            role: MessageRole::Assistant,
            content: String::new(),
            streaming: true,
        }
    }

    /// Append a streamed token and stop streaming when `done` is true.
    pub fn push_token(&mut self, token: &str, done: bool) {
        self.content.push_str(token);
        if done {
            self.streaming = false;
        }
    }
}

/// State for the chat panel.
#[derive(Debug, Default)]
pub struct ChatPane {
    messages: Vec<ChatMessage>,
    scroll: u16,
}

impl ChatPane {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_message(&mut self, msg: ChatMessage) {
        self.messages.push(msg);
    }

    pub fn messages(&self) -> &[ChatMessage] {
        &self.messages
    }

    pub fn message_count(&self) -> usize {
        self.messages.len()
    }

    pub fn scroll_offset(&self) -> u16 {
        self.scroll
    }

    pub fn scroll_down(&mut self, lines: u16) {
        let max_scroll = (self.messages.len() as u16).saturating_sub(1);
        self.scroll = (self.scroll + lines).min(max_scroll);
    }

    pub fn scroll_up(&mut self, lines: u16) {
        self.scroll = self.scroll.saturating_sub(lines);
    }

    pub fn scroll_to_top(&mut self) {
        self.scroll = 0;
    }

    pub fn scroll_to_bottom(&mut self) {
        self.scroll = (self.messages.len() as u16).saturating_sub(1);
    }

    /// Returns all messages that should be visible from the current scroll offset.
    pub fn visible_messages(&self) -> &[ChatMessage] {
        let start = self.scroll as usize;
        if start >= self.messages.len() {
            return &[];
        }
        &self.messages[start..]
    }

    /// Append a token to the last assistant message if it is streaming;
    /// otherwise create a new streaming message.
    pub fn push_stream_token(&mut self, token: &str, done: bool) {
        if let Some(last) = self.messages.last_mut() {
            if last.streaming {
                last.push_token(token, done);
                return;
            }
        }
        let mut msg = ChatMessage::streaming_assistant();
        msg.push_token(token, done);
        self.messages.push(msg);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_and_count_messages() {
        let mut pane = ChatPane::new();
        pane.add_message(ChatMessage::user("hello"));
        pane.add_message(ChatMessage::assistant("hi there"));
        assert_eq!(pane.message_count(), 2);
    }

    #[test]
    fn message_roles_preserved() {
        let mut pane = ChatPane::new();
        pane.add_message(ChatMessage::user("u"));
        pane.add_message(ChatMessage::assistant("a"));
        pane.add_message(ChatMessage::system("s"));
        assert_eq!(pane.messages()[0].role, MessageRole::User);
        assert_eq!(pane.messages()[1].role, MessageRole::Assistant);
        assert_eq!(pane.messages()[2].role, MessageRole::System);
    }

    #[test]
    fn scroll_down_increments() {
        let mut pane = ChatPane::new();
        for i in 0..10 {
            pane.add_message(ChatMessage::user(format!("msg {i}")));
        }
        pane.scroll_down(3);
        assert_eq!(pane.scroll_offset(), 3);
    }

    #[test]
    fn scroll_up_decrements() {
        let mut pane = ChatPane::new();
        for i in 0..10 {
            pane.add_message(ChatMessage::user(format!("msg {i}")));
        }
        pane.scroll_down(5);
        pane.scroll_up(2);
        assert_eq!(pane.scroll_offset(), 3);
    }

    #[test]
    fn scroll_does_not_go_below_zero() {
        let mut pane = ChatPane::new();
        pane.add_message(ChatMessage::user("x"));
        pane.scroll_up(100);
        assert_eq!(pane.scroll_offset(), 0);
    }

    #[test]
    fn scroll_to_top_resets() {
        let mut pane = ChatPane::new();
        for i in 0..5 {
            pane.add_message(ChatMessage::user(format!("{i}")));
        }
        pane.scroll_down(4);
        pane.scroll_to_top();
        assert_eq!(pane.scroll_offset(), 0);
    }

    #[test]
    fn scroll_to_bottom_goes_to_last() {
        let mut pane = ChatPane::new();
        for i in 0..5 {
            pane.add_message(ChatMessage::user(format!("{i}")));
        }
        pane.scroll_to_bottom();
        assert_eq!(pane.scroll_offset(), 4);
    }

    #[test]
    fn streaming_tokens_accumulate() {
        let mut pane = ChatPane::new();
        pane.push_stream_token("Hello", false);
        pane.push_stream_token(", world", true);
        let last = pane.messages().last().unwrap();
        assert_eq!(last.content, "Hello, world");
        assert!(!last.streaming);
    }

    #[test]
    fn visible_messages_respects_scroll() {
        let mut pane = ChatPane::new();
        pane.add_message(ChatMessage::user("a"));
        pane.add_message(ChatMessage::user("b"));
        pane.add_message(ChatMessage::user("c"));
        pane.scroll_down(1);
        let visible = pane.visible_messages();
        assert_eq!(visible[0].content, "b");
    }
}
