//! Tests for hyprcollab-commands.

use hyprcollab_core::traits::SlashCommand;
use hyprcollab_core::types::{ApprovalMode, CommandContext};
use crate::builtins::{
    AgentCommand, ApprovalCommand, BrowseCommand, ConfigCommand, HelpCommand, ImageCommand,
    RunCommand, ScrapeCommand, SkillCommand, TemperatureCommand,
};
use crate::registry::CommandRegistry;

fn test_ctx() -> CommandContext {
    CommandContext {
        session_id: "test-session".to_string(),
        workspace_id: None,
        model: "test-model".to_string(),
        temperature: None,
        approval_mode: ApprovalMode::Normal,
    }
}

// ── Parser tests are in parser.rs ────────────────────────────────────

// ── Built-in command tests ───────────────────────────────────────────

mod agent_cmd {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn list() {
        let cmd = AgentCommand;
        let mut ctx = test_ctx();
        let result = cmd.execute(json!({"raw": "", "args": []}), &mut ctx).await.unwrap();
        assert!(result.contains("code-reviewer"));
    }

    #[tokio::test]
    async fn load_missing_arg() {
        let cmd = AgentCommand;
        let mut ctx = test_ctx();
        let result = cmd.execute(json!({"raw": "load", "args": ["load"]}), &mut ctx).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn load_nonexistent_agent() {
        let cmd = AgentCommand;
        let mut ctx = test_ctx();
        // Agent file almost certainly doesn't exist in test env.
        let result = cmd.execute(json!({"raw": "load debugger", "args": ["load", "debugger"]}), &mut ctx).await;
        match result {
            Ok(msg) => assert!(msg.contains("debugger")),
            Err(e) => {
                let msg = e.to_string();
                assert!(msg.contains("debugger") || msg.contains("not found"));
            }
        }
    }

    #[tokio::test]
    async fn create() {
        let cmd = AgentCommand;
        let mut ctx = test_ctx();
        let result = cmd.execute(json!({"raw": "create myagent", "args": ["create", "myagent"]}), &mut ctx).await.unwrap();
        assert!(result.contains("myagent"));
    }

    #[tokio::test]
    async fn unknown_subcommand() {
        let cmd = AgentCommand;
        let mut ctx = test_ctx();
        let result = cmd.execute(json!({"raw": "frobnicate", "args": ["frobnicate"]}), &mut ctx).await;
        assert!(result.is_err());
    }
}

mod skill_cmd {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn list() {
        let cmd = SkillCommand;
        let mut ctx = test_ctx();
        let result = cmd.execute(json!({"raw": "", "args": []}), &mut ctx).await.unwrap();
        assert!(result.contains("debugging"));
    }

    #[tokio::test]
    async fn load() {
        let cmd = SkillCommand;
        let mut ctx = test_ctx();
        let result = cmd.execute(json!({"raw": "load planning", "args": ["load", "planning"]}), &mut ctx).await.unwrap();
        assert!(result.contains("planning"));
    }

    #[tokio::test]
    async fn load_missing_arg() {
        let cmd = SkillCommand;
        let mut ctx = test_ctx();
        let result = cmd.execute(json!({"raw": "load", "args": ["load"]}), &mut ctx).await;
        assert!(result.is_err());
    }
}

mod temperature_cmd {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn show_default() {
        let cmd = TemperatureCommand;
        let mut ctx = test_ctx();
        let result = cmd.execute(json!({"raw": "", "args": []}), &mut ctx).await.unwrap();
        assert!(result.contains("0.7"));
    }

    #[tokio::test]
    async fn show_set_value() {
        let cmd = TemperatureCommand;
        let mut ctx = test_ctx();
        ctx.temperature = Some(1.2);
        let result = cmd.execute(json!({"raw": "", "args": []}), &mut ctx).await.unwrap();
        assert!(result.contains("1.2"));
    }

