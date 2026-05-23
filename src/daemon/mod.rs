pub mod server;

use crate::utils::paths;
use std::fs;
use std::thread;
use std::time::Duration;
use tracing::{info, warn};

const PID_FILE: &str = "hypr-collab.pid";

pub fn start() -> anyhow::Result<()> {
    let pid_path = paths::pid_path();

    // Check if already running
    if pid_path.exists() {
        if let Ok(pid_str) = fs::read_to_string(&pid_path) {
            if let Ok(pid) = pid_str.trim().parse::<i32>() {
                if is_process_running(pid) {
                    info!("Daemon already running (PID {})", pid);
                    println!("Daemon already running (PID {})", pid);
                    return Ok(());
                }
            }
        }
        warn!("Stale PID file, removing...");
        let _ = fs::remove_file(&pid_path);
    }

    // Ensure runtime dir
    let rt_dir = paths::runtime_dir()?;
    fs::create_dir_all(&rt_dir)?;

    // Spawn daemon process
    let exe = std::env::current_exe()?;
    let child = std::process::Command::new(&exe)
        .env("HYPR_COLLAB_DAEMON", "1")
        .spawn()?;

    let pid = child.id() as i32;
    fs::write(&pid_path, pid.to_string())?;

    info!("Daemon started (PID {})", pid);
    println!("Daemon started (PID {})", pid);
    Ok(())
}

pub fn stop() -> anyhow::Result<()> {
    let pid_path = paths::pid_path();

    if !pid_path.exists() {
        println!("Daemon not running");
        return Ok(());
    }

    if let Ok(pid_str) = fs::read_to_string(&pid_path) {
        if let Ok(pid) = pid_str.trim().parse::<i32>() {
            unsafe { libc::kill(pid, libc::SIGTERM) };
            for _ in 0..10 {
                if !is_process_running(pid) { break; }
                thread::sleep(Duration::from_millis(100));
            }
            if is_process_running(pid) {
                unsafe { libc::kill(pid, libc::SIGKILL) };
            }
            println!("Daemon stopped (PID {})", pid);
        }
    }

    let _ = fs::remove_file(&pid_path);
    let _ = fs::remove_file(paths::socket_path());
    Ok(())
}

pub fn status() -> anyhow::Result<()> {
    let pid_path = paths::pid_path();
    if pid_path.exists() {
        if let Ok(pid_str) = fs::read_to_string(&pid_path) {
            if let Ok(pid) = pid_str.trim().parse::<i32>() {
                if is_process_running(pid) {
                    println!("Running (PID {})", pid);
                    println!("Socket: {}", paths::socket_path().display());
                    return Ok(());
                }
            }
        }
    }
    println!("Not running");
    Ok(())
}

/// Run the daemon main loop (called when HYPR_COLLAB_DAEMON=1)
pub fn run_daemon() -> anyhow::Result<()> {
    info!("HyprCollab daemon starting...");

    let rt_dir = paths::runtime_dir()?;
    fs::create_dir_all(&rt_dir)?;
    fs::create_dir_all(paths::data_dir())?;
    fs::create_dir_all(paths::log_dir())?;

    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(server::run_ipc_server())?;

    Ok(())
}

fn is_process_running(pid: i32) -> bool {
    unsafe { libc::kill(pid, 0) == 0 }
}
