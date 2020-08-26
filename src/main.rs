mod api;
mod audio;
mod config;
mod injector;

use anyhow::{Context, Result};
use cpal::traits::DeviceTrait;
use cpal::traits::StreamTrait;
use device_query::{DeviceQuery, DeviceState};
use dotenvy::dotenv;
use std::env;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::time::{sleep, Duration};

use crate::api::WhisperClient;
use crate::audio::AudioSystem;
use crate::config::AppConfig;
use crate::injector::SystemInjector;

enum CaptureMode {
    Cpal {
        audio_buffer: Arc<Mutex<Vec<i16>>>,
        is_recording: Arc<AtomicBool>,
        wav_spec: hound::WavSpec,
        _stream: cpal::Stream,
    },
    PwRecord {
        recorder: Option<Child>,
        current_file: Option<PathBuf>,
    },
}

fn start_pw_recording() -> Result<(Child, PathBuf)> {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let out_file = PathBuf::from(format!("/tmp/voice-ptt-{}.wav", ts));

    let child = Command::new("pw-record")
        .arg("--rate")
        .arg("16000")
        .arg("--channels")
        .arg("1")
        .arg("--format")
        .arg("s16")
        .arg(&out_file)
        .spawn()
        .context(
            "Failed to start pw-record. Install pipewire tools and ensure PipeWire is running.",
        )?;

    Ok((child, out_file))
}

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

    // 2. Audio Setup with fallback
    println!("Init audio...");
    let mut capture_mode = match AudioSystem::new() {
        Ok(audio_system) => {
            let device_name = audio_system
                .device
                .name()
                .unwrap_or_else(|_| "default".to_string());
            println!("Using input device: {}", device_name);

            let wav_spec = audio_system.get_wav_spec();
            let audio_buffer = Arc::new(Mutex::new(Vec::new()));
            let is_recording = Arc::new(AtomicBool::new(false));
            let stream = audio_system.build_stream(audio_buffer.clone(), is_recording.clone())?;
            stream.play()?;

            CaptureMode::Cpal {
                audio_buffer,
                is_recording,
                wav_spec,
                _stream: stream,
            }
        }
        Err(e) => {
            eprintln!("⚠️ cpal capture init failed: {}", e);
            eprintln!("⚠️ Falling back to PipeWire recorder (pw-record).");
            CaptureMode::PwRecord {
                recorder: None,
                current_file: None,
            }
        }
    };

    // 3. Components
    let whisper_client = Arc::new(WhisperClient::new(api_key));
    let device_state = DeviceState::new();
    let (sound_start, sound_end) = app_config.get_sound_paths();

    let version_info = "v0.1.2 (dynamic-paste)";
    println!("🚀 Voice PTT {} is ready! Hold [{:?}] to speak.", version_info, ptt_key);

    // 4. Main Event Loop
    loop {
        let keys = device_state.get_keys();

        if keys.contains(&ptt_key) {
            match &mut capture_mode {
                CaptureMode::Cpal {
                    audio_buffer,
                    is_recording,
                    ..
                } => {
                    if !is_recording.load(Ordering::Relaxed) {
                        SystemInjector::play_sound(app_config.sound_enabled, &sound_start);
                        println!("🎙️ Recording...");

                        if let Ok(mut buffer) = audio_buffer.lock() {
                            buffer.clear();
                        }
                        is_recording.store(true, Ordering::Relaxed);
                    }
                }
                CaptureMode::PwRecord {
                    recorder,
                    current_file,
                } => {
                    if recorder.is_none() {
                        SystemInjector::play_sound(app_config.sound_enabled, &sound_start);
                        println!("🎙️ Recording...");

                        match start_pw_recording() {
                            Ok((child, wav_path)) => {
                                *recorder = Some(child);
                                *current_file = Some(wav_path);
                            }
                            Err(e) => {
                                eprintln!("❌ Recorder start error: {}", e);
                                SystemInjector::notify("Voice PTT Error", &e.to_string());
                            }
                        }
                    }
                }
            }
        } else {
            match &mut capture_mode {
                CaptureMode::Cpal {
                    audio_buffer,
                    is_recording,
                    wav_spec,
                    ..
                } => {
                    if is_recording.load(Ordering::Relaxed) {
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
                            let wav_spec_clone = *wav_spec;

                            tokio::spawn(async move {
                                match client_clone
                                    .transcribe(buffer_snapshot, wav_spec_clone, &app_config_clone)
                                    .await
                                {
                                    Ok(text) => {
                                        println!("📝 Transcribed: '{}'", text);
                                        if let Err(e) = SystemInjector::type_text(
                                            &text,
                                            app_config_clone.typing_delay_ms,
                                            app_config_clone.initial_delay_ms,
                                            &app_config_clone,
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
                }
                CaptureMode::PwRecord {
                    recorder,
                    current_file,
                } => {
                    if recorder.is_some() {
                        if let Some(mut proc) = recorder.take() {
                            let _ = proc.kill();
                            let _ = proc.wait();
                        }
                        SystemInjector::play_sound(app_config.sound_enabled, &sound_end);
                        println!("⚙️ Processing...");

                        if let Some(recorded_file) = current_file.take() {
                            let size_ok = std::fs::metadata(&recorded_file)
                                .map(|m| m.len() > 44)
                                .unwrap_or(false);

                            if size_ok {
                                let client_clone = whisper_client.clone();
                                let app_config_clone = app_config.clone();

                                tokio::spawn(async move {
                                    match client_clone
                                        .transcribe_wav_file(&recorded_file, &app_config_clone)
                                        .await
                                    {
                                        Ok(text) => {
                                            println!("📝 Transcribed: '{}'", text);
                                            if let Err(e) = SystemInjector::type_text(
                                                &text,
                                                app_config_clone.typing_delay_ms,
                                                app_config_clone.initial_delay_ms,
                                                &app_config_clone,
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
                                    let _ = tokio::fs::remove_file(&recorded_file).await;
                                    println!("\n✅ Ready! Hold [{:?}] to speak.", ptt_key);
                                });
                            } else {
                                eprintln!("⚠️ Recorded audio file is empty.");
                            }
                        }
                    }
                }
            }
        }

        // Use async sleep to be friendly to the tokio runtime
        sleep(Duration::from_millis(20)).await;
    }
}