    #[tokio::test]
    async fn set_valid() {
        let cmd = TemperatureCommand;
        let mut ctx = test_ctx();
        let result = cmd.execute(json!({"raw": "0.5", "args": ["0.5"]}), &mut ctx).await.unwrap();
        assert!(result.contains("0.5"));
        assert_eq!(ctx.temperature, Some(0.5));
    }

    #[tokio::test]
    async fn invalid_out_of_range() {
        let cmd = TemperatureCommand;
        let mut ctx = test_ctx();
        let result = cmd.execute(json!({"raw": "5.0", "args": ["5.0"]}), &mut ctx).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn invalid_non_numeric() {
        let cmd = TemperatureCommand;
        let mut ctx = test_ctx();
        let result = cmd.execute(json!({"raw": "abc", "args": ["abc"]}), &mut ctx).await;
        assert!(result.is_err());
    }
}

mod approval_cmd {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn show() {
        let cmd = ApprovalCommand;
        let mut ctx = test_ctx();
        let result = cmd.execute(json!({"raw": "", "args": []}), &mut ctx).await.unwrap();
        assert!(result.contains("normal"));
    }

    #[tokio::test]
    async fn set_strict() {
        let cmd = ApprovalCommand;
        let mut ctx = test_ctx();
        let result = cmd.execute(json!({"raw": "strict", "args": ["strict"]}), &mut ctx).await.unwrap();
        assert!(result.contains("strict"));
        assert_eq!(ctx.approval_mode, ApprovalMode::Strict);
    }

    #[tokio::test]
    async fn set_auto() {
        let cmd = ApprovalCommand;
        let mut ctx = test_ctx();
        let result = cmd.execute(json!({"raw": "auto", "args": ["auto"]}), &mut ctx).await.unwrap();
        assert!(result.contains("auto"));
        assert_eq!(ctx.approval_mode, ApprovalMode::Auto);
    }

    #[tokio::test]
    async fn invalid_mode() {
        let cmd = ApprovalCommand;
        let mut ctx = test_ctx();
        let result = cmd.execute(json!({"raw": "banana", "args": ["banana"]}), &mut ctx).await;
        assert!(result.is_err());
    }
}

mod run_cmd {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn run_echo() {
        let cmd = RunCommand;
        let mut ctx = test_ctx();
        let result = cmd.execute(json!({"raw": "echo hello", "args": ["echo", "hello"]}), &mut ctx).await.unwrap();
        assert!(result.contains("hello"));
    }

    #[tokio::test]
    async fn empty_command_err() {
        let cmd = RunCommand;
        let mut ctx = test_ctx();
        let result = cmd.execute(json!({"raw": "", "args": []}), &mut ctx).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn strict_mode_blocked() {
        let cmd = RunCommand;
        let mut ctx = test_ctx();
        ctx.approval_mode = ApprovalMode::Strict;
        let result = cmd.execute(json!({"raw": "echo hi", "args": ["echo", "hi"]}), &mut ctx).await;
        assert!(result.is_err());
    }
}

mod browse_cmd {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn browse_url() {
        let cmd = BrowseCommand;
        let mut ctx = test_ctx();
        let result = cmd.execute(json!({"raw": "https://example.com", "args": ["https://example.com"]}), &mut ctx).await.unwrap();
        assert!(result.contains("https://example.com"));
    }

    #[tokio::test]
    async fn empty_url_err() {
        let cmd = BrowseCommand;
        let mut ctx = test_ctx();
        let result = cmd.execute(json!({"raw": "", "args": []}), &mut ctx).await;
        assert!(result.is_err());
    }
}

mod help_cmd {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn help_all() {
        let cmd = HelpCommand;
        let mut ctx = test_ctx();
        let result = cmd.execute(json!({"raw": "", "args": []}), &mut ctx).await.unwrap();
        assert!(result.contains("/agent"));
        assert!(result.contains("/help"));
    }

    #[tokio::test]
    async fn help_search() {
        let cmd = HelpCommand;
        let mut ctx = test_ctx();
        let result = cmd.execute(json!({"raw": "review", "args": ["review"]}), &mut ctx).await.unwrap();
        assert!(result.contains("review"));
    }
}

mod config_cmd {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn list_config() {
        let cmd = ConfigCommand;
        let mut ctx = test_ctx();
        let result = cmd.execute(json!({"raw": "", "args": []}), &mut ctx).await.unwrap();
        assert!(result.contains("model"));
    }

