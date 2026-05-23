pub fn runtime_dir() -> anyhow::Result<std::path::PathBuf> {
    let proj = project_dirs();
    let dir = proj.runtime_dir();
    match dir {
        Some(d) => Ok(d.to_path_buf()),
        None => {
            let fallback = std::env::var("XDG_RUNTIME_DIR")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|_| std::path::PathBuf::from("/tmp"))
                .join("hypr-collab");
            Ok(fallback)
        }
    }
}

use directories::ProjectDirs;
use std::path::PathBuf;

fn project_dirs() -> ProjectDirs {
    ProjectDirs::from("com", "hbuddenberg", "hypr-collab")
        .expect("Cannot determine project directories")
}

pub fn data_dir() -> PathBuf {
    project_dirs().data_dir().to_path_buf()
}

pub fn config_dir() -> PathBuf {
    project_dirs().config_dir().to_path_buf()
}

pub fn cache_dir() -> PathBuf {
    project_dirs().cache_dir().to_path_buf()
}

pub fn log_dir() -> PathBuf {
    project_dirs().data_local_dir().join("logs")
}

pub fn waybar_status_path() -> PathBuf {
    cache_dir().join("waybar.json")
}

pub fn pid_path() -> PathBuf {
    runtime_dir()
        .unwrap_or_else(|_| PathBuf::from("/tmp"))
        .join("hypr-collab.pid")
}

pub fn socket_path() -> PathBuf {
    runtime_dir()
        .unwrap_or_else(|_| PathBuf::from("/tmp"))
        .join("hypr-collab.sock")
}

pub fn config_file() -> PathBuf {
    config_dir().join("config.yaml")
}
