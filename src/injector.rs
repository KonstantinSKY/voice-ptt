use anyhow::{Context, Result};
use std::process::Command;
use std::time::Duration;

/// How long to wait for the user to release all modifier keys before pasting.
#[cfg(target_os = "linux")]
const MODIFIER_RELEASE_TIMEOUT: Duration = Duration::from_secs(3);

/// Polling interval while waiting for modifier keys to be released.
#[cfg(target_os = "linux")]
const MODIFIER_POLL_INTERVAL: Duration = Duration::from_millis(10);

#[cfg(target_os = "linux")]
fn is_modifier(key: &device_query::Keycode) -> bool {
    use device_query::Keycode::*;
    matches!(
        key,
        LControl | RControl | LShift | RShift | LAlt | RAlt | LMeta | RMeta
    )
}

/// Blocks until no modifier key is physically held, or until `timeout` expires.
/// Returns the modifiers still held on timeout (empty vec means all released).
///
/// We wait instead of using `xdotool key --clearmodifiers`: that flag re-presses
/// the modifiers it cleared via XTEST after the keystroke, and if the user
/// releases the physical key in the meantime the modifier stays stuck in the
/// X server (i3 bindings then stop matching until the key is tapped again).
#[cfg(target_os = "linux")]
fn wait_for_modifiers_release(timeout: Duration) -> Vec<device_query::Keycode> {
    use device_query::{DeviceQuery, DeviceState};

    let device_state = DeviceState::new();
    let deadline = std::time::Instant::now() + timeout;
    loop {
        let held: Vec<_> = device_state
            .get_keys()
            .into_iter()
            .filter(is_modifier)
            .collect();
        if held.is_empty() || std::time::Instant::now() >= deadline {
            return held;
        }
        std::thread::sleep(MODIFIER_POLL_INTERVAL);
    }
}

pub struct SystemInjector;

impl SystemInjector {
    /// Plays an audio file using system tools.
    pub fn play_sound(enabled: bool, path: &str) {
        if !enabled {
            return;
        }

        let path = path.to_string();
        std::thread::spawn(move || {
            #[cfg(target_os = "linux")]
            let result = Command::new("paplay").arg(&path).spawn();

            #[cfg(target_os = "macos")]
            let result = Command::new("afplay").arg(&path).spawn();

            match result {
                Ok(mut child) => { let _ = child.wait(); }
                Err(e) => eprintln!("❌ Failed to play sound {}: {}", path, e),
            }
        });
    }

    /// Sends a system notification.
    pub fn notify(title: &str, message: &str) {
        let title = title.to_string();
        let message = message.to_string();
        std::thread::spawn(move || {
            #[cfg(target_os = "linux")]
            let result = Command::new("notify-send")
                .arg(&title)
                .arg(&message)
                .arg("-t")
                .arg("5000")
                .spawn();

            #[cfg(target_os = "macos")]
            let result = {
                let script = format!(
                    "display notification \"{}\" with title \"{}\"",
                    message.replace('\\', "\\\\").replace('"', "\\\""),
                    title.replace('\\', "\\\\").replace('"', "\\\"")
                );
                Command::new("osascript")
                    .arg("-e")
                    .arg(script)
                    .spawn()
            };

            match result {
                Ok(mut child) => { let _ = child.wait(); }
                Err(e) => eprintln!("❌ Failed to send notification: {}", e),
            }
        });
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
                for (name, shortcut) in &config.paste_overrides {
                    if name.to_lowercase() == lower_class {
                        paste_key = shortcut.clone();
                        break;
                    }
                }

                println!("⌨️ Using paste shortcut: '{}'", paste_key);
            }

            // Make sure no modifier is physically held, otherwise it would be
            // merged into the paste shortcut (e.g. Super+Ctrl+Shift+V).
            let held = tokio::task::spawn_blocking(|| {
                wait_for_modifiers_release(MODIFIER_RELEASE_TIMEOUT)
            })
            .await
            .context("Modifier wait task failed")?;

            if !held.is_empty() {
                println!(
                    "⚠️ Modifiers still held after {:?}: {:?}. Paste skipped, text is in the clipboard.",
                    MODIFIER_RELEASE_TIMEOUT, held
                );
                Self::notify(
                    "Voice PTT",
                    "Paste skipped: modifier key held. Text is in the clipboard.",
                );
                return Ok(());
            }

            // Simulate the paste shortcut (either default ctrl+v or override from config).
            // No --clearmodifiers: see wait_for_modifiers_release().
            let status = Command::new("xdotool")
                .args(&["key", &paste_key])
                .status()
                .context("Failed to execute xdotool for pasting")?;

            // Safety net: explicitly release the modifiers of the paste shortcut on
            // the XTEST device, so nothing can stay stuck even if xdotool misbehaved.
            let _ = Command::new("xdotool")
                .args(&["keyup", "ctrl", "shift"])
                .status();

            if !status.success() {
                anyhow::bail!("xdotool paste exited with {}", status);
            }
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
                text.replace('\\', "\\\\").replace('"', "\\\"")
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
