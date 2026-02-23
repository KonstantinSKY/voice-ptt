use anyhow::{Context, Result};
use std::process::Command;
use std::time::Duration;

pub struct SystemInjector;

impl SystemInjector {
    /// Plays an audio file using system tools.
    pub fn play_sound(enabled: bool, path: &str) {
        if !enabled {
            return;
        }

        #[cfg(target_os = "linux")]
        let _ = Command::new("paplay").arg(path).spawn().ok();

        #[cfg(target_os = "macos")]
        let _ = Command::new("afplay").arg(path).spawn().ok();
    }

    /// Sends a system notification.
    pub fn notify(title: &str, message: &str) {
        #[cfg(target_os = "linux")]
        let _ = Command::new("notify-send")
            .arg(title)
            .arg(message)
            .arg("-t")
            .arg("5000")
            .spawn();

        #[cfg(target_os = "macos")]
        {
            let script = format!(
                "display notification \"{}\" with title \"{}\"",
                message.replace("\"", "\\\""),
                title.replace("\"", "\\\"")
            );
            let _ = Command::new("osascript")
                .arg("-e")
                .arg(script)
                .spawn();
        }
    }

    /// Injects text as keyboard input.
    pub async fn type_text(text: &str, delay_ms: u64, initial_delay_ms: u64) -> Result<()> {
        if text.is_empty() {
            return Ok(());
        }

        // Wait a bit before typing to ensure the user has released the PTT key modifiers
        tokio::time::sleep(Duration::from_millis(initial_delay_ms)).await;

        #[cfg(target_os = "linux")]
        {
            let args = vec![
                "type".to_string(),
                "--clearmodifiers".to_string(),
                "--delay".to_string(),
                delay_ms.to_string(),
                text.to_string(),
            ];
            Command::new("xdotool")
                .args(&args)
                .status()
                .context("Failed to execute xdotool. Is it installed?")?;
        }

        #[cfg(target_os = "macos")]
        {
            // AppleScript for typing. 
            // Note: This requires Accessibility permissions for the terminal/app on macOS.
            let script = format!(
                "tell application \"System Events\" to keystroke \"{}\"",
                text.replace("\"", "\\\"")
            );
            Command::new("osascript")
                .arg("-e")
                .arg(script)
                .status()
                .context("Failed to execute osascript for typing")?;
        }

        Ok(())
    }

    /// Verifies that required system tools are available.
    pub fn check_dependencies() -> Result<()> {
        #[cfg(target_os = "linux")]
        if Command::new("xdotool").arg("--version").output().is_err() {
            anyhow::bail!("'xdotool' is required but not found in PATH.");
        }
        
        // MacOS usually has osascript and afplay by default
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xdotool_args_formation() {
        let text = "Hello World";
        let delay = 50;
        let args = SystemInjector::get_xdotool_args(text, delay);

        assert_eq!(args[0], "type");
        assert_eq!(args[1], "--clearmodifiers");
        assert_eq!(args[2], "--delay");
        assert_eq!(args[3], "50");
        assert_eq!(args[4], "Hello World");
    }

    #[test]
    fn test_empty_text_args() {
        let args = SystemInjector::get_xdotool_args("", 0);
        assert_eq!(args[4], "");
    }
}
