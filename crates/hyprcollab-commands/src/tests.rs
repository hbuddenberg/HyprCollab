//! Tests for hyprcollab-commands.

use hyprcollab_core::traits::SlashCommand;
use crate::builtins::*;
use crate::parser::ParsedCommand;
use crate::registry::CommandRegistry;

// ── Parser tests are in parser.rs ────────────────────────────────────

// ── Built-in command tests ───────────────────────────────────────────

mod agent_cmd {
    use super::*;
    #[tokio::test]
    async fn list() {
        let cmd = AgentCommand;
        let args = cmd.parse_args("").unwrap();
        let result = cmd.execute(args).await.unwrap();
        assert!(result.contains("code-reviewer"));
    }
    #[tokio::test]
    async fn load() {
        let cmd = AgentCommand;
        let args = cmd.parse_args("load debugger").unwrap();
        let result = cmd.execute(args).await.unwrap();
        assert!(result.contains("debugger"));
    }
    #[tokio::test]
    async fn load_empty_err() {
        let cmd = AgentCommand;
        let args = cmd.parse_args("load").unwrap();
        let result = cmd.execute(args).await;
        assert!(result.is_err());
    }
}

mod skill_cmd {
    use super::*;
    #[tokio::test]
    async fn list() {
        let cmd = SkillCommand;
        let args = cmd.parse_args("").unwrap();
        let result = cmd.execute(args).await.unwrap();
        assert!(result.contains("debugging"));
    }
    #[tokio::test]
    async fn load() {
        let cmd = SkillCommand;
        let args = cmd.parse_args("load planning").unwrap();
        let result = cmd.execute(args).await.unwrap();
        assert!(result.contains("planning"));
    }
}

mod temperature_cmd {
    use super::*;
    #[tokio::test]
    async fn show() {
        let cmd = TemperatureCommand;
        let args = cmd.parse_args("").unwrap();
        let result = cmd.execute(args).await.unwrap();
        assert!(result.contains("0.7"));
    }
    #[tokio::test]
    async fn set_valid() {
        let cmd = TemperatureCommand;
        let args = cmd.parse_args("0.5").unwrap();
        let result = cmd.execute(args).await.unwrap();
        assert!(result.contains("0.5"));
    }
    #[test]
    fn invalid_value() {
        let cmd = TemperatureCommand;
        let result = cmd.parse_args("5.0");
        assert!(result.is_err());
    }
    #[test]
    fn non_numeric() {
        let cmd = TemperatureCommand;
        let result = cmd.parse_args("abc");
        assert!(result.is_err());
    }
}

mod approval_cmd {
    use super::*;
    #[tokio::test]
    async fn show() {
        let cmd = ApprovalCommand;
        let args = cmd.parse_args("").unwrap();
        let result = cmd.execute(args).await.unwrap();
        assert!(result.contains("normal"));
    }
    #[tokio::test]
    async fn set_strict() {
        let cmd = ApprovalCommand;
        let args = cmd.parse_args("strict").unwrap();
        let result = cmd.execute(args).await.unwrap();
        assert!(result.contains("strict"));
    }
    #[test]
    fn invalid_mode() {
        let cmd = ApprovalCommand;
        let result = cmd.parse_args("banana");
        assert!(result.is_err());
    }
}

mod run_cmd {
    use super::*;
    #[tokio::test]
    async fn run_with_command() {
        let cmd = RunCommand;
        let args = cmd.parse_args("cargo test").unwrap();
        let result = cmd.execute(args).await.unwrap();
        assert!(result.contains("cargo test"));
    }
    #[test]
    fn empty_err() {
        let cmd = RunCommand;
        let result = cmd.parse_args("");
        assert!(result.is_err());
    }
}

mod browse_cmd {
    use super::*;
    #[tokio::test]
    async fn browse_url() {
        let cmd = BrowseCommand;
        let args = cmd.parse_args("https://example.com").unwrap();
        let result = cmd.execute(args).await.unwrap();
        assert!(result.contains("https://example.com"));
    }
}

mod help_cmd {
    use super::*;
    #[tokio::test]
    async fn help_all() {
        let cmd = HelpCommand;
        let args = cmd.parse_args("").unwrap();
        let result = cmd.execute(args).await.unwrap();
        assert!(result.contains("/agent"));
        assert!(result.contains("/help"));
    }
    #[tokio::test]
    async fn help_search() {
        let cmd = HelpCommand;
        let args = cmd.parse_args("review").unwrap();
        let result = cmd.execute(args).await.unwrap();
        assert!(result.contains("review"));
    }
}

mod config_cmd {
    use super::*;
    #[tokio::test]
    async fn list_config() {
        let cmd = ConfigCommand;
        let args = cmd.parse_args("").unwrap();
        let result = cmd.execute(args).await.unwrap();
        assert!(result.contains("model"));
    }
    #[tokio::test]
    async fn set_config() {
        let cmd = ConfigCommand;
        let args = cmd.parse_args("set model anthropic/claude-sonnet-4").unwrap();
        let result = cmd.execute(args).await.unwrap();
        assert!(result.contains("anthropic/claude-sonnet-4"));
    }
}

// ── Registry integration test ────────────────────────────────────────

mod registry_integration {
    use super::*;
    use crate::builtins::register_all;

    #[tokio::test]
    async fn register_all_and_execute() {
        let mut reg = CommandRegistry::new();
        register_all(&mut reg);
        assert_eq!(reg.len(), 10);

        // Test /help
        let result = reg.execute("/help").await.unwrap().unwrap();
        assert!(result.contains("/agent"));

        // Test unknown
        let result = reg.execute("/nonexistent").await.unwrap();
        assert!(result.is_err());
    }

    #[test]
    fn names_include_all() {
        let mut reg = CommandRegistry::new();
        register_all(&mut reg);
        let names = reg.names();
        assert!(names.contains(&"/agent".to_string()));
        assert!(names.contains(&"/help".to_string()));
        assert!(names.contains(&"/temperature".to_string()));
        assert!(names.contains(&"/approval".to_string()));
        assert!(names.contains(&"/run".to_string()));
    }
}
