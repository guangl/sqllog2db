use crate::error::{Result, UpdateError};
use log::{info, warn};
use self_update::cargo_crate_version;

/// Handle the self-update command
pub fn handle_update(check: bool) -> Result<()> {
    let current_version = cargo_crate_version!();
    info!("Current version: {current_version}");

    let status = self_update::backends::github::Update::configure()
        .repo_owner("guangl")
        .repo_name("sqllog2db")
        .bin_name("sqllog2db")
        .show_download_progress(true)
        .current_version(current_version)
        .build()
        .map_err(|e| {
            let err_msg = e.to_string();
            if err_msg.contains("reqwest") || err_msg.contains("network") {
                 UpdateError::UpdateFailed("Network error or GitHub API unreachable. Please check your internet connection.".to_string())
            } else {
                 UpdateError::UpdateFailed(err_msg)
            }
        })?;

    if check {
        let release = status.get_latest_release().map_err(|e| {
            let err_msg = e.to_string();
            if err_msg.contains("reqwest") || err_msg.contains("network") {
                UpdateError::CheckFailed(
                    "Network error: Unable to connect to GitHub to check for updates.".to_string(),
                )
            } else {
                UpdateError::CheckFailed(err_msg)
            }
        })?;
        if self_update::version::bump_is_greater(current_version, &release.version).unwrap_or(false)
        {
            info!("New version available: {}", release.version);
            info!("Run 'sqllog2db self-update' to update.");
        } else {
            info!("You are already using the latest version.");
        }
        return Ok(());
    }

    let release = status.update().map_err(|e| {
        let err_msg = e.to_string();
        if err_msg.contains("reqwest") || err_msg.contains("network") {
            UpdateError::UpdateFailed(
                "Network error during update. Please check your internet connection.".to_string(),
            )
        } else {
            UpdateError::UpdateFailed(err_msg)
        }
    })?;
    if release.updated() {
        info!("Successfully updated to version: {}", release.version());
    } else {
        info!("You are already using the latest version.");
    }

    Ok(())
}

/// Check for updates at startup (silently if no update found)
pub fn check_for_updates_at_startup() {
    std::thread::spawn(|| {
        let current_version = cargo_crate_version!();

        let status = self_update::backends::github::Update::configure()
            .repo_owner("guangl")
            .repo_name("sqllog2db")
            .bin_name("sqllog2db")
            .current_version(current_version)
            .build();

        if let Ok(status) = status {
            if let Ok(release) = status.get_latest_release() {
                if self_update::version::bump_is_greater(current_version, &release.version)
                    .unwrap_or(false)
                {
                    warn!(
                        "A new version is available: {} (current: {})",
                        release.version, current_version
                    );
                    warn!("Run 'sqllog2db self-update' to update.");
                }
            }
        }
    });
    // 不保留 JoinHandle，fire-and-forget（per D-05）
}
