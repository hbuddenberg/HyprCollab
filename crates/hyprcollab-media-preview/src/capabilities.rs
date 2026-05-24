/// Detected terminal graphics protocol support.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphicsProtocol {
    /// Kitty terminal graphics protocol (preferred).
    Kitty,
    /// DEC Sixel graphics (fallback).
    Sixel,
    /// No known graphics protocol — text-only terminal.
    None,
}

/// Terminal capability flags detected from the environment.
#[derive(Debug, Clone)]
pub struct TerminalCapabilities {
    pub graphics: GraphicsProtocol,
    pub true_color: bool,
    pub term: String,
    pub colorterm: String,
    /// Estimated terminal width in columns (0 = unknown).
    pub columns: u16,
}

impl TerminalCapabilities {
    /// Detect capabilities from environment variables without querying the
    /// terminal (safe to call in non-interactive contexts and tests).
    pub fn from_env() -> Self {
        let term = std::env::var("TERM").unwrap_or_default();
        let term_program = std::env::var("TERM_PROGRAM").unwrap_or_default();
        let colorterm = std::env::var("COLORTERM").unwrap_or_default();
        let mlterm = std::env::var("MLTERM").is_ok();

        let graphics = detect_protocol(&term, &term_program, mlterm);
        let true_color = matches!(
            colorterm.to_lowercase().as_str(),
            "truecolor" | "24bit"
        ) || term.contains("kitty");

        Self {
            graphics,
            true_color,
            term,
            colorterm,
            columns: 0,
        }
    }

    /// Build capabilities with an explicit protocol (useful for tests).
    pub fn with_protocol(protocol: GraphicsProtocol) -> Self {
        Self {
            graphics: protocol,
            true_color: protocol == GraphicsProtocol::Kitty,
            term: String::new(),
            colorterm: String::new(),
            columns: 0,
        }
    }

    pub fn supports_kitty(&self) -> bool {
        self.graphics == GraphicsProtocol::Kitty
    }

    pub fn supports_sixel(&self) -> bool {
        self.graphics == GraphicsProtocol::Sixel
    }

    pub fn supports_any_graphics(&self) -> bool {
        self.graphics != GraphicsProtocol::None
    }
}

impl Default for TerminalCapabilities {
    fn default() -> Self {
        Self {
            graphics: GraphicsProtocol::None,
            true_color: false,
            term: String::new(),
            colorterm: String::new(),
            columns: 0,
        }
    }
}

fn detect_protocol(term: &str, term_program: &str, mlterm: bool) -> GraphicsProtocol {
    if term.contains("kitty") || term_program.eq_ignore_ascii_case("kitty") {
        return GraphicsProtocol::Kitty;
    }
    if mlterm
        || term.contains("xterm")
        || term.contains("mlterm")
        || term.contains("sixel")
        || term_program.to_lowercase().contains("iterm")
    {
        return GraphicsProtocol::Sixel;
    }
    GraphicsProtocol::None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_has_no_protocol() {
        let caps = TerminalCapabilities::default();
        assert_eq!(caps.graphics, GraphicsProtocol::None);
        assert!(!caps.supports_kitty());
        assert!(!caps.supports_sixel());
        assert!(!caps.supports_any_graphics());
    }

    #[test]
    fn with_protocol_kitty() {
        let caps = TerminalCapabilities::with_protocol(GraphicsProtocol::Kitty);
        assert!(caps.supports_kitty());
        assert!(caps.true_color);
    }

    #[test]
    fn with_protocol_sixel() {
        let caps = TerminalCapabilities::with_protocol(GraphicsProtocol::Sixel);
        assert!(caps.supports_sixel());
        assert!(!caps.supports_kitty());
    }

    #[test]
    fn detect_kitty_from_term_var() {
        let p = detect_protocol("xterm-kitty", "", false);
        assert_eq!(p, GraphicsProtocol::Kitty);
    }

    #[test]
    fn detect_kitty_from_term_program() {
        let p = detect_protocol("", "kitty", false);
        assert_eq!(p, GraphicsProtocol::Kitty);
    }

    #[test]
    fn detect_sixel_from_xterm() {
        let p = detect_protocol("xterm-256color", "", false);
        assert_eq!(p, GraphicsProtocol::Sixel);
    }

    #[test]
    fn detect_sixel_from_mlterm_env() {
        let p = detect_protocol("", "", true);
        assert_eq!(p, GraphicsProtocol::Sixel);
    }

    #[test]
    fn unknown_term_returns_none() {
        let p = detect_protocol("dumb", "", false);
        assert_eq!(p, GraphicsProtocol::None);
    }
}
