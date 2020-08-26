use anyhow::{Context, Result};
use device_query::Keycode;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::str::FromStr;

#[derive(Deserialize, Clone)]
pub struct AppConfig {
    pub ptt_key: String,
    pub typing_delay_ms: u64,
    pub initial_delay_ms: u64,
    pub model: String,
    pub language: Option<String>,
    pub sound_enabled: bool,
    pub sound_start_path: String,
    pub sound_end_path: String,
    #[allow(dead_code)]
    pub macos_sound_start_path: Option<String>,
    #[allow(dead_code)]
    pub macos_sound_end_path: Option<String>,
    #[allow(dead_code)]
    pub linux_sound_start_path: Option<String>,
    #[allow(dead_code)]
    pub linux_sound_end_path: Option<String>,
    #[serde(default)]
    pub paste_overrides: HashMap<String, String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            ptt_key: "RControl".to_string(),
            typing_delay_ms: 50,
            initial_delay_ms: 150,
            model: "whisper-1".to_string(),
            language: None,
            sound_enabled: true,
            sound_start_path: "/usr/share/sounds/freedesktop/stereo/audio-volume-change.oga"
                .to_string(),
            sound_end_path: "/usr/share/sounds/freedesktop/stereo/screen-capture.oga".to_string(),
            macos_sound_start_path: Some("/System/Library/Sounds/Tink.aiff".to_string()),
            macos_sound_end_path: Some("/System/Library/Sounds/Morse.aiff".to_string()),
            linux_sound_start_path: Some(
                "/usr/share/sounds/freedesktop/stereo/audio-volume-change.oga".to_string(),
            ),
            linux_sound_end_path: Some(
                "/usr/share/sounds/freedesktop/stereo/screen-capture.oga".to_string(),
            ),
            paste_overrides: HashMap::new(),
        }
    }
}

impl AppConfig {
    /// Loads configuration from a TOML file. Falls back to defaults if file is missing.
    pub fn load(path: &Path) -> Result<Self> {
        if path.exists() {
            let content = fs::read_to_string(path)
                .with_context(|| format!("Failed to read config file at {:?}", path))?;
            let config: AppConfig =
                toml::from_str(&content).with_context(|| "Failed to parse config.toml")?;
            Ok(config)
        } else {
            Ok(Self::default())
        }
    }

    /// Returns (start_path, end_path) based on current OS
    pub fn get_sound_paths(&self) -> (String, String) {
        #[cfg(target_os = "macos")]
        {
            let start = self
                .macos_sound_start_path
                .as_ref()
                .unwrap_or(&self.sound_start_path);
            let end = self
                .macos_sound_end_path
                .as_ref()
                .unwrap_or(&self.sound_end_path);
            (start.clone(), end.clone())
        }
        #[cfg(not(target_os = "macos"))]
        {
            let start = self
                .linux_sound_start_path
                .as_ref()
                .unwrap_or(&self.sound_start_path);
            let end = self
                .linux_sound_end_path
                .as_ref()
                .unwrap_or(&self.sound_end_path);
            (start.clone(), end.clone())
        }
    }

    /// Parses the configured PTT key string into a DeviceQuery Keycode.
    pub fn get_ptt_keycode(&self) -> Keycode {
        Keycode::from_str(&self.ptt_key).unwrap_or_else(|_| {
            eprintln!("Invalid key '{}', defaulting to RControl", self.ptt_key);
            Keycode::RControl
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AppConfig::default();
        assert_eq!(config.ptt_key, "RControl");
        assert_eq!(config.model, "whisper-1");
        assert!(config.sound_enabled);
    }

    #[test]
    fn test_keycode_parsing() {
        let mut config = AppConfig::default();
        config.ptt_key = "LAlt".to_string();
        assert_eq!(config.get_ptt_keycode(), Keycode::LAlt);

        config.ptt_key = "InvalidKeyName".to_string();
        // Should fallback to RControl on invalid input
        assert_eq!(config.get_ptt_keycode(), Keycode::RControl);
    }

    #[test]
    fn test_config_load_nonexistent() {
        let path = Path::new("non_existent_config.toml");
        let config = AppConfig::load(path).unwrap();
        assert_eq!(config.ptt_key, AppConfig::default().ptt_key);
    }
}
