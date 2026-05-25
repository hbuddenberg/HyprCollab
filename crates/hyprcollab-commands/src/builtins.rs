//! Built-in slash commands.

mod agent;
mod skill;
mod temperature;
mod approval;
mod r#run;
mod browse;
mod design;
mod review;
mod help;
mod config_cmd;
mod image_cmd;
mod scrape;

pub use agent::AgentCommand;
pub use skill::SkillCommand;
pub use temperature::TemperatureCommand;
pub use approval::ApprovalCommand;
pub use r#run::RunCommand;
pub use browse::BrowseCommand;
pub use design::DesignCommand;
pub use review::ReviewCommand;
pub use help::HelpCommand;
pub use config_cmd::ConfigCommand;
pub use image_cmd::ImageCommand;
pub use scrape::ScrapeCommand;

use crate::registry::CommandRegistry;

/// Register all built-in commands into a registry.
pub fn register_all(registry: &mut CommandRegistry) {
    registry.register(Box::new(AgentCommand));
    registry.register(Box::new(SkillCommand));
    registry.register(Box::new(TemperatureCommand));
    registry.register(Box::new(ApprovalCommand));
    registry.register(Box::new(RunCommand));
    registry.register(Box::new(BrowseCommand));
    registry.register(Box::new(DesignCommand));
    registry.register(Box::new(ReviewCommand));
    registry.register(Box::new(HelpCommand));
    registry.register(Box::new(ConfigCommand));
    registry.register(Box::new(ImageCommand));
    registry.register(Box::new(ScrapeCommand));
}
