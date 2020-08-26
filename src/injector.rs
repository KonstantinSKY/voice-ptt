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
        if let Err(e) = Command::new("afplay").arg(path).spawn() {
            eprintln!("❌ Failed to play sound {}: {}", path, e);
        }
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
    pub async fn type_text(
        text: &str,
        _delay_ms: u64,
        initial_delay_ms: u64,
        config: &crate::config::AppConfig,
    ) -> Result<()> {
        if text.is_empty() {
            return Ok(());
        }

        // Wait a bit before typing to ensure the user has released the PTT key modifiers
        tokio::time::sleep(Duration::from_millis(initial_delay_ms)).await;

        #[cfg(target_os = "linux")]
        {
            // Use xsel to set the clipboard selection
            use std::io::Write;
            let mut child = Command::new("xsel")
                .args(&["--clipboard", "--input"])
                .stdin(std::process::Stdio::piped())
                .spawn()
                .context("Failed to execute xsel. Is it installed?")?;

            if let Some(mut stdin) = child.stdin.take() {
                stdin.write_all(text.as_bytes())?;
            }
            // stdin is dropped here, closing the pipe.
            // xsel reads until EOF, then takes over the clipboard and exits.
            child.wait()?;

            // Small delay to ensure the clipboard is ready before we simulate the paste command
            tokio::time::sleep(Duration::from_millis(50)).await;

            // Detect if the active window has an override in the config
            let mut paste_key = "ctrl+v".to_string();

            let output = Command::new("xdotool")
                .args(&["getactivewindow", "getwindowclassname"])
                .output();

            if let Ok(out) = output {
                let window_class = String::from_utf8_lossy(&out.stdout).trim().to_string();
                let lower_class = window_class.to_lowercase();
                
                println!("📌 Detected window class: '{}'", window_class);
                
                // Case-insensitive lookup in overrides map
                let mut found = false;
                for (name, shortcut) in &config.paste_overrides {
                    if name.to_lowercase() == lower_class {
                        paste_key = shortcut.clone();
                        found = true;
                        break;
                    }
                }
                
                if !found {
                    // Default to ctrl+v if no match found in config
                    paste_key = "ctrl+v".to_string();
                }
                
                println!("⌨️ Using paste shortcut: '{}'", paste_key);
            }

            // Simulate the paste shortcut (either default ctrl+v or override from config)
            Command::new("xdotool")
                .args(&["key", "--clearmodifiers", &paste_key])
                .status()
                .context("Failed to execute xdotool for pasting")?;
        }

        #[cfg(target_os = "macos")]
        {
            // For macOS, we use the clipboard to handle Unicode (like Russian) correctly.
            // We save the current clipboard, set it to our text, paste it, and restore the old clipboard.
            let script = format!(
                "set oldClipboard to the clipboard\n\
                 set the clipboard to \"{}\"\n\
                 tell application \"System Events\"\n\
                     keystroke \"v\" using command down\n\
                 end tell\n\
                 delay 0.1\n\
                 set the clipboard to oldClipboard",
                text.replace("\"", "\\\"").replace("\\", "\\\\")
            );
            Command::new("osascript")
                .arg("-e")
                .arg(script)
                .status()
                .context("Failed to execute osascript for typing via clipboard")?;
        }

        Ok(())
    }

    /// Verifies that required system tools are available.
    pub fn check_dependencies() -> Result<()> {
        #[cfg(target_os = "linux")]
        {
            if Command::new("xdotool").arg("--version").output().is_err() {
                anyhow::bail!("'xdotool' is required but not found in PATH.");
            }
            if Command::new("xsel").arg("--version").output().is_err() {
                anyhow::bail!("'xsel' is required for fast text injection. Please install it (e.g., sudo pacman -S xsel).");
            }
        }
        
        // MacOS usually has osascript and afplay by default
        Ok(())
    }
}
