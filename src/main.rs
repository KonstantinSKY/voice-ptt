mod api;
mod audio;
mod config;
mod injector;

use anyhow::{Context, Result};
use cpal::traits::StreamTrait;
use device_query::{DeviceQuery, DeviceState};
use dotenvy::dotenv;
use std::env;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tokio::time::{sleep, Duration};

use crate::api::WhisperClient;
use crate::audio::AudioSystem;
use crate::config::AppConfig;
use crate::injector::SystemInjector;

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Initialization
    SystemInjector::check_dependencies()?;

    let exe_path = env::current_exe()?;
    let exe_dir = exe_path
        .parent()
        .context("Failed to get executable directory")?;

    // Load .env
    let env_path = exe_dir.join(".env");
    if env_path.exists() {
        dotenvy::from_path(&env_path).ok();
    } else {
        let _ = dotenv();
    }

    let api_key =
        env::var("OPENAI_API_KEY").context("OPENAI_API_KEY environment variable must be set")?;

    // Load config.toml
    let config_path = exe_dir.join("config.toml");
    let app_config = AppConfig::load(&config_path)?;
    let ptt_key = app_config.get_ptt_keycode();

    // 2. Audio Setup
    let audio_system = AudioSystem::new()?;
    let wav_spec = audio_system.get_wav_spec();

    let audio_buffer = Arc::new(Mutex::new(Vec::new()));
    let is_recording = Arc::new(AtomicBool::new(false));

    let stream = audio_system.build_stream(audio_buffer.clone(), is_recording.clone())?;
    stream.play()?;

    // 3. Components
    let whisper_client = Arc::new(WhisperClient::new(api_key));
    let device_state = DeviceState::new();
    let (sound_start, sound_end) = app_config.get_sound_paths();

    println!("🚀 Voice PTT is ready! Hold [{:?}] to speak.", ptt_key);

    // 4. Main Event Loop
    loop {
        let keys = device_state.get_keys();

        if keys.contains(&ptt_key) {
            if !is_recording.load(Ordering::Relaxed) {
                // Start Recording
                SystemInjector::play_sound(app_config.sound_enabled, &sound_start);
                println!("🎙️ Recording...");

                if let Ok(mut buffer) = audio_buffer.lock() {
                    buffer.clear();
                }
                is_recording.store(true, Ordering::Relaxed);
            }
        } else if is_recording.load(Ordering::Relaxed) {
            // Stop Recording & Process
            is_recording.store(false, Ordering::Relaxed);
            SystemInjector::play_sound(app_config.sound_enabled, &sound_end);
            println!("⚙️ Processing...");

            let buffer_snapshot: Vec<i16> = {
                let buf = audio_buffer.lock().unwrap();
                buf.clone()
            };

            if !buffer_snapshot.is_empty() {
                let client_clone = whisper_client.clone();
                let app_config_clone = app_config.clone();

                // Process in a separate task to keep the loop responsive
                tokio::spawn(async move {
                    match client_clone
                        .transcribe(buffer_snapshot, wav_spec, &app_config_clone)
                        .await
                    {
                        Ok(text) => {
                            println!("📝 Transcribed: '{}'", text);
                            if let Err(e) = SystemInjector::type_text(
                                &text,
                                app_config_clone.typing_delay_ms,
                                app_config_clone.initial_delay_ms,
                            )
                            .await
                            {
                                eprintln!("❌ Injection error: {}", e);
                            }
                        }
                        Err(e) => {
                            eprintln!("❌ API Error: {}", e);
                            SystemInjector::notify("Voice PTT Error", &e.to_string());
                        }
                    }
                    println!("\n✅ Ready! Hold [{:?}] to speak.", ptt_key);
                });
            }
        }

        // Use async sleep to be friendly to the tokio runtime
        sleep(Duration::from_millis(50)).await;
    }
}