    #[tokio::test]
    async fn set_model() {
        let cmd = ConfigCommand;
        let mut ctx = test_ctx();
        let result = cmd.execute(
            json!({"raw": "set model anthropic/claude-sonnet-4", "args": ["set", "model", "anthropic/claude-sonnet-4"]}),
            &mut ctx,
        ).await.unwrap();
        assert!(result.contains("anthropic/claude-sonnet-4"));
        assert_eq!(ctx.model, "anthropic/claude-sonnet-4");
    }

    #[tokio::test]
    async fn set_temperature_via_config() {
        let cmd = ConfigCommand;
        let mut ctx = test_ctx();
        let result = cmd.execute(
            json!({"raw": "set temperature 1.0", "args": ["set", "temperature", "1.0"]}),
            &mut ctx,
        ).await.unwrap();
        assert!(result.contains("1"));
        assert_eq!(ctx.temperature, Some(1.0));
    }

    #[tokio::test]
    async fn get_model() {
        let cmd = ConfigCommand;
        let mut ctx = test_ctx();
        let result = cmd.execute(json!({"raw": "get model", "args": ["get", "model"]}), &mut ctx).await.unwrap();
        assert!(result.contains("model"));
    }
}

// ── Registry integration test ────────────────────────────────────────

mod image_cmd {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn generate_image() {
        let cmd = ImageCommand;
        let mut ctx = test_ctx();
        let result = cmd
            .execute(json!({"raw": "a sunset over mountains", "args": ["a", "sunset", "over", "mountains"]}), &mut ctx)
            .await
            .unwrap();
        assert!(result.contains("a sunset over mountains"));
    }

    #[tokio::test]
    async fn empty_prompt_err() {
        let cmd = ImageCommand;
        let mut ctx = test_ctx();
        let result = cmd.execute(json!({"raw": "", "args": []}), &mut ctx).await;
        assert!(result.is_err());
    }
}

mod scrape_cmd {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn scrape_url() {
        let cmd = ScrapeCommand;
        let mut ctx = test_ctx();
        let result = cmd
            .execute(json!({"raw": "https://example.com", "args": ["https://example.com"]}), &mut ctx)
            .await
            .unwrap();
        assert!(result.contains("https://example.com"));
    }

    #[tokio::test]
    async fn empty_url_err() {
        let cmd = ScrapeCommand;
        let mut ctx = test_ctx();
        let result = cmd.execute(json!({"raw": "", "args": []}), &mut ctx).await;
        assert!(result.is_err());
    }
}

mod registry_integration {
    use super::*;
    use crate::builtins::register_all;

    #[tokio::test]
    async fn register_all_and_execute() {
        let mut reg = CommandRegistry::new();
        register_all(&mut reg);
        assert_eq!(reg.len(), 12);

        let mut ctx = test_ctx();

        // Test /help
        let result = reg.execute("/help", &mut ctx).await.unwrap().unwrap();
        assert!(result.contains("/agent"));

        // Test unknown
        let result = reg.execute("/nonexistent", &mut ctx).await.unwrap();
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

    #[tokio::test]
    async fn temperature_mutates_ctx() {
        let mut reg = CommandRegistry::new();
        register_all(&mut reg);
        let mut ctx = test_ctx();

        reg.execute("/temperature 1.5", &mut ctx).await.unwrap().unwrap();
        assert_eq!(ctx.temperature, Some(1.5));
    }

    #[tokio::test]
    async fn approval_mutates_ctx() {
        let mut reg = CommandRegistry::new();
        register_all(&mut reg);
        let mut ctx = test_ctx();

        reg.execute("/approval strict", &mut ctx).await.unwrap().unwrap();
        assert_eq!(ctx.approval_mode, ApprovalMode::Strict);
    }
}
