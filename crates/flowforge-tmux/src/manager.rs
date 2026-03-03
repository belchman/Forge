use std::process::Command;

use flowforge_core::TmuxState;
use tracing::warn;

use crate::display::render_display;

pub struct TmuxManager {
    session_name: String,
}

impl TmuxManager {
    pub fn new(session_name: &str) -> Self {
        Self {
            session_name: session_name.to_string(),
        }
    }

    pub fn is_available(&self) -> bool {
        Command::new("which")
            .arg("tmux")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    pub fn session_exists(&self) -> bool {
        Command::new("tmux")
            .args(["has-session", "-t", &self.session_name])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    pub fn start(&self, state: &TmuxState) -> flowforge_core::Result<()> {
        if !self.is_available() {
            return Err(flowforge_core::Error::Tmux(
                "tmux is not installed or not in PATH".to_string(),
            ));
        }

        if self.session_exists() {
            return self.update(state);
        }

        let status = Command::new("tmux")
            .args([
                "new-session",
                "-d",
                "-s",
                &self.session_name,
                "-x",
                "60",
                "-y",
                "30",
            ])
            .status()
            .map_err(|e| flowforge_core::Error::Tmux(format!("failed to start tmux: {}", e)))?;

        if !status.success() {
            return Err(flowforge_core::Error::Tmux(
                "tmux new-session failed".to_string(),
            ));
        }

        self.update(state)
    }

    pub fn update(&self, state: &TmuxState) -> flowforge_core::Result<()> {
        if !self.session_exists() {
            warn!(
                "tmux session '{}' does not exist, skipping update",
                self.session_name
            );
            return Ok(());
        }

        let display = render_display(state);

        // Write display content to a temp file
        let tmp_path =
            std::env::temp_dir().join(format!("flowforge-tmux-{}.txt", self.session_name));
        std::fs::write(&tmp_path, &display)?;

        // Clear the pane and display the content using load-buffer to avoid shell injection
        let _ = Command::new("tmux")
            .args(["send-keys", "-t", &self.session_name, "clear", "Enter"])
            .status();
        let _ = Command::new("tmux")
            .args(["load-buffer", &tmp_path.display().to_string()])
            .status();
        let _ = Command::new("tmux")
            .args([
                "send-keys",
                "-t",
                &self.session_name,
                "tmux show-buffer",
                "Enter",
            ])
            .status();

        Ok(())
    }

    pub fn stop(&self) -> flowforge_core::Result<()> {
        if !self.session_exists() {
            return Ok(());
        }

        let status = Command::new("tmux")
            .args(["kill-session", "-t", &self.session_name])
            .status()
            .map_err(|e| {
                flowforge_core::Error::Tmux(format!("failed to kill tmux session: {}", e))
            })?;

        if !status.success() {
            return Err(flowforge_core::Error::Tmux(
                "tmux kill-session failed".to_string(),
            ));
        }

        // Clean up temp file
        let tmp_path =
            std::env::temp_dir().join(format!("flowforge-tmux-{}.txt", self.session_name));
        let _ = std::fs::remove_file(tmp_path);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let mgr = TmuxManager::new("test-session");
        assert_eq!(mgr.session_name, "test-session");
    }

    #[test]
    fn test_is_available() {
        let mgr = TmuxManager::new("test");
        let _ = mgr.is_available();
    }
}
