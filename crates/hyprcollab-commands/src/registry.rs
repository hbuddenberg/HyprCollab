//! Command registry — manages slash command implementations.

use std::collections::HashMap;

use hyprcollab_core::errors::{CoreError, Result};
use hyprcollab_core::traits::SlashCommand;
use hyprcollab_core::types::CommandContext;

use crate::ParsedCommand;

/// Registry that maps command names to their `SlashCommand` implementations.
pub struct CommandRegistry {
    commands: HashMap<String, Box<dyn SlashCommand>>,
}

impl CommandRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            commands: HashMap::new(),
        }
    }

    /// Register a command. Accepts names with or without leading `/`.
    pub fn register(&mut self, cmd: Box<dyn SlashCommand>) {
        let name = cmd.name().trim_start_matches('/').to_string();
        self.commands.insert(name, cmd);
    }

    /// Look up a command by name (without leading `/`).
    pub fn get(&self, name: &str) -> Option<&dyn SlashCommand> {
        self.commands.get(name).map(|c| c.as_ref())
    }

    /// List all registered command names (with leading `/`).
    pub fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.commands.keys().map(|k| format!("/{k}")).collect();
        names.sort();
        names
    }

    /// Get all commands with their descriptions.
    pub fn help_list(&self) -> Vec<(String, String)> {
        let mut list: Vec<_> = self
            .commands
            .values()
            .map(|c| (c.name().to_string(), c.description().to_string()))
            .collect();
        list.sort_by(|a, b| a.0.cmp(&b.0));
        list
    }

    /// Parse and execute a raw input string.
    ///
    /// Returns `None` if the input is not a slash command.
    /// `ctx` is passed through to the command and may be mutated.
    pub async fn execute(&self, input: &str, ctx: &mut CommandContext) -> Option<Result<String>> {
        let parsed = ParsedCommand::parse(input)?;

        match parsed {
            Ok(cmd) => {
                let command = self.commands.get(&cmd.name);
                match command {
                    Some(cmd_impl) => {
                        let args = serde_json::json!({
                            "raw": cmd.args_joined(),
                            "args": cmd.args,
                        });
                        Some(cmd_impl.execute(args, ctx).await)
                    }
                    None => Some(Err(CoreError::Config(format!(
                        "Unknown command: /{}. Type /help for available commands.",
                        cmd.name
                    )))),
                }
            }
            Err(e) => Some(Err(e)),
        }
    }

    /// Number of registered commands.
    pub fn len(&self) -> usize {
        self.commands.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod registry_tests {
    use super::*;
    use async_trait::async_trait;
    use hyprcollab_core::types::ApprovalMode;

    fn test_ctx() -> CommandContext {
        CommandContext {
            session_id: "test-session".to_string(),
            workspace_id: None,
            model: "test-model".to_string(),
            temperature: None,
            approval_mode: ApprovalMode::Normal,
        }
    }

    struct MockCommand {
        name: String,
        desc: String,
    }

    #[async_trait]
    impl SlashCommand for MockCommand {
        fn name(&self) -> &str {
            &self.name
        }

        fn description(&self) -> &str {
            &self.desc
        }

        async fn execute(&self, args: serde_json::Value, _ctx: &mut CommandContext) -> Result<String> {
            Ok(format!("executed {} with {}", self.name, args["raw"].as_str().unwrap_or("")))
        }
    }

    #[test]
    fn register_and_get() {
        let mut reg = CommandRegistry::new();
        reg.register(Box::new(MockCommand {
            name: "/test".into(),
            desc: "Test command".into(),
        }));
        assert!(reg.get("test").is_some());
        assert!(reg.get("nonexistent").is_none());
    }

    #[test]
    fn names_sorted() {
        let mut reg = CommandRegistry::new();
        reg.register(Box::new(MockCommand {
            name: "/beta".into(),
            desc: "B".into(),
        }));
        reg.register(Box::new(MockCommand {
            name: "/alpha".into(),
            desc: "A".into(),
        }));
        assert_eq!(reg.names(), vec!["/alpha", "/beta"]);
    }

    #[tokio::test]
    async fn execute_command() {
        let mut reg = CommandRegistry::new();
        reg.register(Box::new(MockCommand {
            name: "/echo".into(),
            desc: "Echo".into(),
        }));
        let mut ctx = test_ctx();
        let result = reg.execute("/echo hello world", &mut ctx).await.unwrap().unwrap();
        assert!(result.contains("hello world"));
    }

    #[tokio::test]
    async fn execute_unknown() {
        let reg = CommandRegistry::new();
        let mut ctx = test_ctx();
        let result = reg.execute("/unknown", &mut ctx).await.unwrap();
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn not_a_command() {
        let reg = CommandRegistry::new();
        let mut ctx = test_ctx();
        assert!(reg.execute("hello world", &mut ctx).await.is_none());
    }

    #[test]
    fn help_list() {
        let mut reg = CommandRegistry::new();
        reg.register(Box::new(MockCommand {
            name: "/foo".into(),
            desc: "Foo desc".into(),
        }));
        reg.register(Box::new(MockCommand {
            name: "/bar".into(),
            desc: "Bar desc".into(),
        }));
        let help = reg.help_list();
        assert_eq!(help.len(), 2);
        assert_eq!(help[0].0, "/bar");
    }
}
