//! Systemd autostart helpers — generate a Linux systemd unit file for OmniRoute.
//!
//! Routes:
//!   POST /api/dashboard/systemd/install    — write unit file + enable service
//!   POST /api/dashboard/systemd/uninstall  — disable + remove unit file
//!   GET  /api/dashboard/systemd/status     — is the unit installed?
//!
//! Linux only. Returns 400 on non-Linux platforms.

use std::path::PathBuf;
use axum::extract::State;
use axum::Json;
use serde_json::{json, Value};
use sqlx::SqlitePool;

use crate::error::{AppError, AppResult};
use crate::middleware::auth::DashboardUser;

fn unit_path() -> PathBuf {
    std::env::var("XDG_CONFIG_HOME").ok()
        .map(|d| PathBuf::from(d).join("systemd").join("user").join("omniroute.service"))
        .unwrap_or_else(|| {
            dirs::home_dir()
                .map(|h| h.join(".config").join("systemd").join("user").join("omniroute.service"))
                .unwrap_or_else(|| PathBuf::from("omniroute.service"))
        })
}

fn exec_path() -> Option<String> {
    std::env::current_exe().ok().map(|p| p.to_string_lossy().to_string())
}

fn env_file_path() -> Option<String> {
    std::env::var("DATA_DIR").ok()
        .map(|d| format!("{}/.env", d))
        .or_else(|| {
            dirs::home_dir().map(|h| format!("{}/.omniroute/.env", h.display()))
        })
}

pub async fn install(_user: DashboardUser) -> AppResult<Json<Value>> {
    #[cfg(not(target_os = "linux"))]
    {
        return Err(AppError::BadRequest("systemd autostart is only available on Linux".into()));
    }

    #[cfg(target_os = "linux")]
    {
        let exec = exec_path().ok_or_else(|| AppError::Internal("cannot resolve current exe".into()))?;
        let env_file = env_file_path();
        let path = unit_path();

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| AppError::Internal(format!("failed to create systemd dir: {}", e)))?;
        }

        let env_line = env_file.as_ref()
            .map(|p| format!("EnvironmentFile={}\n", p))
            .unwrap_or_default();

        let unit = format!(
            "[Unit]\n\
             Description=OmniRoute — OpenAI-compatible AI gateway\n\
             After=network.target\n\
             \n\
             [Service]\n\
             Type=simple\n\
             ExecStart={exec}\n\
             {env_line}\
             Restart=on-failure\n\
             RestartSec=5\n\
             \n\
             [Install]\n\
             WantedBy=default.target\n",
            exec = exec,
            env_line = env_line,
        );

        std::fs::write(&path, &unit)
            .map_err(|e| AppError::Internal(format!("failed to write unit file: {}", e)))?;

        // Try to reload + enable (best-effort — may fail without user-session systemd)
        let _ = std::process::Command::new("systemctl")
            .args(["--user", "daemon-reload"])
            .status();
        let _ = std::process::Command::new("systemctl")
            .args(["--user", "enable", "omniroute.service"])
            .status();

        Ok(Json(json!({
            "success": true,
            "unit_path": path.to_string_lossy(),
            "note": "systemd user service installed. Start with: systemctl --user start omniroute",
        })))
    }
}

pub async fn uninstall(_user: DashboardUser) -> AppResult<Json<Value>> {
    #[cfg(not(target_os = "linux"))]
    {
        return Err(AppError::BadRequest("systemd autostart is only available on Linux".into()));
    }

    #[cfg(target_os = "linux")]
    {
        let path = unit_path();
        // Disable first
        let _ = std::process::Command::new("systemctl")
            .args(["--user", "disable", "omniroute.service"])
            .status();
        let _ = std::process::Command::new("systemctl")
            .args(["--user", "stop", "omniroute.service"])
            .status();

        let removed = path.exists();
        if removed {
            std::fs::remove_file(&path)
                .map_err(|e| AppError::Internal(format!("failed to remove unit file: {}", e)))?;
        }
        let _ = std::process::Command::new("systemctl")
            .args(["--user", "daemon-reload"])
            .status();

        Ok(Json(json!({ "success": true, "removed": removed })))
    }
}

pub async fn status(_user: DashboardUser) -> AppResult<Json<Value>> {
    let path = unit_path();
    Ok(Json(json!({
        "installed": path.exists(),
        "unit_path": path.to_string_lossy(),
    })))
}

// Silence unused import warnings on non-Linux platforms.
#[allow(dead_code)]
fn _silence_warnings() -> sqlx::SqlitePool {
    unimplemented!()
}
