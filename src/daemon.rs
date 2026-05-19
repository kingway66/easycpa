//! Daemon management — PID file, daemonize, stop, reload (SIGHUP)

use std::fs;
use std::io;
use std::path::PathBuf;
use std::process::Command;

pub fn pid_path() -> PathBuf {
    dirs::home_dir()
        .expect("cannot determine home directory")
        .join(".easycpa")
        .join("easycpa.pid")
}

pub fn log_path() -> PathBuf {
    dirs::home_dir()
        .expect("cannot determine home directory")
        .join(".easycpa")
        .join("logs")
        .join("easycpa.log")
}

pub fn write_pid() -> io::Result<()> {
    let path = pid_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, std::process::id().to_string())
}

pub fn read_pid() -> Option<u32> {
    let content = fs::read_to_string(pid_path()).ok()?;
    let pid: u32 = content.trim().parse().ok()?;
    // Check if process is alive
    unsafe {
        // signal 0 = existence check, doesn't actually send a signal
        if libc::kill(pid as i32, 0) == 0 {
            Some(pid)
        } else {
            // Process doesn't exist, clean up stale PID file
            let _ = fs::remove_file(pid_path());
            None
        }
    }
}

pub fn remove_pid() {
    let _ = fs::remove_file(pid_path());
}

pub fn is_running() -> bool {
    read_pid().is_some()
}

/// Daemonize: spawn a child process with --foreground, redirect its
/// stdout/stderr to the log file, then exit the parent.
pub fn daemonize(port: Option<u16>, address: Option<String>, no_open: bool) -> Result<(), String> {
    if is_running() {
        return Err("EasyCPA is already running".to_string());
    }

    let log_file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path())
        .map_err(|e| format!("cannot open log file: {e}"))?;

    let log_file_clone = log_file.try_clone().map_err(|e| e.to_string())?;

    let exe = std::env::current_exe().map_err(|e| format!("cannot get current exe: {e}"))?;

    let mut args: Vec<String> = vec!["start".to_string(), "--foreground".to_string()];
    if let Some(p) = port {
        args.push("--port".to_string());
        args.push(p.to_string());
    }
    if let Some(a) = address {
        args.push("--address".to_string());
        args.push(a);
    }
    if no_open {
        args.push("--no-open".to_string());
    }

    let child = Command::new(exe)
        .args(&args)
        .stdout(log_file)
        .stderr(log_file_clone)
        .spawn()
        .map_err(|e| format!("failed to spawn daemon: {e}"))?;

    // Give the child a moment to start and write its PID
    std::thread::sleep(std::time::Duration::from_millis(500));

    // Check if child is still alive (didn't crash immediately)
    match child.id() {
        pid if pid > 0 => {
            println!("EasyCPA started (PID {pid})");
            Ok(())
        }
        _ => Err("daemon process exited immediately".to_string()),
    }
}

/// Stop the running daemon by sending SIGTERM.
pub fn stop_daemon() -> Result<(), String> {
    let pid = read_pid().ok_or("EasyCPA is not running")?;

    unsafe {
        if libc::kill(pid as i32, libc::SIGTERM) != 0 {
            return Err(format!("failed to send SIGTERM to PID {pid}"));
        }
    }

    // Wait for process to exit (up to 5 seconds)
    for _ in 0..50 {
        if !is_running() {
            remove_pid();
            println!("EasyCPA stopped");
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    // Force kill if still running
    unsafe {
        libc::kill(pid as i32, libc::SIGKILL);
    }
    remove_pid();
    println!("EasyCPA killed (did not respond to SIGTERM)");
    Ok(())
}

/// Send SIGHUP to the daemon to trigger config reload.
pub fn reload_daemon() -> Result<(), String> {
    let pid = read_pid().ok_or("EasyCPA is not running")?;

    unsafe {
        if libc::kill(pid as i32, libc::SIGHUP) != 0 {
            return Err(format!("failed to send SIGHUP to PID {pid}"));
        }
    }

    println!("Reload signal sent to EasyCPA (PID {pid})");
    Ok(())
}

/// Check status and print info.
pub fn print_status() {
    match read_pid() {
        Some(pid) => {
            let port = get_configured_port();
            println!("EasyCPA is running (PID {pid}, port {port})");
        }
        None => {
            println!("EasyCPA is not running");
        }
    }
}

/// Read the configured port from config.json or return default.
fn get_configured_port() -> u16 {
    let config_path = crate::config::get_config_json_path();
    fs::read_to_string(&config_path)
        .ok()
        .and_then(|content| serde_json::from_str::<serde_json::Value>(&content).ok())
        .and_then(|v| v.get("listen")?.as_str().map(String::from))
        .and_then(|listen| listen.rsplit_once(':').and_then(|(_, port)| port.parse().ok()))
        .unwrap_or(15791)
}

/// Open URL in default browser.
pub fn open_browser(url: &str) {
    #[cfg(target_os = "macos")]
    {
        let _ = Command::new("open").arg(url).spawn();
    }
    #[cfg(target_os = "linux")]
    {
        let _ = Command::new("xdg-open").arg(url).spawn();
    }
    #[cfg(target_os = "windows")]
    {
        let _ = Command::new("cmd").args(["/c", "start", url]).spawn();
    }
}
