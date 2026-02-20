use anyhow::{Context, Result};
use device_query::Keycode;
use serde::Deserialize;
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
