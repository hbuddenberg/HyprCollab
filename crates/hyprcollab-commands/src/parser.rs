//! Command parser — splits `/command arg1 arg2 "quoted"` into structured form.

use hyprcollab_core::errors::{CoreError, Result};

/// A parsed slash command.
#[derive(Debug, Clone)]
pub struct ParsedCommand {
    /// Command name without the leading `/`.
    pub name: String,
    /// Positional arguments.
    pub args: Vec<String>,
    /// Named flags (e.g. `--force`, `--model=gpt-4o`).
    pub flags: Vec<(String, Option<String>)>,
    /// The raw input string.
    pub raw: String,
}

impl ParsedCommand {
    /// Parse a raw input string into a ParsedCommand.
    ///
    /// Returns `None` if the input doesn't start with `/`.
    pub fn parse(input: &str) -> Option<Result<Self>> {
        let trimmed = input.trim();
        if !trimmed.starts_with('/') {
            return None;
        }

        // Split into tokens respecting quoted strings.
        let tokens = match tokenize(&trimmed[1..]) {
            Ok(t) => t,
            Err(e) => return Some(Err(e)),
        };

        if tokens.is_empty() {
            return Some(Err(CoreError::Config("Empty command".into())));
        }

        let name = tokens[0].clone();
        let mut args = Vec::new();
        let mut flags = Vec::new();

        for token in &tokens[1..] {
            if let Some(rest) = token.strip_prefix("--") {
                // Long flag: --key or --key=value
                if let Some((k, v)) = rest.split_once('=') {
                    flags.push((k.to_string(), Some(v.to_string())));
                } else {
                    flags.push((rest.to_string(), None));
                }
            } else if let Some(rest) = token.strip_prefix('-') {
                // Short flag: -k or -k=value
                if let Some((k, v)) = rest.split_once('=') {
                    flags.push((k.to_string(), Some(v.to_string())));
                } else {
                    flags.push((rest.to_string(), None));
                }
            } else {
                args.push(token.clone());
            }
        }

        Some(Ok(ParsedCommand {
            name,
            args,
            flags,
            raw: trimmed.to_string(),
        }))
    }

    /// Get the first positional argument, if any.
    pub fn first_arg(&self) -> Option<&str> {
        self.args.first().map(|s| s.as_str())
    }

    /// Get a flag value by name.
    pub fn flag(&self, name: &str) -> Option<Option<&str>> {
        for (k, v) in &self.flags {
            if k == name {
                return Some(v.as_deref());
            }
        }
        None
    }

    /// Check if a flag is present (with or without value).
    pub fn has_flag(&self, name: &str) -> bool {
        self.flags.iter().any(|(k, _)| k == name)
    }

    /// Join all args into a single string.
    pub fn args_joined(&self) -> String {
        self.args.join(" ")
    }
}

/// Tokenize a command string, respecting quoted strings.
fn tokenize(input: &str) -> Result<Vec<String>> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut quote_char = ' ';
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let ch = chars[i];

        if in_quotes {
            if ch == quote_char {
                in_quotes = false;
            } else {
                current.push(ch);
            }
        } else if ch == '"' || ch == '\'' {
            in_quotes = true;
            quote_char = ch;
        } else if ch.is_whitespace() {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
        } else {
            current.push(ch);
        }

        i += 1;
    }

    if in_quotes {
        return Err(CoreError::Config("Unclosed quote in command".into()));
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    Ok(tokens)
}

#[cfg(test)]
mod parser_tests {
    use super::*;

    #[test]
    fn parse_simple_command() {
        let cmd = ParsedCommand::parse("/help").unwrap().unwrap();
        assert_eq!(cmd.name, "help");
        assert!(cmd.args.is_empty());
    }

    #[test]
    fn parse_with_args() {
        let cmd = ParsedCommand::parse("/model list").unwrap().unwrap();
        assert_eq!(cmd.name, "model");
        assert_eq!(cmd.args, vec!["list"]);
    }

    #[test]
    fn parse_with_quoted_arg() {
        let cmd = ParsedCommand::parse(r#"/agent load "Code Reviewer""#).unwrap().unwrap();
        assert_eq!(cmd.name, "agent");
        assert_eq!(cmd.args, vec!["load", "Code Reviewer"]);
    }

    #[test]
    fn parse_with_flags() {
        let cmd = ParsedCommand::parse("/run --force echo hello").unwrap().unwrap();
        assert_eq!(cmd.name, "run");
        assert_eq!(cmd.args, vec!["echo", "hello"]);
        assert!(cmd.has_flag("force"));
    }

    #[test]
    fn parse_with_flag_value() {
        let cmd = ParsedCommand::parse("/model set --provider=openai gpt-4o").unwrap().unwrap();
        assert_eq!(cmd.name, "model");
        assert_eq!(cmd.args, vec!["set", "gpt-4o"]);
        assert_eq!(cmd.flag("provider"), Some(Some("openai")));
    }

    #[test]
    fn parse_not_a_command() {
        assert!(ParsedCommand::parse("hello world").is_none());
    }

    #[test]
    fn parse_unclosed_quote() {
        let result = ParsedCommand::parse(r#"/agent load "unclosed"#).unwrap();
        assert!(result.is_err());
    }

    #[test]
    fn parse_empty_after_slash() {
        let result = ParsedCommand::parse("/").unwrap();
        assert!(result.is_err());
    }

    #[test]
    fn parse_args_joined() {
        let cmd = ParsedCommand::parse("/run echo hello world").unwrap().unwrap();
        assert_eq!(cmd.args_joined(), "echo hello world");
    }

    #[test]
    fn parse_first_arg() {
        let cmd = ParsedCommand::parse("/temperature 0.7").unwrap().unwrap();
        assert_eq!(cmd.first_arg(), Some("0.7"));
    }
}
