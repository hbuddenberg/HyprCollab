use tracing_subscriber::{fmt, prelude::*, EnvFilter};
use tracing_appender::rolling;

use super::paths;

/// Initialize logging — stdout + file
pub fn init() {
    let is_daemon = std::env::var("HYPR_COLLAB_DAEMON").is_ok();

    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));

    if is_daemon {
        let file_appender = rolling::daily(paths::log_dir(), "hypr-collab.log");
        let file_layer = fmt::layer()
            .with_writer(file_appender)
            .with_ansi(false)
            .with_target(true)
            .with_thread_ids(true);

        tracing_subscriber::registry()
            .with(env_filter)
            .with(file_layer)
            .init();
    } else {
        let stdout_layer = fmt::layer()
            .with_writer(std::io::stdout)
            .with_ansi(true)
            .with_target(true);

        tracing_subscriber::registry()
            .with(env_filter)
            .with(stdout_layer)
            .init();
    }
}
