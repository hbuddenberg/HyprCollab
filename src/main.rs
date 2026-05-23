use clap::{Parser, Subcommand};

mod daemon;
use hyprcollab::gui;
use hyprcollab::storage;
use hyprcollab::utils;
use hyprcollab::waybar;

/// HyprCollab — AI chat agent for Hyprland
#[derive(Parser)]
#[command(name = "hyprcollab")]
#[command(about = "AI chat agent for Hyprland — multi-agent conversations with persistent memory")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start/stop/restart the daemon process
    Daemon {
        #[command(subcommand)]
        action: DaemonAction,
    },
    /// Show/hide/toggle the GUI window
    Gui {
        #[command(subcommand)]
        action: GuiActionClap,
    },
    /// Install/remove/update the Waybar module
    Waybar {
        #[command(subcommand)]
        action: WaybarAction,
    },
    /// Manage Hyprland keybinds for HyprCollab
    Keys {
        #[command(subcommand)]
        action: KeyAction,
    },
    /// One-command setup: config + waybar + keybinds
    Setup {
        /// Force reinstall everything
        #[arg(long)]
        force: bool,
    },
    /// Output JSON for Waybar custom module
    #[command(name = "waybar-status")]
    WaybarStatus,
}

#[derive(Subcommand, Clone)]
enum DaemonAction {
    Start,
    Stop,
    Restart,
    Status,
}

#[derive(Subcommand, Clone)]
enum GuiActionClap {
    Show,
    Hide,
    Toggle,
}

impl From<GuiActionClap> for gui::GuiAction {
    fn from(action: GuiActionClap) -> Self {
        match action {
            GuiActionClap::Show => gui::GuiAction::Show,
            GuiActionClap::Hide => gui::GuiAction::Hide,
            GuiActionClap::Toggle => gui::GuiAction::Toggle,
        }
    }
}

#[derive(Subcommand, Clone)]
enum WaybarAction {
    Install { #[arg(long)] force: bool },
    Remove,
    Status,
    Update,
}

#[derive(Subcommand, Clone)]
enum KeyAction {
    Install,
    Remove,
    Status,
}

fn main() -> anyhow::Result<()> {
    // If spawned as daemon (env var set), run daemon directly
    if std::env::var("HYPR_COLLAB_DAEMON").is_ok() {
        utils::logging::init();
        return daemon::run_daemon();
    }

    let cli = Cli::parse();
    utils::logging::init();

    match cli.command {
        Commands::Daemon { action } => match action {
            DaemonAction::Start => {
                daemon::start()?;
                if std::env::var("HYPR_COLLAB_DAEMON").is_ok() {
                    daemon::run_daemon()?;
                }
            }
            DaemonAction::Stop => daemon::stop()?,
            DaemonAction::Restart => {
                daemon::stop()?;
                daemon::start()?;
                if std::env::var("HYPR_COLLAB_DAEMON").is_ok() {
                    daemon::run_daemon()?;
                }
            }
            DaemonAction::Status => daemon::status()?,
        },
        Commands::Gui { action } => {
            gui::handle_action(action.into());
        }
        Commands::Waybar { action } => match action {
            WaybarAction::Install { force } => waybar::handle_install(force)?,
            WaybarAction::Remove => waybar::handle_remove()?,
            WaybarAction::Status => waybar::handle_status()?,
            WaybarAction::Update => waybar::handle_update()?,
        },
        Commands::Keys { action } => match action {
            KeyAction::Install => {
                let config = storage::Config::load()?;
                waybar::keybinds::install_keybinds(&config.keys)?;
            }
            KeyAction::Remove => waybar::keybinds::remove_keybinds()?,
            KeyAction::Status => waybar::keybinds::show_status()?,
        },
        Commands::Setup { force } => {
            let _config = storage::Config::load()?;
            println!("Config created");
            waybar::handle_install(force)?;
            let config = storage::Config::load()?;
            waybar::keybinds::install_keybinds(&config.keys)?;
            println!("Setup complete! Run: hyprcollab daemon start");
        }
        Commands::WaybarStatus => {
            waybar::print_status()?;
        }
    }

    Ok(())
}
