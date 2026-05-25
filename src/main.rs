use std::path::{Path, PathBuf};

use anyhow::Result;
use clap::{Parser, Subcommand};
use serde::Deserialize;

// ── CLI definition ────────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(name = "hyprcollab", about = "HyprCollab AI collaboration platform")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the HTTP API server
    Serve {
        /// Port to listen on
        #[arg(long, default_value_t = 8420)]
        port: u16,
        /// Path to the SQLite database file
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Launch the terminal UI
    Tui,
}

// ── Config ────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Default)]
struct Config {
    openai_api_key: Option<String>,
    anthropic_api_key: Option<String>,
    default_provider: Option<String>,
    db_path: Option<PathBuf>,
}

fn config_file_path() -> PathBuf {
    std::env::var("HOME")
        .map(|h| PathBuf::from(h).join(".config/hyprcollab/config.yaml"))
        .unwrap_or_else(|_| PathBuf::from("config.yaml"))
}

fn load_config() -> Config {
    let path = config_file_path();
    if path.exists() {
        match std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_yaml::from_str::<Config>(&s).ok())
        {
            Some(cfg) => return cfg,
            None => tracing::warn!("failed to parse config at {}", path.display()),
        }
    }
    Config::default()
}

// ── Entry point ───────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Serve { port, db } => serve(port, db).await,
        Commands::Tui => hyprcollab_tui::run().await,
    }
}

async fn serve(port: u16, db_override: Option<PathBuf>) -> Result<()> {
    let mut config = load_config();

    // Environment variables override config file
    if let Ok(key) = std::env::var("OPENAI_API_KEY") {
        config.openai_api_key = Some(key);
    }
    if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
        config.anthropic_api_key = Some(key);
    }

    let db_path = db_override
        .or_else(|| config.db_path.take())
        .unwrap_or_else(default_db_path);

    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let artifact_dir = db_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("artifacts");

    let artifacts =
        hyprcollab_artifacts::ArtifactStore::new(db_path.clone(), artifact_dir).await?;

    let memory = hyprcollab_memory::MemoryStore::open(&db_path)
        .map_err(|e| anyhow::anyhow!("memory store: {e}"))?;

    let router = build_router(&config);

    let state = hyprcollab_server::AppState::new_full(artifacts, router, memory, None);
    let app = hyprcollab_server::create_app(state);

    let addr = format!("0.0.0.0:{port}");
    tracing::info!("hyprcollab server listening on {addr}");

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

fn build_router(config: &Config) -> hyprcollab_router::LlmRouter {
    use hyprcollab_router::LlmRouter;

    let mut router = LlmRouter::new();

    if let Some(key) = &config.openai_api_key {
        let mut client = hyprcollab_provider_openai::OpenAiClient::new(key);
        if let Ok(base_url) = std::env::var("OPENAI_BASE_URL") {
            client = client.with_base_url(&base_url);
        }
        router.register("openai", Box::new(client));
        if config.default_provider.as_deref().unwrap_or("openai") == "openai" {
            router.set_default("openai");
        }
    }

    if let Some(key) = &config.anthropic_api_key {
        router.register(
            "anthropic",
            Box::new(hyprcollab_provider_anthropic::AnthropicClient::new(key)),
        );
        if config.default_provider.as_deref() == Some("anthropic")
            || config.openai_api_key.is_none()
        {
            router.set_default("anthropic");
        }
    }

    router
}

fn default_db_path() -> PathBuf {
    std::env::var("HOME")
        .map(|h| {
            PathBuf::from(h)
                .join(".local/share/hyprcollab/hyprcollab.db")
        })
        .unwrap_or_else(|_| PathBuf::from("hyprcollab.db"))
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_parses_serve_defaults() {
        let cli = Cli::try_parse_from(["hyprcollab", "serve"]).unwrap();
        match cli.command {
            Commands::Serve { port, db } => {
                assert_eq!(port, 8420);
                assert!(db.is_none());
            }
            _ => panic!("expected Serve"),
        }
    }

    #[test]
    fn cli_parses_serve_with_port() {
        let cli = Cli::try_parse_from(["hyprcollab", "serve", "--port", "9000"]).unwrap();
        match cli.command {
            Commands::Serve { port, .. } => assert_eq!(port, 9000),
            _ => panic!("expected Serve"),
        }
    }

    #[test]
    fn cli_parses_serve_with_db() {
        let cli =
            Cli::try_parse_from(["hyprcollab", "serve", "--db", "/tmp/test.db"]).unwrap();
        match cli.command {
            Commands::Serve { db, .. } => {
                assert_eq!(db, Some(PathBuf::from("/tmp/test.db")));
            }
            _ => panic!("expected Serve"),
        }
    }

    #[test]
    fn cli_parses_tui() {
        let cli = Cli::try_parse_from(["hyprcollab", "tui"]).unwrap();
        assert!(matches!(cli.command, Commands::Tui));
    }

    #[test]
    fn cli_rejects_unknown_subcommand() {
        assert!(Cli::try_parse_from(["hyprcollab", "unknown"]).is_err());
    }

    #[test]
    fn config_default_is_empty() {
        let cfg = Config::default();
        assert!(cfg.openai_api_key.is_none());
        assert!(cfg.anthropic_api_key.is_none());
    }

    #[test]
    fn build_router_no_keys_is_empty() {
        let cfg = Config::default();
        let router = build_router(&cfg);
        assert!(router.provider_names().is_empty());
    }

    #[test]
    fn build_router_with_openai_key_registers_provider() {
        let cfg = Config {
            openai_api_key: Some("test-key".into()),
            ..Config::default()
        };
        let router = build_router(&cfg);
        assert!(router.provider_names().contains(&"openai".to_string()));
    }

    #[test]
    fn build_router_with_both_keys() {
        let cfg = Config {
            openai_api_key: Some("oai-key".into()),
            anthropic_api_key: Some("ant-key".into()),
            ..Config::default()
        };
        let router = build_router(&cfg);
        let names = router.provider_names();
        assert!(names.contains(&"openai".to_string()));
        assert!(names.contains(&"anthropic".to_string()));
    }
}
